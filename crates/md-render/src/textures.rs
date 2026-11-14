//! The texture store: disk textures loaded on demand with an LRU budget,
//! the generated noise textures, and random-texture (`randNN`) picks.
//!
//! Where files are found (CShaderParams::CacheParams, plugin.cpp:3158-3171):
//! for each extension of [`TEXTURE_EXTS`] in order, the file in the textures
//! directory (`<milkdrop2>/textures/`, set with [`TextureStore::scan_dir`])
//! or, if absent there, in the preset directory
//! ([`TextureStore::set_preset_dir`]). The C++ loop never `break`s, so every
//! candidate is loaded and the LAST one that loads wins (`textures/x.png`
//! beats `presets/x.jpg`), while a corrupt candidate falls through to the
//! others — see [`texture_candidates`]. Windows file names are
//! case-insensitive, so on every OS `sampler_Heart` finds `heart.JPG` and
//! all texture and noise names compare case-insensitively.
//!
//! Random textures (PickRandomTexture, plugin.cpp:2850-2918): the
//! candidates are the files of the textures directory (not the preset
//! directory) with a texture extension, extension chopped, duplicates kept
//! (`a.jpg` and `a.png` give two entries for root `a`). `randNN_<prefix>`
//! picks only among names starting with `<prefix>` (case-insensitive). The
//! C++ keeps one pick per slot per shader (`RandTexName` is local to
//! `CacheParams`), and `texsize_randNN[_x]` follows the slot's pick
//! whatever the sampler's prefix — see [`RandPicks`].

use crate::gpu::Gpu;
use crate::image_load::{apply_colorkey, load_image, ImageError};
use crate::noise::{
    dwords_to_rgba8, noise_2d, noise_3d, NOISE_2D, NOISE_2D_MIPS, NOISE_3D, NOISE_3D_MIPS,
};
use crate::texcache::{fold, TexCache, TextureId, TEXTURE_OVERHEAD_BYTES};
use crate::texture::{Texture2d, Texture3d};
use rand_core::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// File extensions tried, in order (`texture_exts`, plugin.cpp:2847 —
/// preflight F32). `.dib` files are BMPs.
pub const TEXTURE_EXTS: [&str; 6] = ["jpg", "dds", "png", "tga", "bmp", "dib"];

/// Why [`TextureStore::load`] produced no texture.
#[derive(Debug, thiserror::Error)]
pub enum TextureError {
    #[error("texture {root:?} not found (tried .jpg/.dds/.png/.tga/.bmp/.dib)")]
    NotFound { root: String },
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error("texture {path}: {w}x{h} exceeds the device limit {max}")]
    TooLarge {
        path: PathBuf,
        w: u32,
        h: u32,
        max: u32,
    },
}

/// The file `name` in `dir`, matched case-insensitively.
fn find_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    let want = name.to_lowercase();
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_str().map(str::to_lowercase).as_deref() == Some(&want))
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    hits.sort(); // deterministic when several spellings exist
    hits.into_iter().next()
}

/// The files CacheParams tries for texture `root`, in its loop order
/// (plugin.cpp:3158-3167): for each extension of [`TEXTURE_EXTS`], the
/// file in the first of `dirs` (textures dir, then preset dir) that has
/// it — at most one per extension. The loop has no `break`, so every
/// candidate is loaded in turn and the LAST one that loads wins; a failed
/// load just moves on (see [`TextureStore::load`]).
pub fn texture_candidates(dirs: &[&Path], root: &str) -> Vec<PathBuf> {
    TEXTURE_EXTS
        .iter()
        .filter_map(|ext| {
            dirs.iter()
                .find_map(|dir| find_in_dir(dir, &format!("{root}.{ext}")))
        })
        .collect()
}

/// The random-texture candidates of `dir`: file names with a texture
/// extension, the extension (after the last `.`) chopped, sorted
/// case-insensitively (FindFirstFile order on NTFS).
pub fn scan_texture_names(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| {
            let (stem, ext) = n.rsplit_once('.')?;
            TEXTURE_EXTS
                .iter()
                .any(|x| x.eq_ignore_ascii_case(ext))
                .then(|| stem.to_string())
        })
        .collect();
    names.sort_by_key(|n| (n.to_lowercase(), n.clone()));
    names
}

/// PickRandomTexture: one of `names` starting with `prefix`
/// (case-insensitive; empty matches all), chosen with `rng`; `None` when
/// there is none.
pub fn pick_random_texture<R: Rng + ?Sized>(
    names: &[String],
    prefix: &str,
    rng: &mut R,
) -> Option<String> {
    let prefix = fold(prefix);
    let pool: Vec<&String> = names
        .iter()
        .filter(|n| fold(n).starts_with(&prefix))
        .collect();
    if pool.is_empty() {
        return None;
    }
    Some(pool[rng.next_u32() as usize % pool.len()].clone())
}

/// The first [`TextureId`] of sprite textures, which live in their own
/// cache so the eviction budget never touches them.
const SPRITE_ID_BASE: u32 = 1 << 31;

/// The first id of sprite slots (one per `load_sprite`), which map to a
/// shared texture of the sprite cache.
const INSTANCE_ID_BASE: u32 = 3 << 30;

/// A noise texture of the store.
struct Noise<T> {
    name: &'static str,
    size: u32,
    tex: T,
}

/// Disk textures, noise textures and random-texture candidates.
pub struct TextureStore {
    cache: TexCache<Texture2d>,
    sprites: TexCache<Texture2d>,
    warned_sprites: HashSet<String>,
    sprite_instances: HashMap<u32, TextureId>,
    /// Kills so far, so instance ids are never reused.
    killed: u32,
    textures_dir: Option<PathBuf>,
    preset_dir: Option<PathBuf>,
    random_names: Vec<String>,
    noise2d: Vec<Noise<Texture2d>>,
    noise3d: Vec<Noise<Texture3d>>,
}

impl TextureStore {
    /// A store with the six noise textures generated from `seed` and no
    /// directories (call [`TextureStore::scan_dir`]).
    pub fn new(gpu: &Gpu, seed: u64) -> TextureStore {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(seed);
        let noise2d = NOISE_2D
            .iter()
            .map(|s| {
                let rgba = dwords_to_rgba8(&noise_2d(&mut rng, s.size, s.zoom));
                Noise {
                    name: s.name,
                    size: s.size,
                    tex: Texture2d::from_rgba8(gpu, s.size, s.size, &rgba, NOISE_2D_MIPS),
                }
            })
            .collect();
        let noise3d = NOISE_3D
            .iter()
            .map(|s| {
                let rgba = dwords_to_rgba8(&noise_3d(&mut rng, s.size, s.zoom));
                Noise {
                    name: s.name,
                    size: s.size,
                    tex: Texture3d::from_rgba8(gpu, s.size, s.size, s.size, &rgba, NOISE_3D_MIPS),
                }
            })
            .collect();
        TextureStore {
            cache: TexCache::new(),
            sprites: TexCache::starting_at(SPRITE_ID_BASE),
            warned_sprites: HashSet::new(),
            sprite_instances: HashMap::new(),
            killed: 0,
            textures_dir: None,
            preset_dir: None,
            random_names: Vec::new(),
            noise2d,
            noise3d,
        }
    }

    /// Use `dir` as the textures directory and (re)scan its random-texture
    /// candidates (`m_bNeedRescanTexturesDir`).
    pub fn scan_dir(&mut self, dir: &Path) {
        self.random_names = scan_texture_names(dir);
        self.textures_dir = Some(dir.to_owned());
    }

    /// The directory of the current preset, searched after the textures
    /// directory (`m_szPresetDir`).
    pub fn set_preset_dir(&mut self, dir: Option<&Path>) {
        self.preset_dir = dir.map(Path::to_owned);
    }

    /// Override the eviction budget (defaults [`crate::MAX_IMAGES`],
    /// [`crate::MAX_BYTES`]).
    pub fn set_budget(&mut self, max_images: usize, max_bytes: u64) {
        self.cache.max_images = max_images;
        self.cache.max_bytes = max_bytes;
    }

    /// Call when a preset load completes (`m_nPresetsLoadedTotal++`); the
    /// textures of the last two presets are never evicted.
    pub fn preset_loaded(&mut self) {
        self.cache.preset_loaded();
    }

    /// (evictable textures cached, their counted bytes).
    pub fn usage(&self) -> (usize, u64) {
        self.cache.usage()
    }

    /// The files `root` tries, in order (see [`texture_candidates`]).
    pub fn candidates(&self, root: &str) -> Vec<PathBuf> {
        let dirs: Vec<&Path> = [&self.textures_dir, &self.preset_dir]
            .into_iter()
            .flatten()
            .map(PathBuf::as_path)
            .collect();
        texture_candidates(&dirs, root)
    }

    /// The disk texture `root`: cached (its age refreshed), or found,
    /// decoded and uploaded with a full mip chain after evicting to the
    /// budget. Noise names are not disk textures (see
    /// [`TextureStore::noise2d`]).
    pub fn load(&mut self, gpu: &Gpu, root: &str) -> Result<TextureId, TextureError> {
        if let Some(id) = self.cache.lookup(root) {
            return Ok(id);
        }
        self.cache.make_room();
        // Mirror the C++ candidate loop: the last candidate that decodes
        // (and fits the device) wins; failures fall through to the next.
        let max = gpu.device().limits().max_texture_dimension_2d;
        let mut best = None;
        let mut err = TextureError::NotFound {
            root: root.to_string(),
        };
        for path in self.candidates(root) {
            match load_image(&path) {
                Ok(img) if img.width() > max || img.height() > max => {
                    let (w, h) = img.dimensions();
                    err = TextureError::TooLarge { path, w, h, max };
                }
                Ok(img) => best = Some(img),
                Err(e) => err = e.into(),
            }
        }
        let img = best.ok_or(err)?;
        let (w, h) = img.dimensions();
        let tex = Texture2d::from_rgba8(gpu, w, h, img.as_raw(), true);
        let bytes = u64::from(w) * u64::from(h) * 4 + TEXTURE_OVERHEAD_BYTES;
        Ok(self.cache.insert(root, tex, bytes))
    }

    /// A sprite's image (`LaunchSprite` -> `texmgr::LoadTex`,
    /// plugin.cpp:8567-8713, texmgr.cpp:70-135): the file at `path` (the
    /// caller joins relative names with the MilkDrop2 directory,
    /// plugin.cpp:8589-8594), decoded, with `colorkey` applied
    /// ([`apply_colorkey`]: `colorkey_lo`/`colorkey` of the sprite's INI
    /// section, default 0) and uploaded with a full mip chain
    /// (`D3DXCreateTextureFromFileEx`, `MipLevels = D3DX_DEFAULT`).
    ///
    /// Sprite textures live outside the eviction budget (the C++ keeps them
    /// in `texmgr`, not the texture cache) until [`TextureStore::kill_sprite`].
    /// A path already loaded (compared case-insensitively) gets a new slot
    /// sharing the existing texture and its original colour key, as `LoadTex`
    /// instances an already-loaded file (texmgr.cpp:76-87); each call returns
    /// its own handle, like a slot index.
    ///
    /// On failure no sprite exists: the C++ shows an error and `LaunchSprite`
    /// returns false (plugin.cpp:8667-8712), and `DrawUserSprites` skips
    /// slots without a surface (milkdropfs.cpp:3474). The failure is logged
    /// (a `tracing` warning) once per path.
    pub fn load_sprite(
        &mut self,
        gpu: &Gpu,
        path: &Path,
        colorkey: u32,
    ) -> Result<TextureId, TextureError> {
        let name = path.to_string_lossy();
        if let Some(shared) = self.sprites.lookup(&name) {
            return Ok(self.new_instance(shared));
        }
        let max = gpu.device().limits().max_texture_dimension_2d;
        let loaded = match load_image(path) {
            Ok(img) if img.width() > max || img.height() > max => Err(TextureError::TooLarge {
                path: path.to_owned(),
                w: img.width(),
                h: img.height(),
                max,
            }),
            Ok(img) => Ok(img),
            Err(e) => Err(e.into()),
        };
        let mut img = match loaded {
            Ok(img) => img,
            Err(e) => {
                if self.warned_sprites.insert(fold(&name)) {
                    tracing::warn!(sprite = %name, error = %e,
                        "sprite image cannot be loaded; the sprite is not created");
                }
                return Err(e);
            }
        };
        apply_colorkey(&mut img, colorkey);
        let (w, h) = img.dimensions();
        let tex = Texture2d::from_rgba8(gpu, w, h, img.as_raw(), true);
        let bytes = u64::from(w) * u64::from(h) * 4 + TEXTURE_OVERHEAD_BYTES;
        let shared = self.sprites.insert(&name, tex, bytes);
        Ok(self.new_instance(shared))
    }

    /// A new sprite slot sharing the texture `shared`.
    fn new_instance(&mut self, shared: TextureId) -> TextureId {
        let id = TextureId(INSTANCE_ID_BASE + self.sprite_instances.len() as u32 + self.killed);
        self.sprite_instances.insert(id.0, shared);
        id
    }

    /// `KillTex` (texmgr.cpp:151-172): free the sprite slot `id`; its
    /// handle goes stale (a second kill of it does nothing, as a slot
    /// without a surface does nothing). The texture itself is released
    /// only when no other slot still uses it (the C++ ref count, 158-168),
    /// after which a [`TextureStore::load_sprite`] of the path decodes again.
    pub fn kill_sprite(&mut self, id: TextureId) {
        let Some(shared) = self.sprite_instances.remove(&id.0) else {
            return;
        };
        self.killed += 1;
        if !self.sprite_instances.values().any(|&s| s == shared) {
            self.sprites.remove(shared);
        }
    }

    /// A texture by handle; `None` once evicted (or killed, for a sprite).
    pub fn texture(&self, id: TextureId) -> Option<&Texture2d> {
        if id.0 >= INSTANCE_ID_BASE {
            let shared = self.sprite_instances.get(&id.0)?;
            self.sprites.get(*shared)
        } else if id.0 >= SPRITE_ID_BASE {
            self.sprites.get(id)
        } else {
            self.cache.get(id)
        }
    }

    /// The 2D texture a sampler root binds: a noise texture, else the disk
    /// texture loaded on demand; `None` when missing or undecodable.
    pub fn get(&mut self, gpu: &Gpu, root: &str) -> Option<&Texture2d> {
        if let Some(i) = self.noise2d_index(root) {
            return Some(&self.noise2d[i].tex);
        }
        let id = self.load(gpu, root).ok()?;
        self.cache.get(id)
    }

    fn noise2d_index(&self, name: &str) -> Option<usize> {
        self.noise2d
            .iter()
            .position(|n| n.name.eq_ignore_ascii_case(name))
    }

    /// The noise texture `name` (`noise_lq`, `noise_lq_lite`, `noise_mq`,
    /// `noise_hq`).
    pub fn noise2d(&self, name: &str) -> Option<&Texture2d> {
        self.noise2d_index(name).map(|i| &self.noise2d[i].tex)
    }

    /// The noise volume `name` (`noisevol_lq`, `noisevol_hq`).
    pub fn noise3d(&self, name: &str) -> Option<&Texture3d> {
        self.noise3d
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(name))
            .map(|n| &n.tex)
    }

    /// The random-texture candidates (see the module docs).
    pub fn random_texture_names(&self) -> &[String] {
        &self.random_names
    }

    /// The size of a noise texture or a cached disk texture (no loading,
    /// age untouched) — the `texsize_<root>` value.
    pub fn size_of(&self, root: &str) -> Option<(u32, u32)> {
        if let Some(n) = self
            .noise2d
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(root))
        {
            return Some((n.size, n.size));
        }
        if let Some(n) = self
            .noise3d
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(root))
        {
            return Some((n.size, n.size));
        }
        let t = self.cache.get(self.cache.peek(root)?)?;
        Some((t.w, t.h))
    }
}

/// What a `randNN` slot holds while one shader's bindings are resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Slot {
    #[default]
    Unused,
    /// A sampler asked for the slot but no candidate matched.
    Failed,
    Picked(String),
}

/// The random-texture picks of one shader (`RandTexName[16]` of
/// `CacheParams`); make one per shader per preset load.
#[derive(Clone, Debug, Default)]
pub struct RandPicks {
    slots: [Slot; 16],
}

impl RandPicks {
    pub fn new() -> RandPicks {
        RandPicks::default()
    }

    /// The texture root a sampler root loads: for `randNN[_prefix]` a
    /// fresh pick among `candidates` (the store's
    /// [`TextureStore::random_texture_names`]) starting with `prefix`, recorded
    /// for slot NN (`None` when nothing matches — the C++ then fails to
    /// load `"[randNN] prefix*"`); any other root unchanged.
    pub fn resolve_sampler<R: Rng + ?Sized>(
        &mut self,
        candidates: &[String],
        root: &str,
        rng: &mut R,
    ) -> Option<String> {
        let Some((slot, prefix)) = md_hlsl::parse_rand_root(root) else {
            return Some(root.to_string());
        };
        let pick = pick_random_texture(candidates, prefix, rng);
        self.slots[usize::from(slot)] = match &pick {
            Some(p) => Slot::Picked(p.clone()),
            None => Slot::Failed,
        };
        pick
    }

    /// The texture root a `texsize_<root>` uniform reports: for
    /// `randNN[_x]` the slot's pick regardless of `_x` (ledger Task 4.5),
    /// `None` if the slot's pick failed, and plain `randNN` if no sampler
    /// used the slot (plugin.cpp:3277-3293); any other root unchanged.
    pub fn texsize_root<'a>(&'a self, root: &'a str) -> Option<&'a str> {
        let Some((slot, _)) = md_hlsl::parse_rand_root(root) else {
            return Some(root);
        };
        match &self.slots[usize::from(slot)] {
            Slot::Unused => Some(&root[..6]),
            Slot::Failed => None,
            Slot::Picked(p) => Some(p),
        }
    }
}
