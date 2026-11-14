//! Preset parser tests (Task 2.3). Fixtures: `tests/fixtures/all_keys.milk`
//! (crate-local; every scalar key with a distinct non-default value) and the
//! shared corpus under `<workspace>/tests/presets/`.

use md_preset::{
    gen_comp_shader, gen_warp_shader, parse_milk, CompShaderParams, Preset, PsVersion,
};

const ALL_KEYS: &str = include_str!("fixtures/all_keys.milk");

fn corpus(rel: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/presets")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn parse(text: &str) -> Preset {
    parse_milk(text).expect("parse_milk")
}

#[test]
fn defaults_match_cstate_default() {
    // Spot-check against CState::Default (state.cpp:515-702).
    let p = Preset::defaults();
    assert_eq!(p.decay.value(), 0.98); // :539
    assert_eq!(p.gamma_adj.value(), 2.0); // :540
    assert_eq!(p.zoom.value(), 1.0); // :656
    assert_eq!(p.wave_mode, 0); // :565
    assert_eq!(p.wave_alpha.value(), 0.8); // :570
    assert_eq!(p.wave_smoothing.value(), 0.75); // :572
    assert_eq!(p.mod_wave_alpha_end.value(), 0.95); // :576
    assert!(p.tex_wrap); // :547
    assert!(p.maximize_wave_color); // :582
    assert_eq!(p.mv_x.value(), 12.0); // :583
    assert_eq!(p.mv_l.value(), 0.9); // :587
    assert_eq!(p.ib_r.value(), 0.25); // :671
    assert_eq!(p.ob_size.value(), 0.01); // :665
    assert_eq!(p.blur1_edge_darken.value(), 0.25); // :558
    assert_eq!(p.rating, 3.0); // :538
    assert_eq!(p.waves[2].samples, 512); // :596
    assert_eq!(p.waves[3].smoothing, 0.5); // :599
    assert_eq!(p.shapes[1].sides, 4); // :612
    assert_eq!(p.shapes[0].instances, 1); // :616
    assert_eq!(p.shapes[3].g2, 1.0); // :628
    assert_eq!(p.shapes[2].border_a, 0.1); // :634
    assert_eq!(p.warp_ps_version(), PsVersion::None); // :689
    assert!(p.per_frame.is_empty() && p.warp_shader.is_empty());
    assert!(!p.transition.blending); // :533
    assert_eq!(p.transition.old_wave_mode, -1); // :566
}

#[test]
fn parses_all_scalar_keys() {
    let p = parse(ALL_KEYS);
    // MILKDROP_PRESET_VERSION=201 selects PSVERSION_WARP/COMP; PSVERSION=2
    // is ignored for version > 200.
    assert_eq!(p.warp_ps, 3, "PSVERSION_WARP");
    assert_eq!(p.comp_ps, 4, "PSVERSION_COMP");
    assert_eq!(p.warp_ps_version(), PsVersion::Ps2_X);
    assert_eq!(p.comp_ps_version(), PsVersion::Ps3_0);
    assert_eq!(p.rating, 0.1084_f32, "fRating");
    assert_eq!(p.gamma_adj.value(), 0.1155_f32, "fGammaAdj");
    assert_eq!(p.decay.value(), 0.1226_f32, "fDecay");
    assert_eq!(p.video_echo_zoom.value(), 0.1297_f32, "fVideoEchoZoom");
    assert_eq!(p.video_echo_alpha.value(), 0.1368_f32, "fVideoEchoAlpha");
    assert_eq!(p.video_echo_orientation, 11, "nVideoEchoOrientation");
    assert_eq!(p.wave_mode, 12, "nWaveMode");
    assert!(p.additive_waves, "bAdditiveWaves");
    assert!(p.wave_dots, "bWaveDots");
    assert!(p.wave_thick, "bWaveThick");
    assert!(p.mod_wave_alpha_by_volume, "bModWaveAlphaByVolume");
    assert!(!p.maximize_wave_color, "bMaximizeWaveColor");
    assert!(!p.tex_wrap, "bTexWrap");
    assert!(p.darken_center, "bDarkenCenter");
    assert!(p.red_blue_stereo, "bRedBlueStereo");
    assert!(p.brighten, "bBrighten");
    assert!(p.darken, "bDarken");
    assert!(p.solarize, "bSolarize");
    assert!(p.invert, "bInvert");
    assert_eq!(p.wave_alpha.value(), 0.1439_f32, "fWaveAlpha");
    assert_eq!(p.wave_scale.value(), 0.1510_f32, "fWaveScale");
    assert_eq!(p.wave_smoothing.value(), 0.1581_f32, "fWaveSmoothing");
    assert_eq!(p.wave_param.value(), 0.1652_f32, "fWaveParam");
    assert_eq!(
        p.mod_wave_alpha_start.value(),
        0.1723_f32,
        "fModWaveAlphaStart"
    );
    assert_eq!(p.mod_wave_alpha_end.value(), 0.1794_f32, "fModWaveAlphaEnd");
    assert_eq!(p.warp_anim_speed, 0.1865_f32, "fWarpAnimSpeed");
    assert_eq!(p.warp_scale.value(), 0.1936_f32, "fWarpScale");
    assert_eq!(p.zoom_exponent.value(), 0.2007_f32, "fZoomExponent");
    assert_eq!(p.shader.value(), 0.2078_f32, "fShader");
    assert_eq!(p.zoom.value(), 0.2149_f32, "zoom");
    assert_eq!(p.rot.value(), 0.2220_f32, "rot");
    assert_eq!(p.cx.value(), 0.2291_f32, "cx");
    assert_eq!(p.cy.value(), 0.2362_f32, "cy");
    assert_eq!(p.dx.value(), 0.2433_f32, "dx");
    assert_eq!(p.dy.value(), 0.2504_f32, "dy");
    assert_eq!(p.warp.value(), 0.2575_f32, "warp");
    assert_eq!(p.sx.value(), 0.2646_f32, "sx");
    assert_eq!(p.sy.value(), 0.2717_f32, "sy");
    assert_eq!(p.wave_r.value(), 0.2788_f32, "wave_r");
    assert_eq!(p.wave_g.value(), 0.2859_f32, "wave_g");
    assert_eq!(p.wave_b.value(), 0.2930_f32, "wave_b");
    assert_eq!(p.wave_x.value(), 0.3001_f32, "wave_x");
    assert_eq!(p.wave_y.value(), 0.3072_f32, "wave_y");
    assert_eq!(p.ob_size.value(), 0.3143_f32, "ob_size");
    assert_eq!(p.ob_r.value(), 0.3214_f32, "ob_r");
    assert_eq!(p.ob_g.value(), 0.3285_f32, "ob_g");
    assert_eq!(p.ob_b.value(), 0.3356_f32, "ob_b");
    assert_eq!(p.ob_a.value(), 0.3427_f32, "ob_a");
    assert_eq!(p.ib_size.value(), 0.3498_f32, "ib_size");
    assert_eq!(p.ib_r.value(), 0.3569_f32, "ib_r");
    assert_eq!(p.ib_g.value(), 0.3640_f32, "ib_g");
    assert_eq!(p.ib_b.value(), 0.3711_f32, "ib_b");
    assert_eq!(p.ib_a.value(), 0.3782_f32, "ib_a");
    assert_eq!(p.mv_x.value(), 0.3853_f32, "nMotionVectorsX");
    assert_eq!(p.mv_y.value(), 0.3924_f32, "nMotionVectorsY");
    assert_eq!(p.mv_dx.value(), 0.3995_f32, "mv_dx");
    assert_eq!(p.mv_dy.value(), 0.4066_f32, "mv_dy");
    assert_eq!(p.mv_l.value(), 0.4137_f32, "mv_l");
    assert_eq!(p.mv_r.value(), 0.4208_f32, "mv_r");
    assert_eq!(p.mv_g.value(), 0.4279_f32, "mv_g");
    assert_eq!(p.mv_b.value(), 0.4350_f32, "mv_b");
    assert_eq!(p.mv_a.value(), 0.4421_f32, "mv_a");
    assert_eq!(p.blur1_min.value(), 0.4492_f32, "b1n");
    assert_eq!(p.blur2_min.value(), 0.4563_f32, "b2n");
    assert_eq!(p.blur3_min.value(), 0.4634_f32, "b3n");
    assert_eq!(p.blur1_max.value(), 0.4705_f32, "b1x");
    assert_eq!(p.blur2_max.value(), 0.4776_f32, "b2x");
    assert_eq!(p.blur3_max.value(), 0.4847_f32, "b3x");
    assert_eq!(p.blur1_edge_darken.value(), 0.4918_f32, "b1ed");
    assert_eq!(p.waves[0].enabled, 13, "wavecode_0_enabled");
    assert_eq!(p.waves[0].samples, 14, "wavecode_0_samples");
    assert_eq!(p.waves[0].sep, 15, "wavecode_0_sep");
    assert_eq!(p.waves[0].spectrum, 16, "wavecode_0_bSpectrum");
    assert_eq!(p.waves[0].use_dots, 17, "wavecode_0_bUseDots");
    assert_eq!(p.waves[0].draw_thick, 18, "wavecode_0_bDrawThick");
    assert_eq!(p.waves[0].additive, 19, "wavecode_0_bAdditive");
    assert_eq!(p.waves[0].scaling, 0.4989_f32, "wavecode_0_scaling");
    assert_eq!(p.waves[0].smoothing, 0.5060_f32, "wavecode_0_smoothing");
    assert_eq!(p.waves[0].r, 0.5131_f32, "wavecode_0_r");
    assert_eq!(p.waves[0].g, 0.5202_f32, "wavecode_0_g");
    assert_eq!(p.waves[0].b, 0.5273_f32, "wavecode_0_b");
    assert_eq!(p.waves[0].a, 0.5344_f32, "wavecode_0_a");
    assert_eq!(p.waves[1].enabled, 20, "wavecode_1_enabled");
    assert_eq!(p.waves[1].samples, 21, "wavecode_1_samples");
    assert_eq!(p.waves[1].sep, 22, "wavecode_1_sep");
    assert_eq!(p.waves[1].spectrum, 23, "wavecode_1_bSpectrum");
    assert_eq!(p.waves[1].use_dots, 24, "wavecode_1_bUseDots");
    assert_eq!(p.waves[1].draw_thick, 25, "wavecode_1_bDrawThick");
    assert_eq!(p.waves[1].additive, 26, "wavecode_1_bAdditive");
    assert_eq!(p.waves[1].scaling, 0.5415_f32, "wavecode_1_scaling");
    assert_eq!(p.waves[1].smoothing, 0.5486_f32, "wavecode_1_smoothing");
    assert_eq!(p.waves[1].r, 0.5557_f32, "wavecode_1_r");
    assert_eq!(p.waves[1].g, 0.5628_f32, "wavecode_1_g");
    assert_eq!(p.waves[1].b, 0.5699_f32, "wavecode_1_b");
    assert_eq!(p.waves[1].a, 0.5770_f32, "wavecode_1_a");
    assert_eq!(p.waves[2].enabled, 27, "wavecode_2_enabled");
    assert_eq!(p.waves[2].samples, 28, "wavecode_2_samples");
    assert_eq!(p.waves[2].sep, 29, "wavecode_2_sep");
    assert_eq!(p.waves[2].spectrum, 30, "wavecode_2_bSpectrum");
    assert_eq!(p.waves[2].use_dots, 31, "wavecode_2_bUseDots");
    assert_eq!(p.waves[2].draw_thick, 32, "wavecode_2_bDrawThick");
    assert_eq!(p.waves[2].additive, 33, "wavecode_2_bAdditive");
    assert_eq!(p.waves[2].scaling, 0.5841_f32, "wavecode_2_scaling");
    assert_eq!(p.waves[2].smoothing, 0.5912_f32, "wavecode_2_smoothing");
    assert_eq!(p.waves[2].r, 0.5983_f32, "wavecode_2_r");
    assert_eq!(p.waves[2].g, 0.6054_f32, "wavecode_2_g");
    assert_eq!(p.waves[2].b, 0.6125_f32, "wavecode_2_b");
    assert_eq!(p.waves[2].a, 0.6196_f32, "wavecode_2_a");
    assert_eq!(p.waves[3].enabled, 34, "wavecode_3_enabled");
    assert_eq!(p.waves[3].samples, 35, "wavecode_3_samples");
    assert_eq!(p.waves[3].sep, 36, "wavecode_3_sep");
    assert_eq!(p.waves[3].spectrum, 37, "wavecode_3_bSpectrum");
    assert_eq!(p.waves[3].use_dots, 38, "wavecode_3_bUseDots");
    assert_eq!(p.waves[3].draw_thick, 39, "wavecode_3_bDrawThick");
    assert_eq!(p.waves[3].additive, 40, "wavecode_3_bAdditive");
    assert_eq!(p.waves[3].scaling, 0.6267_f32, "wavecode_3_scaling");
    assert_eq!(p.waves[3].smoothing, 0.6338_f32, "wavecode_3_smoothing");
    assert_eq!(p.waves[3].r, 0.6409_f32, "wavecode_3_r");
    assert_eq!(p.waves[3].g, 0.6480_f32, "wavecode_3_g");
    assert_eq!(p.waves[3].b, 0.6551_f32, "wavecode_3_b");
    assert_eq!(p.waves[3].a, 0.6622_f32, "wavecode_3_a");
    assert_eq!(p.shapes[0].enabled, 41, "shapecode_0_enabled");
    assert_eq!(p.shapes[0].sides, 42, "shapecode_0_sides");
    assert_eq!(p.shapes[0].additive, 43, "shapecode_0_additive");
    assert_eq!(p.shapes[0].thick_outline, 44, "shapecode_0_thickOutline");
    assert_eq!(p.shapes[0].textured, 45, "shapecode_0_textured");
    assert_eq!(p.shapes[0].instances, 46, "shapecode_0_num_inst");
    assert_eq!(p.shapes[0].x, 0.6693_f32, "shapecode_0_x");
    assert_eq!(p.shapes[0].y, 0.6764_f32, "shapecode_0_y");
    assert_eq!(p.shapes[0].rad, 0.6835_f32, "shapecode_0_rad");
    assert_eq!(p.shapes[0].ang, 0.6906_f32, "shapecode_0_ang");
    assert_eq!(p.shapes[0].tex_ang, 0.6977_f32, "shapecode_0_tex_ang");
    assert_eq!(p.shapes[0].tex_zoom, 0.7048_f32, "shapecode_0_tex_zoom");
    assert_eq!(p.shapes[0].r, 0.7119_f32, "shapecode_0_r");
    assert_eq!(p.shapes[0].g, 0.7190_f32, "shapecode_0_g");
    assert_eq!(p.shapes[0].b, 0.7261_f32, "shapecode_0_b");
    assert_eq!(p.shapes[0].a, 0.7332_f32, "shapecode_0_a");
    assert_eq!(p.shapes[0].r2, 0.7403_f32, "shapecode_0_r2");
    assert_eq!(p.shapes[0].g2, 0.7474_f32, "shapecode_0_g2");
    assert_eq!(p.shapes[0].b2, 0.7545_f32, "shapecode_0_b2");
    assert_eq!(p.shapes[0].a2, 0.7616_f32, "shapecode_0_a2");
    assert_eq!(p.shapes[0].border_r, 0.7687_f32, "shapecode_0_border_r");
    assert_eq!(p.shapes[0].border_g, 0.7758_f32, "shapecode_0_border_g");
    assert_eq!(p.shapes[0].border_b, 0.7829_f32, "shapecode_0_border_b");
    assert_eq!(p.shapes[0].border_a, 0.7900_f32, "shapecode_0_border_a");
    assert_eq!(p.shapes[1].enabled, 47, "shapecode_1_enabled");
    assert_eq!(p.shapes[1].sides, 48, "shapecode_1_sides");
    assert_eq!(p.shapes[1].additive, 49, "shapecode_1_additive");
    assert_eq!(p.shapes[1].thick_outline, 50, "shapecode_1_thickOutline");
    assert_eq!(p.shapes[1].textured, 51, "shapecode_1_textured");
    assert_eq!(p.shapes[1].instances, 52, "shapecode_1_num_inst");
    assert_eq!(p.shapes[1].x, 0.7971_f32, "shapecode_1_x");
    assert_eq!(p.shapes[1].y, 0.8042_f32, "shapecode_1_y");
    assert_eq!(p.shapes[1].rad, 0.8113_f32, "shapecode_1_rad");
    assert_eq!(p.shapes[1].ang, 0.8184_f32, "shapecode_1_ang");
    assert_eq!(p.shapes[1].tex_ang, 0.8255_f32, "shapecode_1_tex_ang");
    assert_eq!(p.shapes[1].tex_zoom, 0.8326_f32, "shapecode_1_tex_zoom");
    assert_eq!(p.shapes[1].r, 0.8397_f32, "shapecode_1_r");
    assert_eq!(p.shapes[1].g, 0.8468_f32, "shapecode_1_g");
    assert_eq!(p.shapes[1].b, 0.8539_f32, "shapecode_1_b");
    assert_eq!(p.shapes[1].a, 0.8610_f32, "shapecode_1_a");
    assert_eq!(p.shapes[1].r2, 0.8681_f32, "shapecode_1_r2");
    assert_eq!(p.shapes[1].g2, 0.8752_f32, "shapecode_1_g2");
    assert_eq!(p.shapes[1].b2, 0.8823_f32, "shapecode_1_b2");
    assert_eq!(p.shapes[1].a2, 0.8894_f32, "shapecode_1_a2");
    assert_eq!(p.shapes[1].border_r, 0.8965_f32, "shapecode_1_border_r");
    assert_eq!(p.shapes[1].border_g, 0.9036_f32, "shapecode_1_border_g");
    assert_eq!(p.shapes[1].border_b, 0.9107_f32, "shapecode_1_border_b");
    assert_eq!(p.shapes[1].border_a, 0.9178_f32, "shapecode_1_border_a");
    assert_eq!(p.shapes[2].enabled, 53, "shapecode_2_enabled");
    assert_eq!(p.shapes[2].sides, 54, "shapecode_2_sides");
    assert_eq!(p.shapes[2].additive, 55, "shapecode_2_additive");
    assert_eq!(p.shapes[2].thick_outline, 56, "shapecode_2_thickOutline");
    assert_eq!(p.shapes[2].textured, 57, "shapecode_2_textured");
    assert_eq!(p.shapes[2].instances, 58, "shapecode_2_num_inst");
    assert_eq!(p.shapes[2].x, 0.9249_f32, "shapecode_2_x");
    assert_eq!(p.shapes[2].y, 0.9320_f32, "shapecode_2_y");
    assert_eq!(p.shapes[2].rad, 0.9391_f32, "shapecode_2_rad");
    assert_eq!(p.shapes[2].ang, 0.9462_f32, "shapecode_2_ang");
    assert_eq!(p.shapes[2].tex_ang, 0.9533_f32, "shapecode_2_tex_ang");
    assert_eq!(p.shapes[2].tex_zoom, 0.9604_f32, "shapecode_2_tex_zoom");
    assert_eq!(p.shapes[2].r, 0.9675_f32, "shapecode_2_r");
    assert_eq!(p.shapes[2].g, 0.9746_f32, "shapecode_2_g");
    assert_eq!(p.shapes[2].b, 0.9817_f32, "shapecode_2_b");
    assert_eq!(p.shapes[2].a, 0.9888_f32, "shapecode_2_a");
    assert_eq!(p.shapes[2].r2, 0.9959_f32, "shapecode_2_r2");
    assert_eq!(p.shapes[2].g2, 1.0030_f32, "shapecode_2_g2");
    assert_eq!(p.shapes[2].b2, 1.0101_f32, "shapecode_2_b2");
    assert_eq!(p.shapes[2].a2, 1.0172_f32, "shapecode_2_a2");
    assert_eq!(p.shapes[2].border_r, 1.0243_f32, "shapecode_2_border_r");
    assert_eq!(p.shapes[2].border_g, 1.0314_f32, "shapecode_2_border_g");
    assert_eq!(p.shapes[2].border_b, 1.0385_f32, "shapecode_2_border_b");
    assert_eq!(p.shapes[2].border_a, 1.0456_f32, "shapecode_2_border_a");
    assert_eq!(p.shapes[3].enabled, 59, "shapecode_3_enabled");
    assert_eq!(p.shapes[3].sides, 60, "shapecode_3_sides");
    assert_eq!(p.shapes[3].additive, 61, "shapecode_3_additive");
    assert_eq!(p.shapes[3].thick_outline, 62, "shapecode_3_thickOutline");
    assert_eq!(p.shapes[3].textured, 63, "shapecode_3_textured");
    assert_eq!(p.shapes[3].instances, 64, "shapecode_3_num_inst");
    assert_eq!(p.shapes[3].x, 1.0527_f32, "shapecode_3_x");
    assert_eq!(p.shapes[3].y, 1.0598_f32, "shapecode_3_y");
    assert_eq!(p.shapes[3].rad, 1.0669_f32, "shapecode_3_rad");
    assert_eq!(p.shapes[3].ang, 1.0740_f32, "shapecode_3_ang");
    assert_eq!(p.shapes[3].tex_ang, 1.0811_f32, "shapecode_3_tex_ang");
    assert_eq!(p.shapes[3].tex_zoom, 1.0882_f32, "shapecode_3_tex_zoom");
    assert_eq!(p.shapes[3].r, 1.0953_f32, "shapecode_3_r");
    assert_eq!(p.shapes[3].g, 1.1024_f32, "shapecode_3_g");
    assert_eq!(p.shapes[3].b, 1.1095_f32, "shapecode_3_b");
    assert_eq!(p.shapes[3].a, 1.1166_f32, "shapecode_3_a");
    assert_eq!(p.shapes[3].r2, 1.1237_f32, "shapecode_3_r2");
    assert_eq!(p.shapes[3].g2, 1.1308_f32, "shapecode_3_g2");
    assert_eq!(p.shapes[3].b2, 1.1379_f32, "shapecode_3_b2");
    assert_eq!(p.shapes[3].a2, 1.1450_f32, "shapecode_3_a2");
    assert_eq!(p.shapes[3].border_r, 1.1521_f32, "shapecode_3_border_r");
    assert_eq!(p.shapes[3].border_g, 1.1592_f32, "shapecode_3_border_g");
    assert_eq!(p.shapes[3].border_b, 1.1663_f32, "shapecode_3_border_b");
    assert_eq!(p.shapes[3].border_a, 1.1734_f32, "shapecode_3_border_a");
    assert_eq!(
        p.per_frame_init,
        "per_frame_init_l1 = 1;\nper_frame_init_l2 = 2;"
    );
    assert_eq!(
        p.per_frame,
        "per_frame_l1 = 1;\nper_frame_l2 = 2;\nper_frame_l3 = 3;"
    );
    assert_eq!(p.per_pixel, "per_pixel_l1 = 1;\nper_pixel_l2 = 2;");
    assert_eq!(p.waves[0].init, "wave_0_init_l1 = 1;");
    assert_eq!(
        p.waves[0].per_frame,
        "wave_0_per_frame_l1 = 1;\nwave_0_per_frame_l2 = 2;"
    );
    assert_eq!(
        p.waves[0].per_point,
        "wave_0_per_point_l1 = 1;\nwave_0_per_point_l2 = 2;\nwave_0_per_point_l3 = 3;"
    );
    assert_eq!(p.waves[1].init, "wave_1_init_l1 = 1;");
    assert_eq!(
        p.waves[1].per_frame,
        "wave_1_per_frame_l1 = 1;\nwave_1_per_frame_l2 = 2;"
    );
    assert_eq!(
        p.waves[1].per_point,
        "wave_1_per_point_l1 = 1;\nwave_1_per_point_l2 = 2;\nwave_1_per_point_l3 = 3;"
    );
    assert_eq!(p.waves[2].init, "wave_2_init_l1 = 1;");
    assert_eq!(
        p.waves[2].per_frame,
        "wave_2_per_frame_l1 = 1;\nwave_2_per_frame_l2 = 2;"
    );
    assert_eq!(
        p.waves[2].per_point,
        "wave_2_per_point_l1 = 1;\nwave_2_per_point_l2 = 2;\nwave_2_per_point_l3 = 3;"
    );
    assert_eq!(p.waves[3].init, "wave_3_init_l1 = 1;");
    assert_eq!(
        p.waves[3].per_frame,
        "wave_3_per_frame_l1 = 1;\nwave_3_per_frame_l2 = 2;"
    );
    assert_eq!(
        p.waves[3].per_point,
        "wave_3_per_point_l1 = 1;\nwave_3_per_point_l2 = 2;\nwave_3_per_point_l3 = 3;"
    );
    assert_eq!(
        p.shapes[0].init,
        "shape_0_init_l1 = 1;\nshape_0_init_l2 = 2;"
    );
    assert_eq!(p.shapes[0].per_frame, "shape_0_per_frame_l1 = 1;");
    assert_eq!(
        p.shapes[1].init,
        "shape_1_init_l1 = 1;\nshape_1_init_l2 = 2;"
    );
    assert_eq!(p.shapes[1].per_frame, "shape_1_per_frame_l1 = 1;");
    assert_eq!(
        p.shapes[2].init,
        "shape_2_init_l1 = 1;\nshape_2_init_l2 = 2;"
    );
    assert_eq!(p.shapes[2].per_frame, "shape_2_per_frame_l1 = 1;");
    assert_eq!(
        p.shapes[3].init,
        "shape_3_init_l1 = 1;\nshape_3_init_l2 = 2;"
    );
    assert_eq!(p.shapes[3].per_frame, "shape_3_per_frame_l1 = 1;");
    assert_eq!(
        p.warp_shader,
        "shader_body\n{\n    ret = tex2D(sampler_main, uv).xyz * 0.5;\n}"
    );
    assert_eq!(
        p.comp_shader,
        "shader_body\n{\n    ret = 1 - tex2D(sampler_main, uv).xyz;\n}"
    );
}

#[test]
fn parses_four_waves_and_shapes() {
    let p = parse(&corpus("ps3/four_waves.milk"));
    assert!(p.waves.iter().all(|w| w.enabled == 1));
    assert!(p.shapes.iter().all(|s| s.enabled == 0));
    assert_eq!(p.waves[0].spectrum, 1);
    assert_eq!(p.waves[0].scaling, 2.0);
    assert_eq!(p.waves[0].g, 0.2);
    assert_eq!(p.waves[0].per_point, "x = sample;\ny = 0.1 + value1*0.3;");
    assert_eq!(
        p.waves[2].per_frame,
        "t1 = time; t2 = bass_att*0.1; t8 = 0.25;"
    );
    assert_eq!(p.waves[3].per_point.lines().count(), 3);

    let p = parse(&corpus("ps2x/four_shapes.milk"));
    assert!(p.shapes.iter().all(|s| s.enabled == 1));
    assert!(p.waves.iter().all(|w| w.enabled == 0));
    assert_eq!(p.shapes[0].per_frame.lines().count(), 4);
    assert_eq!(p.shapes[3].init, "t1 = rand(1000)*0.001;");
    assert_eq!(
        p.shapes[2].per_frame,
        "tex_ang = 0.01*sin(time);\nang = 0.785398;"
    );

    // Every index is distinct in the all-keys fixture (no aliasing).
    let p = parse(ALL_KEYS);
    for i in 0..4 {
        for j in (i + 1)..4 {
            assert_ne!(p.waves[i], p.waves[j]);
            assert_ne!(p.shapes[i], p.shapes[j]);
        }
    }
}

fn versions(text: &str) -> (i32, i32) {
    let p = parse(text);
    (p.warp_ps, p.comp_ps)
}

#[test]
fn ps_version_precedence() {
    // No MILKDROP_PRESET_VERSION -> 100 -> no shaders, whatever else is set.
    assert_eq!(
        versions("PSVERSION=3\nPSVERSION_WARP=4\nPSVERSION_COMP=4\n"),
        (0, 0)
    );
    assert_eq!(
        versions("MILKDROP_PRESET_VERSION=199\nPSVERSION=3\n"),
        (0, 0)
    );
    // == 200: PSVERSION (default 2) for both; the split keys are ignored.
    assert_eq!(
        versions("MILKDROP_PRESET_VERSION=200\nPSVERSION=3\nPSVERSION_WARP=4\nPSVERSION_COMP=2\n"),
        (3, 3)
    );
    assert_eq!(versions("MILKDROP_PRESET_VERSION=200\n"), (2, 2));
    // > 200: PSVERSION_WARP / PSVERSION_COMP (each default 2); PSVERSION ignored.
    assert_eq!(
        versions("MILKDROP_PRESET_VERSION=201\nPSVERSION=4\nPSVERSION_WARP=3\n"),
        (3, 2)
    );
    assert_eq!(
        versions("MILKDROP_PRESET_VERSION=201\nPSVERSION=4\n"),
        (2, 2)
    );
    assert_eq!(
        versions("MILKDROP_PRESET_VERSION=250\nPSVERSION_COMP=0\n"),
        (2, 0)
    );
    // The <200 rule wins even with explicit shader text.
    let p = parse("MILKDROP_PRESET_VERSION=100\nPSVERSION_WARP=2\nwarp_1=`shader_body\n");
    assert_eq!(p.warp_ps_version(), PsVersion::None);
    assert_eq!(p.warp_shader, "shader_body");

    // Corpus: version 200 with PSVERSION only; 201 with differing split keys.
    let v200 = parse(&corpus("ps2/version200.milk"));
    assert_eq!(
        (v200.warp_ps_version(), v200.comp_ps_version()),
        (PsVersion::Ps2_0, PsVersion::Ps2_0)
    );
    let mixed = parse(&corpus("ps3/mixed_versions.milk"));
    assert_eq!((mixed.warp_ps, mixed.comp_ps), (2, 4));
    assert_eq!(
        (mixed.warp_ps_version(), mixed.comp_ps_version()),
        (PsVersion::Ps2_0, PsVersion::Ps3_0)
    );
    let classic = parse(&corpus("classic/drift_tunnel.milk"));
    assert_eq!((classic.warp_ps, classic.comp_ps), (0, 0));

    // Enum mapping (MD2_PS_*, plugin.h:94-98).
    let map: Vec<PsVersion> = [-1, 0, 1, 2, 3, 4, 5, 9]
        .map(PsVersion::from_file_value)
        .to_vec();
    use PsVersion::*;
    assert_eq!(map, [None, None, Ps2_0, Ps2_0, Ps2_X, Ps3_0, Ps4_0, Ps4_0]);
}

#[test]
fn parses_crlf_bom_and_trailing_junk() {
    let lf = parse(ALL_KEYS);
    let crlf = format!(
        "\u{feff}{}\r\n; trailing junk after the preset\r\nthanks for watching\r\n\u{1a}",
        ALL_KEYS.replace('\n', "\r\n")
    );
    assert_eq!(parse(&crlf), lf);
    // The corpus edge file, against itself normalised to LF without BOM/junk.
    let edge = corpus("edge/crlf_bom_junk.milk");
    assert!(edge.starts_with('\u{feff}') && edge.contains("\r\n"));
    let clean = edge.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    // Drop the trailing junk: everything after the first blank line.
    let clean = &clean[..clean.find("\n\n").expect("blank line before junk") + 1];
    assert!(!clean.contains("thanks"));
    let a = parse(&edge);
    assert_eq!(a, parse(clean));
    assert_eq!(a.decay.value(), 0.97);
    assert_eq!(a.warp_ps_version(), PsVersion::Ps2_0);
}

#[test]
fn missing_keys_keep_defaults() {
    let d = Preset::defaults();
    let p = parse("[preset00]\n");
    let mut expected = d.clone();
    // C++ quirks (state.cpp:1368-1372, 1381): wave_r..wave_y fall back to the
    // default rot (0.0) and mv_a to bMotionVectorsOn (absent -> 0.0).
    for f in [
        &mut expected.wave_r,
        &mut expected.wave_g,
        &mut expected.wave_b,
        &mut expected.wave_x,
        &mut expected.wave_y,
    ] {
        f.set(0.0);
    }
    expected.mv_a.set(0.0);
    // Empty shader sections are generated from the (default) values.
    expected.warp_shader = gen_warp_shader(0.98, true);
    expected.comp_shader = gen_comp_shader(&CompShaderParams {
        gamma: 2.0,
        echo_alpha: 0.0,
        echo_zoom: 2.0,
        echo_orient: 0,
        hue_shader: 0.0,
        brighten: false,
        darken: false,
        solarize: false,
        invert: false,
    });
    assert_eq!(p, expected);

    // A partial file changes only what it names.
    let p = parse("[preset00]\nzoom=1.5\nwavecode_2_samples=100\nshapecode_1_sides=7\n");
    assert_eq!(p.zoom.value(), 1.5);
    assert_eq!(p.rot.value(), d.rot.value());
    assert_eq!(p.waves[2].samples, 100);
    assert_eq!(p.waves[2].smoothing, d.waves[2].smoothing);
    assert_eq!(p.waves[1], d.waves[1]);
    assert_eq!(p.shapes[1].sides, 7);
    assert_eq!(p.shapes[1].rad, d.shapes[1].rad);
    assert_eq!(p.decay.value(), d.decay.value());
    // Unparsable values keep the fallback (sscanf failure).
    let p = parse("fDecay=abc\nnWaveMode=x\n");
    assert_eq!((p.decay.value(), p.wave_mode), (0.98, 0));
}

#[test]
fn legacy_motion_vectors_on_seeds_mv_a() {
    assert_eq!(parse("bMotionVectorsOn=1\n").mv_a.value(), 1.0);
    assert_eq!(parse("bMotionVectorsOn=0\n").mv_a.value(), 0.0);
    assert_eq!(parse("bMotionVectorsOn=1\nmv_a=0.25\n").mv_a.value(), 0.25);
    assert_eq!(
        parse(&corpus("classic/bass_kaleido.milk")).mv_a.value(),
        1.0
    );
}

#[test]
fn empty_shader_sections_are_generated_from_parsed_values() {
    // upgraded_default.milk was authored to match the generated text for
    // decay 0.97 / gamma 1.56 / wrap on.
    let up = parse(&corpus("ps2/upgraded_default.milk"));
    assert_eq!(up.warp_shader, gen_warp_shader(0.97, true));
    let comp_params = CompShaderParams {
        gamma: 1.56,
        echo_alpha: 0.0,
        echo_zoom: 2.0,
        echo_orient: 0,
        hue_shader: 0.0,
        brighten: false,
        darken: false,
        solarize: false,
        invert: false,
    };
    assert_eq!(up.comp_shader, gen_comp_shader(&comp_params));
    // Stripping its shader lines makes the parser generate the same text.
    let stripped: String = corpus("ps2/upgraded_default.milk")
        .lines()
        .filter(|l| !l.starts_with("warp_") && !l.starts_with("comp_"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(parse(&stripped), up);

    // upgraded_echo.milk: decay 0.95, no wrap, echo alpha 0.5 / zoom 1.33 /
    // orientation 1, gamma 1.9.
    let echo_text = corpus("ps2x/upgraded_echo.milk");
    let echo = parse(&echo_text);
    let stripped: String = echo_text
        .lines()
        .filter(|l| !l.starts_with("warp_") && !l.starts_with("comp_"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(parse(&stripped), echo);
    assert!(echo.warp_shader.contains("sampler_fc_main"));

    // Generation happens regardless of version (Import does not check it).
    let classic = parse(&corpus("classic/drift_tunnel.milk"));
    assert_eq!(
        classic.warp_shader,
        gen_warp_shader(classic.decay.value(), classic.tex_wrap)
    );
    assert!(!classic.comp_shader.is_empty());

    // A present-but-empty first line is NOT empty to C++ (it stores the
    // line terminator), so nothing is generated.
    let p = parse("MILKDROP_PRESET_VERSION=201\nwarp_1=\ncomp_1=`\n");
    assert_eq!((p.warp_shader.as_str(), p.comp_shader.as_str()), ("", ""));
}

#[test]
fn code_is_stored_as_read_and_stripped_for_eel() {
    let p = parse("per_frame_1=zoom = 1.1; // note\nper_frame_2=`rot = 0.1; \\\\ x\nwave_0_per_point1=x = 1;//c\nshape_3_per_frame1=y=2;\n");
    assert_eq!(p.per_frame, "zoom = 1.1; // note\nrot = 0.1; \\\\ x");
    assert_eq!(p.per_frame_eel(), "zoom = 1.1; rot = 0.1; ");
    assert_eq!(p.waves[0].per_point_eel(), "x = 1;");
    assert_eq!(p.shapes[3].per_frame_eel(), "y=2;");
    assert_eq!(p.per_pixel_eel(), "");
}
