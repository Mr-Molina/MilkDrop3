//! ns-eel2 RAM: `megabuf` (per-VM sparse blocks) and `gmegabuf` (process-wide).
//!
//! Semantics are pinned to `code/ns-eel2/nseel-ram.c` and `_asm_megabuf`:
//!
//! * `megabuf(i)` index = `trunc(i + 0.00001)` (`g_closefact` is added, then
//!   `fistp` runs with the rounding mode forced to chop by `GLUE_CALL_CODE`,
//!   so negatives truncate toward zero: `get(-0.5)` reads slot 0).
//! * An index outside `[0, 8388608)` reads 0 and ignores writes.
//! * Blocks of 65536 values are allocated on first write; reads never allocate.
//! * `gmegabuf` differs: MilkDrop never calls `NSEEL_VM_SetGRAM`, so
//!   `__NSEEL_RAMAllocGMEM` falls back to one flat static buffer of
//!   `NSEEL_SHARED_GRAM_SIZE = 1 << 20` values indexed by `w & (2^20 - 1)`.
//!   Every index wraps (negative ones included); nothing is out of range.

/// Values per block (`NSEEL_RAM_ITEMSPERBLOCK`).
pub const ITEMS_PER_BLOCK: usize = 65536;
/// Number of blocks (`NSEEL_RAM_BLOCKS`).
pub const BLOCKS: usize = 128;
/// Total addressable slots of a `MegaBuf`.
pub const MEGABUF_SIZE: i64 = (ITEMS_PER_BLOCK * BLOCKS) as i64;
/// `NSEEL_SHARED_GRAM_SIZE`.
pub const GRAM_SIZE: usize = 1 << 20;
/// `NSEEL_CLOSEFACTOR`, added before truncating a `megabuf`/`gmegabuf` index.
const INDEX_EPS: f64 = 0.00001;
/// Epsilon `memcpy`/`memset` add to their three arguments.
const BULK_EPS: f64 = 0.0001;

/// `EEL_F2int` under chop rounding: truncate toward zero; values that do not
/// fit an `i32` (and NaN) give x87's "integer indefinite", `i32::MIN`.
pub(crate) fn eel_int(x: f64) -> i32 {
    let t = x.trunc();
    if (-2147483648.0..=2147483647.0).contains(&t) {
        t as i32
    } else {
        i32::MIN
    }
}

type Block = Box<[f64; ITEMS_PER_BLOCK]>;

fn new_block() -> Block {
    vec![0.0; ITEMS_PER_BLOCK]
        .into_boxed_slice()
        .try_into()
        .expect("block length is ITEMS_PER_BLOCK")
}

/// Sparse 128 x 65536 array of `f64` backing `megabuf`.
#[derive(Default)]
pub struct MegaBuf {
    blocks: Vec<Option<Block>>,
}

impl MegaBuf {
    pub fn new() -> MegaBuf {
        MegaBuf {
            blocks: (0..BLOCKS).map(|_| None).collect(),
        }
    }

    fn slot(i: f64) -> Option<(usize, usize)> {
        let w = i64::from(eel_int(i + INDEX_EPS));
        Self::split(w)
    }

    fn split(w: i64) -> Option<(usize, usize)> {
        if (0..MEGABUF_SIZE).contains(&w) {
            Some((w as usize / ITEMS_PER_BLOCK, w as usize % ITEMS_PER_BLOCK))
        } else {
            None
        }
    }

    /// Value at `i`; 0 when out of range or in an unallocated block.
    pub fn get(&self, i: f64) -> f64 {
        match Self::slot(i) {
            Some((b, o)) => self
                .blocks
                .get(b)
                .and_then(|x| x.as_ref())
                .map_or(0.0, |blk| blk[o]),
            None => 0.0,
        }
    }

    fn block_mut(&mut self, b: usize) -> &mut Block {
        if self.blocks.is_empty() {
            self.blocks = (0..BLOCKS).map(|_| None).collect();
        }
        self.blocks[b].get_or_insert_with(new_block)
    }

    /// Store `v` at `i`; ignored when out of range. Allocates the block.
    pub fn set(&mut self, i: f64, v: f64) {
        if let Some((b, o)) = Self::slot(i) {
            self.block_mut(b)[o] = v;
        }
    }

    /// `freembuf(n)`: drop every block that starts at or above slot `n`
    /// (`n` truncated, negative clamped to 0; `n >= 8388608` frees nothing).
    /// ns-eel2 only records this and frees when the host calls
    /// `NSEEL_VM_freeRAMIfCodeRequested`; this frees at once.
    pub fn free(&mut self, above_index: f64) {
        let d = i64::from(eel_int(above_index).max(0));
        if d >= MEGABUF_SIZE {
            return;
        }
        for (x, blk) in self.blocks.iter_mut().enumerate() {
            if (x * ITEMS_PER_BLOCK) as i64 >= d {
                *blk = None;
            }
        }
    }

    /// Drop all blocks (`NSEEL_VM_freeRAM`).
    pub fn clear(&mut self) {
        self.blocks.iter_mut().for_each(|b| *b = None);
    }

    /// Number of allocated blocks.
    pub fn allocated_blocks(&self) -> usize {
        self.blocks.iter().filter(|b| b.is_some()).count()
    }

    /// `memcpy(dst, src, len)`, as `__NSEEL_RAM_MemCpy`: arguments get +0.0001
    /// and truncate; a negative start trims the front of both ranges; copying
    /// proceeds in chunks that never cross a block boundary (each chunk is a
    /// `memmove`) and stops at 8388608 on either side.
    pub fn copy(&mut self, dst: f64, src: f64, len: f64) {
        let mut d = i64::from(eel_int(dst + BULK_EPS));
        let mut s = i64::from(eel_int(src + BULK_EPS));
        let mut n = i64::from(eel_int(len + BULK_EPS));
        if s < 0 {
            n += s;
            d -= s;
            s = 0;
        }
        if d < 0 {
            n += d;
            s -= d;
            d = 0;
        }
        let bs = ITEMS_PER_BLOCK as i64;
        while n > 0 {
            if d >= MEGABUF_SIZE || s >= MEGABUF_SIZE {
                break;
            }
            let chunk = n.min(bs - d % bs).min(bs - s % bs);
            if chunk < 1 {
                break;
            }
            self.copy_chunk(d as usize, s as usize, chunk as usize);
            s += chunk;
            d += chunk;
            n -= chunk;
        }
    }

    fn copy_chunk(&mut self, d: usize, s: usize, n: usize) {
        let (sb, so) = (s / ITEMS_PER_BLOCK, s % ITEMS_PER_BLOCK);
        let (db, dof) = (d / ITEMS_PER_BLOCK, d % ITEMS_PER_BLOCK);
        let tmp: Vec<f64> = match self.blocks.get(sb).and_then(|x| x.as_ref()) {
            Some(blk) => blk[so..so + n].to_vec(),
            // Unallocated source reads as zeros; skip allocating the source
            // (unobservable) and only touch an already-allocated destination.
            None => {
                if let Some(Some(blk)) = self.blocks.get_mut(db) {
                    blk[dof..dof + n].fill(0.0);
                }
                return;
            }
        };
        self.block_mut(db)[dof..dof + n].copy_from_slice(&tmp);
    }

    /// `memset(dst, v, len)`, as `__NSEEL_RAM_MemSet`: `dst` and `len` get
    /// +0.0001 and truncate; a negative start trims `len`; the range is
    /// clamped to 8388608. Allocates every block it writes.
    pub fn fill(&mut self, dst: f64, v: f64, len: f64) {
        let mut o = i64::from(eel_int(dst + BULK_EPS));
        let mut n = i64::from(eel_int(len + BULK_EPS));
        if o < 0 {
            n += o;
            o = 0;
        }
        if o >= MEGABUF_SIZE {
            return;
        }
        if o + n > MEGABUF_SIZE {
            n = MEGABUF_SIZE - o;
        }
        let bs = ITEMS_PER_BLOCK as i64;
        while n > 0 {
            let cnt = (bs - o % bs).min(n);
            let off = (o % bs) as usize;
            self.block_mut((o / bs) as usize)[off..off + cnt as usize].fill(v);
            o += cnt;
            n -= cnt;
        }
    }
}

/// `gmegabuf`: one flat, wrapping 1,048,576-slot buffer (see module docs).
/// Allocated on first write; reads before that return 0.
#[derive(Default)]
pub struct GramBuf {
    data: Vec<f64>,
}

impl GramBuf {
    fn slot(i: f64) -> usize {
        (eel_int(i + INDEX_EPS) as u32 as usize) & (GRAM_SIZE - 1)
    }

    pub fn get(&self, i: f64) -> f64 {
        if self.data.is_empty() {
            0.0
        } else {
            self.data[Self::slot(i)]
        }
    }

    pub fn set(&mut self, i: f64, v: f64) {
        if self.data.is_empty() {
            self.data = vec![0.0; GRAM_SIZE];
        }
        self.data[Self::slot(i)] = v;
    }
}

/// State shared by every `Vm`: `reg00..reg99` and `gmegabuf`.
pub struct GlobalMem {
    pub regs: [f64; 100],
    pub gmegabuf: GramBuf,
}

impl GlobalMem {
    pub fn new() -> GlobalMem {
        GlobalMem {
            regs: [0.0; 100],
            gmegabuf: GramBuf::default(),
        }
    }
}

impl Default for GlobalMem {
    fn default() -> Self {
        Self::new()
    }
}
