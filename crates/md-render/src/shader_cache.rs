//! The preset shader cache: translation runs once per shader per process
//! (memory) and, with a cache directory, once per machine (disk).
//!
//! Key (preflight F2): SHA-256 over the md-hlsl crate version
//! ([`md_hlsl::VERSION`]) and everything [`md_hlsl::translate`] assembles —
//! shader kind, `ps_version`, preamble and body — each length-prefixed. The
//! assembled HLSL is a pure function of those inputs, so this is a hash of
//! the assembled text without re-implementing the assembly.
//!
//! Memory: key → the shared [`CachedShader`] (translation result plus its
//! `wgpu::ShaderModule`), or the [`TranslateError`] (a broken preset is not
//! re-translated on every load). At [`MAX_MEMORY_ENTRIES`] the map is
//! flushed.
//!
//! Disk (F35): `RenderSettings::shader_cache_dir` supplied by the caller
//! (md-render does not depend on md-platform). One file per successful
//! translation, `<dir>/<hex key>.mdshader`, holding the whole
//! [`Translated`] (WGSL alone loses the binding info) in a small line
//! format (see [`encode`]) with a format header, the md-hlsl version and a
//! SHA-256 of the WGSL. An entry that is unreadable, from another format or
//! translator version, fails its checksum or fails naga validation is
//! ignored and regenerated; write failures are logged and ignored.

use crate::gpu::{block_on, Gpu};
use md_hlsl::{SamplerBinding, ShaderInput, ShaderKind, TranslateError, Translated};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// First line of every disk entry; bump on any format change.
pub const CACHE_FORMAT: &str = "md-render shader cache 1";
/// Memory entries kept before the map is flushed.
pub const MAX_MEMORY_ENTRIES: usize = 512;
/// File extension of disk entries.
pub const CACHE_EXT: &str = "mdshader";

/// A cache key: SHA-256 of the translator inputs (see the module docs).
pub type ShaderKey = [u8; 32];

/// The key of `input`.
pub fn shader_key(input: &ShaderInput) -> ShaderKey {
    let kind = match input.kind {
        ShaderKind::Warp => "warp",
        ShaderKind::Comp => "comp",
    };
    let version = format!("{:?}", input.ps_version);
    let mut h = Sha256::new();
    for part in [md_hlsl::VERSION, kind, &version, input.preamble, input.body] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    h.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn sha256_hex(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// A word of the line format: non-empty, no whitespace.
fn is_word(s: &str) -> bool {
    !s.is_empty() && !s.contains(char::is_whitespace)
}

/// The disk form of `t`, or `None` if a name cannot be stored (never for
/// translator output: names are HLSL identifiers). Layout, one item per
/// line:
///
/// ```text
/// md-render shader cache 1
/// md-hlsl <version>
/// sha256 <hex of the WGSL>
/// highest_blur <0..3>
/// texsize <full name>                                     (repeated, in order)
/// sampler <slot> <wrap 0|1> <bilinear 0|1> <dims> <name> <root>   (repeated)
/// wgsl
/// <the WGSL, verbatim, to end of file>
/// ```
pub fn encode(t: &Translated) -> Option<String> {
    let mut s = format!(
        "{CACHE_FORMAT}\nmd-hlsl {}\nsha256 {}\nhighest_blur {}\n",
        md_hlsl::VERSION,
        sha256_hex(&t.wgsl),
        t.highest_blur
    );
    for n in &t.texsize_names {
        if !is_word(n) {
            return None;
        }
        let _ = writeln!(s, "texsize {n}");
    }
    for b in &t.samplers {
        if !is_word(&b.name) || !is_word(&b.root) {
            return None;
        }
        let _ = writeln!(
            s,
            "sampler {} {} {} {} {} {}",
            b.slot,
            u8::from(b.wrap),
            u8::from(b.bilinear),
            b.dims,
            b.name,
            b.root
        );
    }
    s.push_str("wgsl\n");
    s.push_str(&t.wgsl);
    Some(s)
}

fn flag(s: &str) -> Option<bool> {
    match s {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// Parse an [`encode`]d entry; `None` if it is malformed, from another
/// format or md-hlsl version, or its WGSL fails the checksum. (The WGSL is
/// not validated here.)
pub fn decode(text: &str) -> Option<Translated> {
    let (head, wgsl) = text.split_once("\nwgsl\n")?;
    let mut lines = head.lines();
    if lines.next()? != CACHE_FORMAT {
        return None;
    }
    if lines.next()? != format!("md-hlsl {}", md_hlsl::VERSION) {
        return None;
    }
    if lines.next()?.strip_prefix("sha256 ")? != sha256_hex(wgsl) {
        return None;
    }
    let highest_blur: u8 = lines.next()?.strip_prefix("highest_blur ")?.parse().ok()?;
    if highest_blur > 3 {
        return None;
    }
    let (mut texsize_names, mut samplers) = (Vec::new(), Vec::<SamplerBinding>::new());
    for line in lines {
        let mut w = line.split(' ');
        match w.next()? {
            "texsize" if samplers.is_empty() => texsize_names.push(w.next()?.to_string()),
            "sampler" => {
                let slot: u32 = w.next()?.parse().ok()?;
                let (wrap, bilinear) = (flag(w.next()?)?, flag(w.next()?)?);
                let dims: u8 = w.next()?.parse().ok()?;
                let (name, root) = (w.next()?.to_string(), w.next()?.to_string());
                let in_order = usize::try_from(slot).ok()? == samplers.len();
                if !in_order || !(dims == 2 || dims == 3) || !is_word(&name) || !is_word(&root) {
                    return None;
                }
                samplers.push(SamplerBinding {
                    name,
                    root,
                    slot,
                    wrap,
                    bilinear,
                    dims,
                });
            }
            _ => return None,
        }
        if w.next().is_some() {
            return None;
        }
    }
    if texsize_names.iter().any(|n| !is_word(n)) || samplers.len() > md_hlsl::MAX_SAMPLERS {
        return None;
    }
    Some(Translated {
        wgsl: wgsl.to_string(),
        samplers,
        texsize_names,
        highest_blur,
    })
}

/// A translated shader and its module, shared by every preset load that
/// produced the same key.
#[derive(Debug)]
pub struct CachedShader {
    pub translated: Translated,
    pub module: wgpu::ShaderModule,
}

/// The memory + disk shader cache (see the module docs).
#[derive(Debug)]
pub struct ShaderCache {
    dir: Option<PathBuf>,
    memory: HashMap<ShaderKey, Result<Arc<CachedShader>, TranslateError>>,
    translations: u64,
    disk_hits: u64,
}

impl ShaderCache {
    /// An empty cache; `dir` enables the disk cache (created on first
    /// write).
    pub fn new(dir: Option<PathBuf>) -> ShaderCache {
        ShaderCache {
            dir,
            memory: HashMap::new(),
            translations: 0,
            disk_hits: 0,
        }
    }

    /// The disk cache directory, if any.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Calls of [`md_hlsl::translate`] so far (test hook: a hit in either
    /// cache does not translate).
    pub fn translations(&self) -> u64 {
        self.translations
    }

    /// Shaders taken from the disk cache so far.
    pub fn disk_hits(&self) -> u64 {
        self.disk_hits
    }

    /// The disk entry of `key` (`None` without a directory).
    pub fn entry_path(&self, key: &ShaderKey) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(format!("{}.{CACHE_EXT}", hex(key))))
    }

    /// The shader for `input`: from memory, else from disk, else
    /// translated (and written to disk). The error is the translator's, or
    /// wgpu's rejection of the module as an error at line 0.
    pub fn get(
        &mut self,
        gpu: &Gpu,
        input: &ShaderInput,
    ) -> Result<Arc<CachedShader>, TranslateError> {
        let key = shader_key(input);
        if let Some(hit) = self.memory.get(&key) {
            return hit.clone();
        }
        let result = self.load(gpu, input, &key);
        if self.memory.len() >= MAX_MEMORY_ENTRIES {
            self.memory.clear();
        }
        self.memory.insert(key, result.clone());
        result
    }

    fn load(
        &mut self,
        gpu: &Gpu,
        input: &ShaderInput,
        key: &ShaderKey,
    ) -> Result<Arc<CachedShader>, TranslateError> {
        if let Some(translated) = self.read_disk(key) {
            self.disk_hits += 1;
            let module = create_module(gpu, &translated.wgsl)?;
            return Ok(Arc::new(CachedShader { translated, module }));
        }
        self.translations += 1;
        let translated = md_hlsl::translate(input)?;
        let module = create_module(gpu, &translated.wgsl)?;
        self.write_disk(key, &translated);
        Ok(Arc::new(CachedShader { translated, module }))
    }

    fn read_disk(&self, key: &ShaderKey) -> Option<Translated> {
        let path = self.entry_path(key)?;
        let text = std::fs::read_to_string(&path).ok()?;
        let t = decode(&text).filter(|t| md_hlsl::validate_wgsl(&t.wgsl).is_ok());
        if t.is_none() {
            tracing::debug!(path = %path.display(), "ignoring invalid shader cache entry");
        }
        t
    }

    fn write_disk(&self, key: &ShaderKey, t: &Translated) {
        let (Some(path), Some(text)) = (self.entry_path(key), encode(t)) else {
            return;
        };
        // Write a temporary file and rename it, so a reader never sees a
        // partial entry (and a torn write fails the checksum anyway).
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        let res = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(e) = res {
            let _ = std::fs::remove_file(&tmp);
            tracing::debug!(path = %path.display(), error = %e, "cannot write shader cache entry");
        }
    }
}

/// Create a module from validated WGSL, turning a wgpu validation error
/// into a [`TranslateError`] at line 0 instead of the default panic.
fn create_module(gpu: &Gpu, wgsl: &str) -> Result<wgpu::ShaderModule, TranslateError> {
    let scope = gpu.device().push_error_scope(wgpu::ErrorFilter::Validation);
    let module = gpu
        .device()
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("md-render preset shader"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
    match block_on(scope.pop()) {
        None => Ok(module),
        Some(e) => Err(TranslateError::new(
            0,
            0,
            format!("wgpu rejected the shader module: {e}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Translated {
        Translated {
            wgsl: "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n"
                .to_string(),
            samplers: vec![
                SamplerBinding {
                    name: "sampler_main".into(),
                    root: "main".into(),
                    slot: 0,
                    wrap: true,
                    bilinear: true,
                    dims: 2,
                },
                SamplerBinding {
                    name: "sampler_pc_noisevol_hq".into(),
                    root: "noisevol_hq".into(),
                    slot: 1,
                    wrap: false,
                    bilinear: false,
                    dims: 3,
                },
            ],
            texsize_names: vec!["texsize_main".into(), "texsize_rand01".into()],
            highest_blur: 2,
        }
    }

    fn input(body: &str) -> ShaderInput<'_> {
        ShaderInput {
            kind: ShaderKind::Warp,
            body,
            preamble: "pre",
            ps_version: md_hlsl::PsVersion::Ps2_0,
        }
    }

    #[test]
    fn encode_decode_round_trips() {
        let t = sample();
        let text = encode(&t).unwrap();
        assert!(text.starts_with(CACHE_FORMAT));
        assert_eq!(decode(&text), Some(t));
        let empty = Translated {
            samplers: vec![],
            texsize_names: vec![],
            highest_blur: 0,
            ..sample()
        };
        assert_eq!(decode(&encode(&empty).unwrap()), Some(empty));
    }

    #[test]
    fn corrupt_entries_are_rejected() {
        let text = encode(&sample()).unwrap();
        assert_eq!(decode(""), None);
        assert_eq!(decode("garbage\n"), None);
        // Truncated WGSL fails the checksum.
        assert_eq!(decode(&text[..text.len() - 3]), None);
        // Another format or translator version.
        assert_eq!(
            decode(&text.replace(CACHE_FORMAT, "md-render shader cache 0")),
            None
        );
        let v = format!("md-hlsl {}", md_hlsl::VERSION);
        assert_eq!(decode(&text.replacen(&v, "md-hlsl 0.0.0-x", 1)), None);
        // Bad fields.
        assert_eq!(
            decode(&text.replace("sampler 1 0 0 3", "sampler 2 0 0 3")),
            None
        );
        assert_eq!(
            decode(&text.replace("sampler 1 0 0 3", "sampler 1 0 0 4")),
            None
        );
        assert_eq!(
            decode(&text.replace("highest_blur 2", "highest_blur 9")),
            None
        );
        assert_eq!(
            decode(&text.replace("texsize_main", "texsize_main extra")),
            None
        );
    }

    #[test]
    fn names_with_whitespace_are_not_stored() {
        let mut t = sample();
        t.samplers[0].root = "a b".into();
        assert_eq!(encode(&t), None);
    }

    #[test]
    fn key_covers_every_input() {
        let base = shader_key(&input("body"));
        assert_eq!(base, shader_key(&input("body")));
        assert_ne!(base, shader_key(&input("body2")));
        assert_ne!(
            base,
            shader_key(&ShaderInput {
                kind: ShaderKind::Comp,
                ..input("body")
            })
        );
        assert_ne!(
            base,
            shader_key(&ShaderInput {
                preamble: "pre2",
                ..input("body")
            })
        );
        let v4 = ShaderInput {
            ps_version: md_hlsl::PsVersion::Ps4_0,
            ..input("body")
        };
        assert_ne!(base, shader_key(&v4));
        // Length prefixes: moving text between parts changes the key.
        let moved = ShaderInput {
            preamble: "prebo",
            ..input("dy")
        };
        assert_ne!(base, shader_key(&moved));
    }
}
