//! MT19937 (Matsumoto and Nishimura), written from the published algorithm.
//!
//! ns-eel2 keeps one process-wide generator (`genrand_int32` in
//! `nseel-cfunc.c`) seeded on first use with `init_genrand(0x4141f00d)`;
//! `rand()` therefore produces the same stream in every run. md-eval keeps
//! that generator in `GlobalMem` so every `Vm` shares it, as the C static is
//! shared (ledger ruling F12: a fixed-seed MT19937 is allowed here).

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// Seed ns-eel2 uses for its static generator.
pub const NSEEL_SEED: u32 = 0x4141_f00d;

#[derive(Clone)]
pub struct Mt19937 {
    mt: [u32; N],
    idx: usize,
}

impl Mt19937 {
    /// `init_genrand(seed)`.
    pub fn seeded(seed: u32) -> Self {
        let mut mt = [0u32; N];
        mt[0] = seed;
        for i in 1..N {
            let p = mt[i - 1];
            mt[i] = 1_812_433_253u32
                .wrapping_mul(p ^ (p >> 30))
                .wrapping_add(i as u32);
        }
        Mt19937 { mt, idx: N }
    }

    fn twist(&mut self) {
        for k in 0..N {
            let y = (self.mt[k] & UPPER_MASK) | (self.mt[(k + 1) % N] & LOWER_MASK);
            let mag = if y & 1 == 1 { MATRIX_A } else { 0 };
            self.mt[k] = self.mt[(k + M) % N] ^ (y >> 1) ^ mag;
        }
        self.idx = 0;
    }

    /// `genrand_int32()`.
    pub fn next_u32(&mut self) -> u32 {
        if self.idx >= N {
            self.twist();
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `nseel_int_rand(x)`: `floor(x)` clamped to at least 1 (a NaN argument
    /// stays NaN, since `NaN < 1.0` is false), then
    /// `genrand_int32() * (1.0 / 4294967295.0) * x`, so the result lies in
    /// the CLOSED interval `[0, x]`.
    pub fn rand(&mut self, x: f64) -> f64 {
        let mut x = x.floor();
        // C: `if (x < 1.0) x = 1.0;` NaN compares false and stays NaN.
        if x < 1.0 {
            x = 1.0;
        }
        f64::from(self.next_u32()) * (1.0 / 4_294_967_295.0) * x
    }
}

impl Default for Mt19937 {
    fn default() -> Self {
        Mt19937::seeded(NSEEL_SEED)
    }
}
