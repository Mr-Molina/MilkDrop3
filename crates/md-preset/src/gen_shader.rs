//! Default warp/comp shader text for presets without `warp_`/`comp_` lines,
//! ported from `CPlugin::GenWarpPShaderText` / `GenCompPShaderText`
//! (code/vis_milk2/plugin.cpp:8782-8850@6f7cd5d), ledger ruling F19.
//!
//! The C++ copies `data/warp_ps.fx` / `data/comp_ps.fx`, finds the LAST `{`
//! and replaces everything after it. In both files that `{` is the one on
//! line 2 after `shader_body`, so the kept prefix is always
//! `"shader_body\n{"`. The text is returned in the same form as
//! [`crate::Ini::code_block`]: lines joined with `'\n'`, no trailing
//! separator (C++ ends every line, including the last, with
//! `LINEFEED_CONTROL_CHAR`).

use std::fmt::Write;

/// Everything up to and including the last `{` of warp_ps.fx / comp_ps.fx.
const SHADER_HEAD: &str = "shader_body\n{";

/// `GenWarpPShaderText(decay, bWrap)`.
///
/// Divergence: the C++ passes `bWrap ? L"" : L"_fc"` (wide literals) to a
/// narrow `%s`, which prints only `_` and yields `sampler__main` for
/// `bTexWrap=0` -- a sampler that does not exist, so that shader fails to
/// compile. This port emits the intended `sampler_fc_main` (as MilkDrop 2.25
/// did before the wide-string change; tests/presets/ps2x/upgraded_echo.milk
/// was authored the same way).
pub fn gen_warp_shader(decay: f32, wrap: bool) -> String {
    let mut s = String::from(SHADER_HEAD);
    let sampler = if wrap { "" } else { "_fc" };
    let _ = write!(
        s,
        "\n    // sample previous frame\
         \n    ret = tex2D( sampler{sampler}_main, uv ).xyz;\
         \n    \
         \n    // darken (decay) over time\
         \n    ret *= {:.2}; //or try: ret -= 0.004;\
         \n}}",
        f64::from(decay)
    );
    s
}

/// Inputs of `GenCompPShaderText` (one struct instead of nine arguments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompShaderParams {
    /// `fGammaAdj` (`brightness`).
    pub gamma: f32,
    /// `fVideoEchoAlpha`.
    pub echo_alpha: f32,
    /// `fVideoEchoZoom`.
    pub echo_zoom: f32,
    /// `nVideoEchoOrientation`.
    pub echo_orient: i32,
    /// `fShader` (`hue_shader`).
    pub hue_shader: f32,
    /// `bBrighten`.
    pub brighten: bool,
    /// `bDarken`.
    pub darken: bool,
    /// `bSolarize`.
    pub solarize: bool,
    /// `bInvert`.
    pub invert: bool,
}

/// `GenCompPShaderText(brightness, ve_alpha, ve_zoom, ve_orient, hue_shader,
/// bBrighten, bDarken, bSolarize, bInvert)`.
pub fn gen_comp_shader(p: &CompShaderParams) -> String {
    let mut s = String::from(SHADER_HEAD);
    // C++ formats the float args promoted to double; f32 -> f64 is exact.
    let d = f64::from;
    if p.echo_alpha > 0.001 {
        let orient_x = if p.echo_orient % 2 != 0 { -1 } else { 1 };
        let orient_y = if p.echo_orient >= 2 { -1 } else { 1 };
        let inv_zoom = 1.0f32 / p.echo_zoom;
        let _ = write!(
            s,
            "\n    float2 uv_echo = (uv - 0.5)*{:.3}*float2({orient_x},{orient_y}) + 0.5;\
             \n    ret = lerp( tex2D(sampler_main, uv).xyz, \
             \n                tex2D(sampler_main, uv_echo).xyz, \
             \n                {:.2} \
             \n              ); //video echo\
             \n    ret *= {:.2}; //gamma",
            d(inv_zoom),
            d(p.echo_alpha),
            d(p.gamma)
        );
    } else {
        let _ = write!(
            s,
            "\n    ret = tex2D(sampler_main, uv).xyz;\
             \n    ret *= {:.2}; //gamma",
            d(p.gamma)
        );
    }
    if p.hue_shader >= 1.0 {
        s.push_str("\n    ret *= hue_shader; //old hue shader effect");
    } else if p.hue_shader > 0.001 {
        let _ = write!(
            s,
            "\n    ret *= {:.2} + {:.2}*hue_shader; //old hue shader effect",
            d(1.0 - p.hue_shader),
            d(p.hue_shader)
        );
    }
    if p.brighten {
        s.push_str("\n    ret = sqrt(ret); //brighten");
    }
    if p.darken {
        s.push_str("\n    ret *= ret; //darken");
    }
    if p.solarize {
        s.push_str("\n    ret = ret*(1-ret)*4; //solarize");
    }
    if p.invert {
        s.push_str("\n    ret = 1 - ret; //invert");
    }
    s.push_str("\n}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(gamma: f32) -> CompShaderParams {
        CompShaderParams {
            gamma,
            echo_alpha: 0.0,
            echo_zoom: 2.0,
            echo_orient: 0,
            hue_shader: 0.0,
            brighten: false,
            darken: false,
            solarize: false,
            invert: false,
        }
    }

    #[test]
    fn warp_text_is_exact() {
        assert_eq!(
            gen_warp_shader(0.97, true),
            "shader_body\n{\n    // sample previous frame\n    ret = tex2D( sampler_main, uv ).xyz;\n    \n    // darken (decay) over time\n    ret *= 0.97; //or try: ret -= 0.004;\n}"
        );
    }

    #[test]
    fn warp_without_wrap_uses_clamped_sampler() {
        let t = gen_warp_shader(0.5, false);
        assert!(
            t.contains("    ret = tex2D( sampler_fc_main, uv ).xyz;\n"),
            "{t}"
        );
        assert!(t.contains("    ret *= 0.50; //or try"), "{t}");
    }

    #[test]
    fn comp_plain_text_is_exact() {
        assert_eq!(
            gen_comp_shader(&plain(1.56)),
            "shader_body\n{\n    ret = tex2D(sampler_main, uv).xyz;\n    ret *= 1.56; //gamma\n}"
        );
    }

    #[test]
    fn comp_echo_text_is_exact() {
        let p = CompShaderParams {
            echo_alpha: 0.5,
            echo_zoom: 1.33,
            echo_orient: 3,
            ..plain(2.0)
        };
        assert_eq!(
            gen_comp_shader(&p),
            "shader_body\n{\n    float2 uv_echo = (uv - 0.5)*0.752*float2(-1,-1) + 0.5;\n    ret = lerp( tex2D(sampler_main, uv).xyz, \n                tex2D(sampler_main, uv_echo).xyz, \n                0.50 \n              ); //video echo\n    ret *= 2.00; //gamma\n}"
        );
    }

    #[test]
    fn comp_echo_orientations() {
        for (o, xy) in [
            (0, "1,1"),
            (1, "-1,1"),
            (2, "1,-1"),
            (3, "-1,-1"),
            (-1, "-1,1"),
        ] {
            let p = CompShaderParams {
                echo_alpha: 0.2,
                echo_orient: o,
                ..plain(1.0)
            };
            assert!(
                gen_comp_shader(&p).contains(&format!("float2({xy})")),
                "orient {o}"
            );
        }
        // alpha at the 0.001 threshold is not an echo.
        let p = CompShaderParams {
            echo_alpha: 0.001,
            ..plain(1.0)
        };
        assert!(!gen_comp_shader(&p).contains("uv_echo"));
    }

    #[test]
    fn comp_hue_and_flags_in_order() {
        let p = CompShaderParams {
            hue_shader: 0.25,
            brighten: true,
            darken: true,
            solarize: true,
            invert: true,
            ..plain(1.0)
        };
        assert_eq!(
            gen_comp_shader(&p),
            "shader_body\n{\n    ret = tex2D(sampler_main, uv).xyz;\n    ret *= 1.00; //gamma\n    ret *= 0.75 + 0.25*hue_shader; //old hue shader effect\n    ret = sqrt(ret); //brighten\n    ret *= ret; //darken\n    ret = ret*(1-ret)*4; //solarize\n    ret = 1 - ret; //invert\n}"
        );
        let full = CompShaderParams {
            hue_shader: 1.0,
            ..plain(1.0)
        };
        assert!(
            gen_comp_shader(&full).contains("\n    ret *= hue_shader; //old hue shader effect\n")
        );
        let tiny = CompShaderParams {
            hue_shader: 0.001,
            ..plain(1.0)
        };
        assert!(!gen_comp_shader(&tiny).contains("hue_shader"));
    }
}
