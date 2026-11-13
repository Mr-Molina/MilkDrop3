//! C cast semantics shared by the draw builders.

/// C's `(int)` on a `double` under MSVC (x87 chop): truncate toward zero;
/// NaN and values that do not fit an `i32` give "integer indefinite",
/// `i32::MIN`. Mirrors md-eval's `eel_int` (md-render does not depend on
/// md-eval); a saturating Rust cast would differ.
pub(crate) fn c_int(x: f64) -> i32 {
    let t = x.trunc();
    if (-2147483648.0..=2147483647.0).contains(&t) {
        t as i32
    } else {
        i32::MIN
    }
}

/// `((int)(c * 255 * mult)) & 0xFF`, the colour byte of the shapes
/// (milkdropfs.cpp:2304-2313) and custom waves (2615-2619); the product is
/// taken in `double`, as the C++'s `double * int * float` is.
pub(crate) fn color_byte(c: f64, mult: f32) -> u8 {
    (c_int(c * 255.0 * f64::from(mult)) & 0xFF) as u8
}

/// `D3DCOLOR_RGBA_01(r, g, b, a)` (milkdropfs.cpp:40) on `float` arguments:
/// `((int)(x * 255)) & 0xFF` per channel, the product taken in `float`,
/// the cast as [`c_int`]. Returned as R, G, B, A bytes.
pub(crate) fn rgba_01_f32(r: f32, g: f32, b: f32, a: f32) -> [u8; 4] {
    [r, g, b, a].map(|x| (c_int(f64::from(x * 255.0)) & 0xFF) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_int_truncates_and_gives_int_min_out_of_range() {
        assert_eq!(c_int(3.9), 3);
        assert_eq!(c_int(-3.9), -3);
        assert_eq!(c_int(f64::NAN), i32::MIN);
        assert_eq!(c_int(3e9), i32::MIN);
        assert_eq!(c_int(-3e9), i32::MIN);
        assert_eq!(c_int(f64::INFINITY), i32::MIN);
    }
}
