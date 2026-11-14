//! Tooltips of the menu items: the `.rc` string table (plugin.rc), by the
//! `IDS_MENU_*_TT` id each `AddItem` call names (plugin.cpp:6702-6869).
//! Generated from the resource text; `\n` is the newline the C++ strings hold.

pub const EDIT_PRESET_INIT_CODE_TT: &str = "read-only: zoom, rot, warp, cx, cy, dx, dy, sx, sy; decay, gamma;\n           echo_zoom, echo_scale, echo_orient;\n           ib_{size|r|g|b|a}, ob_{size|r|g|b|a}, mv_{x|y|dx|dy|l|r|g|b|a};\n           wave_{r|g|b|a|x|y|mode|mystery|usedots|thick|additive|brighten};\n           darken_center, wrap; invert, brighten, darken, solarize\n           time, fps, frame, progress; {bass|mid|treb}[_att]\nwrite:     q1-q8, monitor";
pub const EDIT_PER_FRAME_EQUATIONS_TT: &str = "read-only:  time, fps, frame, progress; {bass|mid|treb}[_att]\nread/write: zoom, rot, warp, cx, cy, dx, dy, sx, sy; q1-q8; monitor\n            mv_{x|y|dx|dy|l|r|g|b|a}, ib_{size|r|g|b|a}, ob_{size|r|g|b|a};\n            wave_{r|g|b|a|x|y|mode|mystery|usedots|thick|additive|brighten};\n            darken_center, wrap; invert, brighten, darken, solarize\n            decay, gamma, echo_zoom, echo_alpha, echo_orient";
pub const EDIT_PER_VERTEX_EQUATIONS_TT: &str = "read-only:  x, y, rad, ang; time, fps, frame, progress; {bass|mid|treb}[_att]\nread/write: dx, dy, zoom, rot, warp, cx, cy, sx, sy, q1-q8";
pub const EDIT_WARP_SHADER_TT: &str =
    "This pixel shader drives the warping, color, etc. of the internal image each frame.\n";
pub const EDIT_COMPOSITE_SHADER_TT: &str = "This pixel shader drives the final presentation of the internal image to the screen each frame.\n";
pub const EDIT_UPGRADE_PRESET_PS_VERSION_TT: &str =
    "Upgrades the preset to the next-available pixel shader version.";
pub const EDIT_DO_A_PRESET_MASH_UP_TT: &str =
    "Mix qualities from many different presets to create a new one.";
pub const WAVE_TYPE_TT: &str = "each value represents a different way of drawing the waveform";
pub const SIZE_TT: &str = "relative size of the waveform";
pub const SMOOTH_TT: &str = "controls the smoothness of the waveform; 0=natural sound data (no smoothing), 0.9=max. smoothing";
pub const MYSTERY_PARAMETER_TT: &str =
    "what this one does is a secret (actually, its effect depends on the 'wave type'";
pub const POSITION_X_TT: &str =
    "position of the waveform: 0 = far left edge of screen, 0.5 = center, 1 = far right";
pub const POSITION_Y_TT: &str =
    "position of the waveform: 0 = very bottom of screen, 0.5 = center, 1 = top";
pub const COLOR_RED_TT: &str = "amount of red color in the wave (0..1)";
pub const COLOR_GREEN_TT: &str = "amount of green color in the wave (0..1)";
pub const COLOR_BLUE_TT: &str = "amount of blue color in the wave (0..1)";
pub const OPACITY_TT: &str = "opacity of the waveform; lower numbers = more transparent";
pub const USE_DOTS_TT: &str = "if true, the waveform is drawn as dots (instead of lines)";
pub const DRAW_THICK_TT: &str =
    "if true, the waveform's lines (or dots) are drawn with double thickness";
pub const MODULATE_OPACITY_BY_VOLUME_TT: &str =
    "if true, the waveform opacity is affected by the music's volume";
pub const MODULATION_TRANSPARENT_VOLUME_TT: &str = "when the relative volume hits this level, the wave becomes transparent.  1 = normal loudness, 0.5 = extremely quiet, 1.5 = extremely loud";
pub const MODULATION_OPAQUE_VOLUME_TT: &str = "when the relative volume hits this level, the wave becomes opaque.  1 = normal loudness, 0.5 = extremely quiet, 1.5 = extremely loud";
pub const ADDITIVE_DRAWING_TT: &str =
    "if true, the wave is drawn additively, saturating the image at white";
pub const COLOR_BRIGHTENING_TT: &str = "if true, the red, green, and blue color components will be scaled up until at least one of them reaches 1.0";
pub const OUTER_BORDER_THICKNESS_TT: &str =
    "thickness of the outer border drawn at the edges of the screen";
pub const COLOR_RED_OUTER_TT: &str = "amount of red color in the outer border";
pub const COLOR_GREEN_OUTER_TT: &str = "amount of green color in the outer border";
pub const COLOR_BLUE_OUTER_TT: &str = "amount of blue color in the outer border";
pub const OPACITY_OUTER_TT: &str = "opacity of the outer border (0=transparent, 1=opaque)";
pub const INNER_BORDER_THICKNESS_TT: &str =
    "thickness of the inner border drawn at the edges of the screen";
pub const COLOR_RED_INNER_TT: &str = "amount of red color in the inner border";
pub const COLOR_GREEN_INNER_TT: &str = "amount of green color in the inner border";
pub const COLOR_BLUE_INNER_TT: &str = "amount of blue color in the inner border";
pub const OPACITY_INNER_TT: &str = "opacity of the inner border (0=transparent, 1=opaque)";
pub const MOTION_VECTOR_OPACITY_TT: &str =
    "opacity of the motion vectors (0=transparent, 1=opaque)";
pub const NUM_MOT_VECTORS_X_TT: &str = "the number of motion vectors on the x-axis";
pub const NUM_MOT_VECTORS_Y_TT: &str = "the number of motion vectors on the y-axis";
pub const OFFSET_X_TT: &str = "horizontal placement offset of the motion vectors";
pub const OFFSET_Y_TT: &str = "vertical placement offset of the motion vectors";
pub const TRAIL_LENGTH_TT: &str = "the length of the motion vectors (1=normal)";
pub const COLOR_RED_MOTION_VECTOR_TT: &str = "amount of red color in the motion vectors";
pub const COLOR_GREEN_MOTION_VECTOR_TT: &str = "amount of green color in the motion vectors";
pub const COLOR_BLUE_MOTION_VECTOR_TT: &str = "amount of blue color in the motion vectors";
pub const ZOOM_AMOUNT_TT: &str =
    "controls inward/outward motion.  0.9=zoom out, 1.0=no zoom, 1.1=zoom in";
pub const ZOOM_EXPONENT_TT: &str = "controls the curvature of the zoom; 1=normal";
pub const WARP_AMOUNT_TT: &str =
    "controls the magnitude of the warping; 0=none, 1=normal, 2=major warping...";
pub const WARP_SCALE_TT: &str =
    "controls the wavelength of the warp; 1=normal, less=turbulent, more=smoother";
pub const WARP_SPEED_TT: &str =
    "controls the speed of the warp; 1=normal, less=slower, more=faster";
pub const ROTATION_AMOUNT_TT: &str = "controls the amount of rotation.  0=none, 0.1=slightly right, -0.1=slightly clockwise, 0.1=CCW";
pub const ROTATION_CENTER_OF_X_TT: &str =
    "controls where the center of rotation is, horizontally.  0=left, 0.5=center, 1=right";
pub const ROTATION_CENTER_OF_Y_TT: &str =
    "controls where the center of rotation is, vertically.  0=top, 0.5=center, 1=bottom";
pub const TRANSLATION_X_TT: &str = "controls amount of constant horizontal motion; -0.01 = slight shift right, 0=none, 0.01 = to left";
pub const TRANSLATION_Y_TT: &str = "controls amount of constant vertical motion; -0.01 = slight shift downward, 0=none, 0.01 = upward";
pub const SCALING_X_TT: &str =
    "controls amount of constant horizontal stretching; 0.99=shrink, 1=normal, 1.01=stretch";
pub const SCALING_Y_TT: &str =
    "controls amount of constant vertical stretching; 0.99=shrink, 1=normal, 1.01=stretch";
pub const SUSTAIN_LEVEL_TT: &str =
    "controls the eventual fade to black; 1=no fade, 0.9=strong fade; 0.98=recommended.";
pub const DARKEN_CENTER_TT: &str =
    "when ON, help keeps the image from getting too bright by continually dimming the center point";
pub const GAMMA_ADJUSTMENT_TT: &str = "controls brightness; 1=normal, 2=double, 3=triple, etc.";
pub const HUE_SHADER_TT: &str = "adds subtle color variations to the image.  0=off, 1=fully on";
pub const VIDEO_ECHO_ALPHA_TT: &str = "controls the opacity of the second graphics layer; 0=transparent (off), 0.5=half-mix, 1=opaque";
pub const VIDEO_ECHO_ZOOM_TT: &str = "controls the size of the second graphics layer";
pub const VIDEO_ECHO_ORIENTATION_TT: &str = "selects an orientation for the second graphics layer.  0=normal, 1=flip on x, 2=flip on y, 3=flip on both";
pub const TEXTURE_WRAP_TT: &str =
    "sets whether or not screen elements can drift off of one side and onto the other";
pub const FILTER_INVERT_TT: &str = "inverts the colors in the image";
pub const FILTER_BRIGHTEN_TT: &str =
    "brightens the darker parts of the image (nonlinear; square root filter)";
pub const FILTER_DARKEN_TT: &str =
    "darkens the brighter parts of the image (nonlinear; squaring filter)";
pub const FILTER_SOLARIZE_TT: &str = "emphasizes mid-range colors";
pub const BLUR1_EDGE_DARKEN_AMOUNT_TT: &str =
    "keep this >0.25 to avoid edge artifacts, and <1.0 to avoid black borders.";
pub const BLUR1_MIN_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.";
pub const BLUR1_MAX_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.";
pub const BLUR2_MIN_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.  MUST BE SUBSET OF BLUR1 RANGE.";
pub const BLUR2_MAX_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.  MUST BE SUBSET OF BLUR1 RANGE.";
pub const BLUR3_MIN_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.  MUST BE SUBSET OF BLUR1, BLUR2 RANGES.";
pub const BLUR3_MAX_COLOR_VALUE_TT: &str = "narrowing these to just the color range you need will greatly increase color fidelity in the blurred images.  MUST BE SUBSET OF BLUR1, BLUR2 RANGES.";
pub const ENABLED_TT: &str = "enables or disables this custom waveform/spectrum";
pub const NUMBER_OF_SAMPLES_TT: &str = "the number of samples (points) that makes up the waveform";
pub const L_R_SEPARATION_TT: &str = "the offset between the left & right channels; useful for doing phase plots. Keep low (<32) when using w/spectrum.";
pub const SCALING_TT: &str = "the size of the wave (1=normal)";
pub const SMOOTHING_TT: &str = "0=the raw wave; 1=a highly damped (smoothed) wave";
pub const OPACITY_WAVE_TT: &str = "opacity of the waveform; 0=transparent, 1=opaque";
pub const USE_SPECTRUM_TT: &str = "if ON, the data in value1 and value2 will constitute a frequency spectrum (instead of waveform values)";
pub const USE_DOTS_WAVE_TT: &str =
    "if ON, the samples will be overdrawn 4X to make them thicker, bolder, and more visible";
pub const DRAW_THICK_WAVE_TT: &str =
    "if ON, the samples will be overdrawn 4X to make them thicker, bolder, and more visible";
pub const ADDITIVE_DRAWING_WAVE_TT: &str = "if ON, the samples will add color to sature the image toward white; otherwise, they replace what's there.";
pub const EXPORT_TO_FILE_TT: &str =
    "export the settings for this custom waveform to a file on disk";
pub const IMPORT_FROM_FILE_TT: &str = "import settings for a custom waveform from a file on disk";
pub const EDIT_INIT_CODE_TT: &str =
    "IN: time, frame, fps, progress; samples; q1-q8 (from preset init) / OUT: t1-t8";
pub const EDIT_PER_FRAME_CODE_TT: &str = "IN: time, frame, fps, progress; samples; q1-q8, t1-t8; r,g,b,a; {bass|mid|treb}[_att] / OUT: r,g,b,a; samples; t1-t8";
pub const EDIT_PER_POINT_CODE_TT: &str = "IN: sample [0..1]; value1 [left ch], value2 [right ch], plus all vars for per-frame code / OUT: x,y; r,g,b,a; t1-t8";
pub const ENABLED_SHAPE_TT: &str = "enables or disables this shape";
pub const NUMBER_OF_INSTANCES_TT: &str = "the number of times (num_inst) to draw this shape.  Each instance will have a different value for 'instance' in the per-frame eqs (0,1,2...)";
pub const NUMBER_OF_SIDES_TT: &str = "the default number of sides that make up the polygonal shape";
pub const DRAW_THICK_SHAPE_TT: &str =
    "if ON, the border will be overdrawn 4X to make it thicker, bolder, and more visible";
pub const ADDITIVE_DRAWING_SHAPE_TT: &str = "if ON, the shape will add color to sature the image toward white; otherwise, it will replace what's there.";
pub const X_POSITION_TT: &str = "default x position of the shape (0..1; 0=left side, 1=right side)";
pub const Y_POSITION_TT: &str = "default y position of the shape (0..1; 0=bottom, 1=top of screen)";
pub const RADIUS_TT: &str = "default radius of the shape (0+)";
pub const ANGLE_TT: &str = "default rotation angle of the shape (0...3.14*2)";
pub const TEXTURED_TT: &str =
    "if ON, the shape will be textured with the image from the previous frame";
pub const TEXTURE_ZOOM_TT: &str = "the portion of the previous frame's image to use with the shape";
pub const TEXTURE_ANGLE_TT: &str =
    "the angle at which to rotate the previous frame's image before applying it to the shape";
pub const INNER_COLOR_RED_TT: &str =
    "default amount of red color toward the center of the shape (0..1)";
pub const INNER_COLOR_GREEN_TT: &str =
    "default amount of green color toward the center of the shape (0..1)";
pub const INNER_COLOR_BLUE_TT: &str =
    "default amount of blue color toward the center of the shape (0..1)";
pub const INNER_OPACITY_TT: &str =
    "default opacity of the center of the shape; 0=transparent, 1=opaque";
pub const OUTER_COLOR_RED_TT: &str =
    "default amount of red color toward the outer edge of the shape (0..1)";
pub const OUTER_COLOR_GREEN_TT: &str =
    "default amount of green color toward the outer edge of the shape (0..1)";
pub const OUTER_COLOR_BLUE_TT: &str =
    "default amount of blue color toward the outer edge of the shape (0..1)";
pub const OUTER_OPACITY_TT: &str =
    "default opacity of the outer edge of the shape; 0=transparent, 1=opaque";
pub const BORDER_COLOR_RED_TT: &str = "default amount of red color in the shape's border (0..1)";
pub const BORDER_COLOR_GREEN_TT: &str =
    "default amount of green color in the shape's border (0..1)";
pub const BORDER_COLOR_BLUE_TT: &str = "default amount of blue color in the shape's border (0..1)";
pub const BORDER_OPACITY_TT: &str =
    "default opacity of the shape's border; 0=transparent, 1=opaque";
pub const EXPORT_TO_FILE_SHAPE_TT: &str =
    "export the settings for this custom shape to a file on disk";
pub const IMPORT_FROM_FILE_SHAPE_TT: &str =
    "import settings for a custom shape from a file on disk";
pub const EDIT_INIT_CODE_SHAPE_TT: &str = "IN: time, frame, fps, progress; q1-q8 (from preset init); x,y,rad,ang; r,g,b,a; r2,g2,b2,a2; border_{r|g|b|a}; sides, thick, additive, textured\nOUT: t1-t8; x,y,rad,ang; r,g,b,a; r2,g2,b2,a2; border_{r|g|b|a}; sides, thick, additive, textured";
pub const EDIT_PER_FRAME_INSTANCE_CODE_TT: &str = "IN: instance, num_inst, time, frame, fps, progress; q1-q8 (from preset init); x,y,rad,ang; r,g,b,a; r2,g2,b2,a2; border_{r|g|b|a}; sides, thick, additive, textured\nOUT: t1-t8; x,y,rad,ang; r,g,b,a; r2,g2,b2,a2; border_{r|g|b|a}; sides, thick, additive, textured";
