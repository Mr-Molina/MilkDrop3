//! Sampler and `texsize_*` resolution for one preset shader — the texture
//! half of `CShaderParams::CacheParams` (plugin.cpp:2940-3326@6f7cd5d) and the
//! sampler states of `ApplyShaderParams` (milkdropfs.cpp:3826-3853@6f7cd5d).
//!
//! Each [`SamplerBinding`] root binds:
//! - `main` → the ping-pong texture the pass reads ([`SlotSource::Main`];
//!   the C++ binds `m_lpVS[0]`, which is never the warp target and is the
//!   freshly drawn frame at composite time);
//! - `blur1`, `blur2`, `blur3` → blur textures 1, 3, 5 (`m_lpBlur[1/3/5]`);
//! - a noise name → the store's noise texture (`noisevol_*` are volumes);
//! - `randNN[_prefix]` → a random pick from the textures directory, one
//!   [`RandPicks`] per shader (the C++'s `RandTexName` is local to
//!   `CacheParams`), drawn from the renderer's RNG;
//! - anything else → [`TextureStore`]'s disk texture of that name.
//!
//! The C++ compares `main`/`blurN` case-sensitively (`wcscmp`); so does
//! this. A texture that cannot be found or decoded binds the 1×1 white
//! fallback and is logged ONCE per renderer at WARN (Review Focus #1; the
//! C++ aborted the rest of `CacheParams` with an on-screen error). A
//! binding whose dimension does not match what the root provides (a volume
//! sampled with `tex2D`, `main` with `tex3D`) binds the fallback of the
//! shader's dimension — D3D9 left that undefined.
//!
//! `texsize_<root>` is `(w, h, 1/w, 1/h)` of the texture `root` names
//! (`randNN[_x]` follows slot NN's pick, ledger Task 4.5), looked up like
//! the C++ among loaded textures, noise and blur textures; it is zero, as
//! in the C++, when nothing matches (notably `texsize_main`, which the C++
//! never registered) — logged at DEBUG.
//!
//! Filtering (`ApplyShaderParams`): bilinear → linear min/mag/mip, point →
//! nearest min/mag/mip; wrap → repeat, else clamp. The C++ hard-codes
//! `bAniso = false` for preset samplers, so `RenderSettings::anisotropic`
//! does not apply here.

use crate::fixed::SamplerKind;
use crate::gpu::Gpu;
use crate::texture::{Texture2d, Texture3d};
use crate::textures::{RandPicks, TextureStore};
use md_hlsl::{SamplerBinding, Translated};
use std::collections::HashSet;

/// What one sampler slot of a preset shader binds.
#[derive(Clone, Debug)]
pub enum SlotSource {
    /// The ping-pong texture chosen when drawing (see
    /// [`crate::ShaderProgram::bind`]).
    Main,
    /// Blur level 1..=3 (blur texture `2 * level - 1`).
    Blur(u8),
    Tex2d(Texture2d),
    Tex3d(Texture3d),
}

/// The D3D9 sampler state of a binding.
pub fn sampler_kind(b: &SamplerBinding) -> SamplerKind {
    match (b.bilinear, b.wrap) {
        (true, true) => SamplerKind::LinearWrap,
        (true, false) => SamplerKind::LinearClamp,
        (false, true) => SamplerKind::PointWrap,
        (false, false) => SamplerKind::PointClamp,
    }
}

/// The blur level of a `blurN` root (C++ spelling, case-sensitive).
fn blur_level(root: &str) -> Option<u8> {
    match root {
        "blur1" => Some(1),
        "blur2" => Some(2),
        "blur3" => Some(3),
        _ => None,
    }
}

/// Everything resolution reads and updates, borrowed from the renderer.
pub(crate) struct Resolver<'a, R: rand_core::Rng> {
    pub gpu: &'a Gpu,
    pub textures: &'a mut TextureStore,
    pub rng: &'a mut R,
    /// Texture names already warned about (case-folded).
    pub warned: &'a mut HashSet<String>,
    pub fallback_2d: &'a Texture2d,
    pub fallback_3d: &'a Texture3d,
    /// Sizes of the six blur textures.
    pub blur_sizes: [(u32, u32); 6],
}

impl<R: rand_core::Rng> Resolver<'_, R> {
    fn warn_once(&mut self, name: &str, why: &str) {
        if self.warned.insert(name.to_lowercase()) {
            tracing::warn!(texture = %name, reason = %why,
                "preset texture unavailable; binding the 1x1 white fallback");
        }
    }

    fn fallback(&mut self, dims: u8, name: &str, why: &str) -> SlotSource {
        self.warn_once(name, why);
        if dims == 3 {
            SlotSource::Tex3d(self.fallback_3d.clone())
        } else {
            SlotSource::Tex2d(self.fallback_2d.clone())
        }
    }

    /// The source of one binding (`picks` records `randNN` choices).
    fn source(&mut self, b: &SamplerBinding, picks: &mut RandPicks) -> SlotSource {
        let root = b.root.as_str();
        let is_2d = b.dims == 2;
        if root == "main" {
            return if is_2d {
                SlotSource::Main
            } else {
                self.fallback(b.dims, root, "main is a 2D texture")
            };
        }
        if let Some(level) = blur_level(root) {
            return if is_2d {
                SlotSource::Blur(level)
            } else {
                self.fallback(b.dims, root, "blur textures are 2D")
            };
        }
        if let Some(t) = self.textures.noise3d(root) {
            return if is_2d {
                self.fallback(b.dims, root, "noise volume sampled as 2D")
            } else {
                SlotSource::Tex3d(t.clone())
            };
        }
        if !is_2d {
            return self.fallback(b.dims, root, "only noisevol_* textures are volumes");
        }
        let names = match md_hlsl::parse_rand_root(root) {
            Some(_) => self.textures.random_texture_names().to_vec(),
            None => Vec::new(),
        };
        let Some(name) = picks.resolve_sampler(&names, root, &mut *self.rng) else {
            return self.fallback(2, root, "no random texture matches");
        };
        if let Some(t) = self.textures.noise2d(&name) {
            return SlotSource::Tex2d(t.clone());
        }
        match self.textures.load(self.gpu, &name) {
            Ok(id) => match self.textures.texture(id) {
                Some(t) => SlotSource::Tex2d(t.clone()),
                None => self.fallback(2, &name, "evicted while loading"),
            },
            Err(e) => self.fallback(2, &name, &e.to_string()),
        }
    }

    /// The size `texsize_<root>` reports, if any texture matches.
    fn texsize_of(&self, root: &str, picks: &RandPicks) -> Option<(u32, u32)> {
        let root = picks.texsize_root(root)?;
        match blur_level(root) {
            Some(level) => Some(self.blur_sizes[usize::from(level) * 2 - 1]),
            None => self.textures.size_of(root),
        }
    }

    /// Resolve every sampler of `t` and then its `texsize_*` values, in
    /// `t`'s orders (samplers first: a texsize can only name a texture a
    /// sampler loaded, as in the C++'s two passes).
    pub fn resolve(&mut self, t: &Translated) -> (Vec<SlotSource>, Vec<[f32; 4]>) {
        let mut picks = RandPicks::new();
        let sources = t
            .samplers
            .iter()
            .map(|b| self.source(b, &mut picks))
            .collect();
        let texsizes = t
            .texsize_names
            .iter()
            .map(|name| {
                let root = md_hlsl::texsize_root(name).unwrap_or(name);
                match self.texsize_of(root, &picks) {
                    Some((w, h)) => {
                        let (w, h) = (w as f32, h as f32);
                        [w, h, 1.0 / w, 1.0 / h]
                    }
                    None => {
                        tracing::debug!(name = %name, "texsize for a texture not in use: zero");
                        [0.0; 4]
                    }
                }
            })
            .collect();
        (sources, texsizes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(bilinear: bool, wrap: bool) -> SamplerBinding {
        SamplerBinding {
            name: "sampler_x".into(),
            root: "x".into(),
            slot: 0,
            wrap,
            bilinear,
            dims: 2,
        }
    }

    #[test]
    fn sampler_states_follow_the_binding() {
        assert_eq!(sampler_kind(&binding(true, true)), SamplerKind::LinearWrap);
        assert_eq!(
            sampler_kind(&binding(true, false)),
            SamplerKind::LinearClamp
        );
        assert_eq!(sampler_kind(&binding(false, true)), SamplerKind::PointWrap);
        assert_eq!(
            sampler_kind(&binding(false, false)),
            SamplerKind::PointClamp
        );
    }

    #[test]
    fn blur_roots_are_case_sensitive() {
        assert_eq!(blur_level("blur2"), Some(2));
        assert_eq!(blur_level("Blur2"), None);
        assert_eq!(blur_level("blur4"), None);
    }
}
