//! Shelf-packed RGBA8 glyph atlas.
//!
//! Texture format: RGB = 255, A = coverage (straight alpha, as `text.wgsl` assumes).
//! `antialiased: false` thresholds coverage (>= 128 -> 255, else 0).
//!
//! Growth: the texture starts at `initial` x `initial` and doubles (both dimensions,
//! keeping every existing glyph at its pixel position; only UVs change) up to `max`
//! (2048 by default). When a glyph does not fit even at `max`, the atlas is *reset*:
//! every glyph is evicted and re-rasterized on demand, `generation()` increases and the
//! whole texture is marked dirty. `layout` handles this transparently within a call (it
//! resets at most once, then drops glyphs that still do not fit, e.g. a single glyph
//! larger than the atlas, which is then drawn as blank advance).
//! Quads from earlier `layout` calls are stale once `generation()` changes.

use std::collections::HashMap;

use crate::fonts;
use crate::FontSpec;

pub const DEFAULT_INITIAL_SIZE: u32 = 256;
pub const DEFAULT_MAX_SIZE: u32 = 2048;
/// Transparent gap (right and bottom of every glyph) so bilinear sampling never bleeds.
const PAD: u32 = 1;

/// Region of the texture changed since the last [`Atlas::take_dirty`]. `resized` means
/// the texture was reallocated (growth or reset): re-create/upload the whole texture
/// (`x = y = 0`, `w`/`h` = full size).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtyRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub resized: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Pixels right of the pen origin to the bitmap's left edge.
    pub left: i32,
    /// Pixels above the baseline to the bitmap's top edge.
    pub top: i32,
}

struct Shelf {
    y: u32,
    h: u32,
    x: u32,
}

pub struct Atlas {
    font: fontdue::Font,
    fallback: bool,
    antialiased: bool,
    px: f32,
    ascent: i32,
    line_height: i32,
    width: u32,
    height: u32,
    max: u32,
    pixels: Vec<u8>,
    shelves: Vec<Shelf>,
    placed: HashMap<char, Option<Placed>>,
    advances: HashMap<char, i32>,
    dirty: Option<DirtyRegion>,
    generation: u64,
    resets: u32,
}

fn blank(size: u32) -> Vec<u8> {
    let mut v = vec![0u8; (size * size * 4) as usize];
    for p in v.chunks_exact_mut(4) {
        p[0] = 255;
        p[1] = 255;
        p[2] = 255;
    }
    v
}

impl Atlas {
    pub fn new(spec: &FontSpec, db: &fontdb::Database) -> Atlas {
        Atlas::with_limits(spec, db, DEFAULT_INITIAL_SIZE, DEFAULT_MAX_SIZE)
    }

    /// `initial` and `max` are clamped to `>= 1` and `max >= initial`; use powers of two.
    pub fn with_limits(spec: &FontSpec, db: &fontdb::Database, initial: u32, max: u32) -> Atlas {
        let loaded = fonts::load(spec, db);
        let initial = initial.max(1);
        let max = max.max(initial);
        let size_px = spec.size_px.max(1) as i32;
        // Win32 positive lfHeight = ascent + descent: scale so the font's
        // (ascent - descent) is exactly `size_px` pixels.
        let m1 = loaded.font.horizontal_line_metrics(1.0);
        let (asc1, span1) = match m1 {
            Some(m) if m.ascent - m.descent > 0.0 => (m.ascent, m.ascent - m.descent),
            _ => (0.8, 1.0),
        };
        let px = size_px as f32 / span1;
        let ascent = ((asc1 * px).round() as i32).clamp(0, size_px);
        Atlas {
            font: loaded.font,
            fallback: loaded.fallback,
            antialiased: spec.antialiased,
            px,
            ascent,
            line_height: size_px,
            width: initial,
            height: initial,
            max,
            pixels: blank(initial),
            shelves: Vec::new(),
            placed: HashMap::new(),
            advances: HashMap::new(),
            dirty: Some(DirtyRegion {
                x: 0,
                y: 0,
                w: initial,
                h: initial,
                resized: true,
            }),
            generation: 0,
            resets: 0,
        }
    }

    /// True when the requested face was not found and the bundled font is used.
    pub fn uses_fallback(&self) -> bool {
        self.fallback
    }
    /// Line advance in pixels: ascent + descent = the spec's `size_px` (external leading
    /// is not added, matching `ID3DXFont::DrawText`).
    pub fn line_height(&self) -> i32 {
        self.line_height
    }
    /// Baseline offset below the top of a line.
    pub fn ascent(&self) -> i32 {
        self.ascent
    }
    /// Number of times the atlas was reset because it was full at max size.
    pub fn resets(&self) -> u32 {
        self.resets
    }
    /// Increases whenever the texture is reallocated (growth/reset); UVs and glyph
    /// positions from older `layout` calls are then invalid.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// `(rgba8 pixels, width, height)`; row-major, `width * 4` bytes per row.
    pub fn texture_rgba(&self) -> (&[u8], u32, u32) {
        (&self.pixels, self.width, self.height)
    }

    /// Returns (and clears) the region changed since the last call; `None` if clean.
    pub fn take_dirty(&mut self) -> Option<DirtyRegion> {
        self.dirty.take()
    }

    /// Integer pen advance (GDI-style hinted width; no kerning).
    pub(crate) fn advance(&mut self, ch: char) -> i32 {
        if let Some(a) = self.advances.get(&ch) {
            return *a;
        }
        let a = self.font.metrics(ch, self.px).advance_width.round() as i32;
        self.advances.insert(ch, a);
        a
    }

    pub(crate) fn placed(&self, ch: char) -> Option<Placed> {
        self.placed.get(&ch).copied().flatten()
    }

    /// Make sure `ch` is in the atlas. `Err(())` means the atlas is full at max size.
    /// Glyphs without a bitmap (space) or that can never fit are recorded as absent
    /// and return `Ok`.
    pub(crate) fn ensure(&mut self, ch: char) -> Result<(), ()> {
        if self.placed.contains_key(&ch) {
            return Ok(());
        }
        let (m, bitmap) = self.font.rasterize(ch, self.px);
        let (w, h) = (m.width as u32, m.height as u32);
        if w == 0 || h == 0 {
            self.placed.insert(ch, None);
            return Ok(());
        }
        if w + PAD > self.max || h + PAD > self.max {
            self.placed.insert(ch, None); // can never fit: dropped
            return Ok(());
        }
        let (x, y) = loop {
            if let Some(pos) = self.pack(w, h) {
                break pos;
            }
            if self.width >= self.max {
                return Err(());
            }
            self.grow();
        };
        for row in 0..h {
            for col in 0..w {
                let cov = bitmap[(row * w + col) as usize];
                let a = if self.antialiased {
                    cov
                } else if cov >= 128 {
                    255
                } else {
                    0
                };
                let i = (((y + row) * self.width + x + col) * 4 + 3) as usize;
                self.pixels[i] = a;
            }
        }
        self.mark_dirty(x, y, w, h);
        self.placed.insert(
            ch,
            Some(Placed {
                x,
                y,
                w,
                h,
                left: m.xmin,
                top: m.ymin + m.height as i32,
            }),
        );
        Ok(())
    }

    fn pack(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (need_w, need_h) = (w + PAD, h + PAD);
        // best fit: the shortest existing shelf that is tall enough and has room
        let mut best: Option<usize> = None;
        for (i, s) in self.shelves.iter().enumerate() {
            if s.h >= need_h
                && s.x + need_w <= self.width
                && best.is_none_or(|b| s.h < self.shelves[b].h)
            {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            let s = &mut self.shelves[i];
            let pos = (s.x, s.y);
            s.x += need_w;
            return Some(pos);
        }
        let next_y = self.shelves.last().map_or(0, |s| s.y + s.h);
        if next_y + need_h <= self.height && need_w <= self.width {
            self.shelves.push(Shelf {
                y: next_y,
                h: need_h,
                x: need_w,
            });
            return Some((0, next_y));
        }
        None
    }

    fn grow(&mut self) {
        let (ow, nw) = (self.width, (self.width * 2).min(self.max));
        let mut np = blank(nw);
        for row in 0..self.height {
            let src = (row * ow * 4) as usize;
            let dst = (row * nw * 4) as usize;
            np[dst..dst + (ow * 4) as usize]
                .copy_from_slice(&self.pixels[src..src + (ow * 4) as usize]);
        }
        self.pixels = np;
        self.width = nw;
        self.height = nw;
        self.generation += 1;
        self.dirty = Some(DirtyRegion {
            x: 0,
            y: 0,
            w: nw,
            h: nw,
            resized: true,
        });
    }

    /// Evict everything (atlas full at max size).
    pub(crate) fn reset(&mut self) {
        self.pixels = blank(self.width);
        self.shelves.clear();
        self.placed.clear();
        self.generation += 1;
        self.resets += 1;
        self.dirty = Some(DirtyRegion {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
            resized: true,
        });
    }

    fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.dirty = Some(match self.dirty {
            None => DirtyRegion {
                x,
                y,
                w,
                h,
                resized: false,
            },
            Some(d) if d.resized => d,
            Some(d) => {
                let (x0, y0) = (d.x.min(x), d.y.min(y));
                let (x1, y1) = ((d.x + d.w).max(x + w), (d.y + d.h).max(y + h));
                DirtyRegion {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                    resized: false,
                }
            }
        });
    }

    pub(crate) fn tex_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
