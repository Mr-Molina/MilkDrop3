//! Decoding texture files to RGBA8, standing in for
//! `D3DXCreateTextureFromFileExW` (plugin.cpp:3176).
//!
//! JPEG, PNG, TGA and BMP go through the `image` crate (pure Rust); `.dib`
//! files are BMPs. The format comes from the file's magic bytes when it has
//! any (D3DX also sniffs content, so a PNG named `.jpg` loads), else from
//! the extension (TGA has no magic). DDS goes through `ddsfile`, and only
//! uncompressed layouts decode: legacy bitmask RGB(A) and luminance files
//! of 8, 16, 24 or 32 bits per pixel, and DX10 `R8G8B8A8`/`B8G8R8A8`/
//! `B8G8R8X8`. Block-compressed (FourCC) DDS and volumes return
//! [`ImageError::Unsupported`]. Only the top mip level of a DDS is used;
//! the store regenerates the chain.

use ddsfile::{Dds, DxgiFormat, PixelFormatFlags};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Why a texture file could not be decoded.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot decode {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("unsupported image {path}: {message}")]
    Unsupported { path: PathBuf, message: String },
}

/// Read and decode `path` to RGBA8 (rows top to bottom).
pub fn load_image(path: &Path) -> Result<RgbaImage, ImageError> {
    let bytes = std::fs::read(path).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    decode(&bytes, ext.as_deref()).map_err(|e| e.at(path))
}

/// A decode failure before the path is attached.
enum Fail {
    Decode(String),
    Unsupported(String),
}

impl Fail {
    fn at(self, path: &Path) -> ImageError {
        let path = path.to_owned();
        match self {
            Fail::Decode(message) => ImageError::Decode { path, message },
            Fail::Unsupported(message) => ImageError::Unsupported { path, message },
        }
    }
}

fn decode(bytes: &[u8], ext: Option<&str>) -> Result<RgbaImage, Fail> {
    if bytes.starts_with(b"DDS ") {
        return decode_dds(bytes);
    }
    let hint = match ext {
        Some("jpg" | "jpeg") => Some(ImageFormat::Jpeg),
        Some("png") => Some(ImageFormat::Png),
        Some("tga") => Some(ImageFormat::Tga),
        Some("bmp" | "dib") => Some(ImageFormat::Bmp),
        _ => None,
    };
    let mut reader = ImageReader::new(Cursor::new(bytes));
    if let Some(f) = hint {
        reader.set_format(f);
    }
    let reader = reader
        .with_guessed_format()
        .map_err(|e| Fail::Decode(e.to_string()))?;
    if reader.format().is_none() {
        return Err(Fail::Unsupported("unknown image format".into()));
    }
    match reader.decode() {
        Ok(img) => Ok(img.to_rgba8()),
        Err(image::ImageError::Unsupported(e)) => Err(Fail::Unsupported(e.to_string())),
        Err(e) => Err(Fail::Decode(e.to_string())),
    }
}

fn decode_dds(bytes: &[u8]) -> Result<RgbaImage, Fail> {
    let dds = Dds::read(bytes).map_err(|e| Fail::Decode(e.to_string()))?;
    let (w, h) = (dds.get_width(), dds.get_height());
    if w == 0 || h == 0 {
        return Err(Fail::Decode(format!("empty DDS {w}x{h}")));
    }
    if dds.get_depth() > 1 {
        return Err(Fail::Unsupported("volume DDS".into()));
    }
    let data = dds.get_data(0).map_err(|e| Fail::Decode(e.to_string()))?;
    let layout = if let Some(h10) = &dds.header10 {
        match h10.dxgi_format {
            DxgiFormat::R8G8B8A8_UNorm | DxgiFormat::R8G8B8A8_UNorm_sRGB => {
                Masks::rgba(32, 0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000)
            }
            DxgiFormat::B8G8R8A8_UNorm | DxgiFormat::B8G8R8A8_UNorm_sRGB => {
                Masks::rgba(32, 0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000)
            }
            DxgiFormat::B8G8R8X8_UNorm | DxgiFormat::B8G8R8X8_UNorm_sRGB => {
                Masks::rgba(32, 0xFF_0000, 0xFF00, 0xFF, 0)
            }
            f => return Err(Fail::Unsupported(format!("DDS format {f:?}"))),
        }
    } else {
        let spf = &dds.header.spf;
        if let Some(cc) = &spf.fourcc {
            let code = String::from_utf8_lossy(&cc.0.to_le_bytes()).into_owned();
            return Err(Fail::Unsupported(format!(
                "compressed DDS (FourCC {code:?})"
            )));
        }
        let bits = spf.rgb_bit_count.unwrap_or(0);
        let a = spf.a_bit_mask.unwrap_or(0);
        if spf.flags.contains(PixelFormatFlags::RGB) {
            Masks::rgba(
                bits,
                spf.r_bit_mask.unwrap_or(0),
                spf.g_bit_mask.unwrap_or(0),
                spf.b_bit_mask.unwrap_or(0),
                a,
            )
        } else if spf.flags.contains(PixelFormatFlags::LUMINANCE) {
            // ddsfile drops the masks of luminance files: L8, A8L8, L16.
            let l = if a == 0 && bits >= 16 { 0xFFFF } else { 0xFF };
            Masks::rgba(bits, l, l, l, a)
        } else {
            return Err(Fail::Unsupported("DDS pixel format".into()));
        }
    };
    layout.decode(w, h, data)
}

/// An uncompressed little-endian pixel layout.
struct Masks {
    bits: u32,
    /// R, G, B, A masks; a zero mask reads 0 (colour) or 255 (alpha).
    m: [u32; 4],
}

impl Masks {
    fn rgba(bits: u32, r: u32, g: u32, b: u32, a: u32) -> Masks {
        Masks {
            bits,
            m: [r, g, b, a],
        }
    }

    fn decode(&self, w: u32, h: u32, data: &[u8]) -> Result<RgbaImage, Fail> {
        if !matches!(self.bits, 8 | 16 | 24 | 32) {
            return Err(Fail::Unsupported(format!("{} bits per pixel", self.bits)));
        }
        let bpp = (self.bits / 8) as usize;
        let need = w as usize * h as usize * bpp;
        if data.len() < need {
            return Err(Fail::Decode(format!(
                "DDS data too short: {} < {need} bytes",
                data.len()
            )));
        }
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for px in data[..need].chunks_exact(bpp) {
            let mut word = [0u8; 4];
            word[..bpp].copy_from_slice(px);
            let v = u32::from_le_bytes(word);
            for (i, &m) in self.m.iter().enumerate() {
                out.push(channel(v, m, i == 3));
            }
        }
        RgbaImage::from_raw(w, h, out).ok_or_else(|| Fail::Decode("DDS size".into()))
    }
}

/// The value under mask `m` scaled to 8 bits.
fn channel(v: u32, m: u32, alpha: bool) -> u8 {
    if m == 0 {
        return if alpha { 255 } else { 0 };
    }
    let x = u64::from((v & m) >> m.trailing_zeros());
    let max = (1u64 << m.count_ones()) - 1;
    ((x * 255 + max / 2) / max) as u8
}

/// The colour key of `D3DXCreateTextureFromFileEx` as `texmgr::LoadTex`
/// passes it, `0xFF000000 | ck` (texmgr.cpp:109): every pixel whose ARGB
/// equals the key exactly becomes transparent black (`0x00000000`). One
/// packed colour, not a range (plugin.cpp:142-144: the old `colorkey_lo` /
/// `colorkey_hi` pair "have been replaced with just one setting"), and the
/// key's alpha is forced to `0xFF`, so only fully opaque pixels can match.
/// `ck == 0` is therefore not "off": opaque black is keyed.
pub fn apply_colorkey(img: &mut RgbaImage, colorkey: u32) {
    let key = colorkey & 0x00FF_FFFF;
    for p in img.pixels_mut() {
        let [r, g, b, a] = p.0;
        if a == 255 && u32::from_be_bytes([0, r, g, b]) == key {
            p.0 = [0; 4];
        }
    }
}

#[cfg(test)]
mod colorkey_tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn colorkey_is_an_exact_packed_match_on_opaque_pixels() {
        // D3DXCreateTextureFromFileEx's ColorKey = 0xFF000000 | ck: an exact
        // ARGB match becomes transparent black (texmgr.cpp:109).
        let mut img = RgbaImage::new(5, 1);
        let key = 0x0010_2030_u32;
        let px = [
            [0x10, 0x20, 0x30, 255], // the key: keyed out
            [0x10, 0x20, 0x31, 255], // one off: kept
            [0x10, 0x20, 0x30, 128], // translucent, so ARGB differs: kept
            [0x30, 0x20, 0x10, 255], // swapped channels: kept
            [0x00, 0x00, 0x00, 255], // black, not the key: kept
        ];
        for (x, p) in px.iter().enumerate() {
            img.put_pixel(x as u32, 0, Rgba(*p));
        }
        apply_colorkey(&mut img, key);
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 0, 0]);
        for x in 1..5 {
            assert_eq!(img.get_pixel(x, 0).0, px[x as usize], "pixel {x}");
        }
        // The alpha byte of `ck` is ORed with 0xFF, so it cannot matter.
        let mut img2 = RgbaImage::from_pixel(1, 1, Rgba([0x10, 0x20, 0x30, 255]));
        apply_colorkey(&mut img2, 0x7F10_2030);
        assert_eq!(img2.get_pixel(0, 0).0, [0, 0, 0, 0]);
        // The default key 0 keys opaque black (0xFF000000 is not "off").
        let mut img3 = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        apply_colorkey(&mut img3, 0);
        assert_eq!(img3.get_pixel(0, 0).0, [0, 0, 0, 0]);
    }
}
