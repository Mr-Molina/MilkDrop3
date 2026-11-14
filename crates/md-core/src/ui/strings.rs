//! The UI's text: the entries of the `.rc` string table (plugin.rc) the menus,
//! prompts and overlays use, under the `IDS_*` name each carries. `%s`/`%d`
//! formats are functions. `\n` is a real newline.

/// `IDS_SZ_MENU_NAV_TOOLTIP` (plugin.rc:128).
pub const MENU_NAV_TOOLTIP: &str =
    "navigation: ESC: exit, Left Arrow: back, Right Arrow: select, UP/DOWN: change sel";
/// `IDS_ON`.
pub const ON: &str = "ON";
/// `IDS_OFF`.
pub const OFF: &str = "OFF";
/// `IDS_USE_UP_DOWN_ARROW_KEYS`.
pub const USE_UP_DOWN_ARROW_KEYS: &str = "(use up/down arrow keys, PGUP, PGDN to change value)";
/// `IDS_CURRENT_VALUE_OF_X`.
pub fn current_value_of(name: &str) -> String {
    format!("Current value of {name}:")
}
/// `IDS_LOAD_FROM_FILE`.
pub const LOAD_FROM_FILE: &str =
    "Load from file:  [note: preset's wave scaling, as well as q1-q8, will not be imported]";
/// `IDS_SAVE_TO_FILE`.
pub const SAVE_TO_FILE: &str =
    "Save to file:  [note: preset's wave scaling, as well as q1-q8, will not be exported]";
/// `IDS_ENTER_THE_NEW_STRING`.
pub const ENTER_THE_NEW_STRING: &str =
    "Enter the new string; hit CTRL+ENTER to apply or ESC to cancel.";
/// `IDS_SAVE_AS`.
pub const SAVE_AS: &str = "Save as:";
/// `IDS_DIRECTORY_TO_JUMP_TO`.
pub const DIRECTORY_TO_JUMP_TO: &str = "Directory to jump to:";
/// `IDS_ENTER_THE_NEW_NAME_FOR_X`.
pub fn enter_the_new_name_for(name: &str) -> String {
    format!("Enter the new name for \"{name}\":")
}
/// `IDS_RATING`.
pub const RATING: &str = "Rating";
/// `IDS_DISABLED`.
pub const DISABLED: &str = "[disabled] ";
/// `IDS_FPS`.
pub const FPS: &str = "fps";
/// `IDS_PF_MONITOR`.
pub const PF_MONITOR: &str = "pf monitor";
/// `IDS_PRESS_F1_MSG`.
pub const PRESS_F1_MSG: &str = "Press F1 for Help ";
/// `IDS_ILLEGAL_CHARACTER`.
pub const ILLEGAL_CHARACTER: &str = "(illegal character)";
/// `IDS_STRING_TOO_LONG`.
pub const STRING_TOO_LONG: &str = "(string too long)";
/// `IDS_INVALID_PATH`.
pub const INVALID_PATH: &str = "(invalid path)";
/// `IDS_ERROR_IMPORTING_BAD_FILENAME`.
pub const ERROR_IMPORTING_BAD_FILENAME: &str =
    "(error importing - bad filename, or file does not exist)";
/// `IDS_ERROR_IMPORTING_BAD_FILENAME_OR_NOT_OVERWRITEABLE`.
pub const ERROR_EXPORTING_BAD_FILENAME: &str =
    "(error exporting - bad filename, or file can not be overwritten)";
/// `IDS_PAGE_X` (the string editor's page tooltip).
pub fn page(n: usize) -> String {
    format!(" Page {n} ")
}
/// `IDS_PAGE_X_OF_X`.
pub fn page_of(n: usize, total: usize) -> String {
    format!(" (page {n} of {total}) ")
}
/// `IDS_LOCKED`.
pub const LOCKED: &str = "<locked> ";
/// `IDS_PARENT_DIRECTORY`.
pub const PARENT_DIRECTORY: &str = "parent directory";
/// `IDS_DIRECTORY_OF_X`.
pub fn directory_of(dir: &str) -> String {
    format!("Directory of: {dir}")
}
/// `IDS_LOAD_WHICH_PRESET_PLUS_COMMANDS`.
pub const LOAD_WHICH_PRESET: &str =
    "Load which preset?  (arrow keys to scroll; Esc/close, Enter/select, INS/rename; DEL/delete)";
/// `IDS_ERROR_NO_PRESET_FILE_FOUND_IN_X_MILK`.
pub fn no_preset_file_found_in(dir: &str) -> String {
    format!("ERROR: No preset files found in {dir}*.milk")
}
/// `IDS_ARE_YOU_SURE_YOU_WANT_TO_DELETE_PRESET`.
pub const ARE_YOU_SURE_DELETE: &str = "Are you SURE you want to delete this preset? [y/N]";
/// `IDS_PRESET_TO_DELETE`.
pub fn preset_to_delete(name: &str) -> String {
    format!("(preset to delete: {name})")
}
/// `IDS_FILE_ALREADY_EXISTS_OVERWRITE_IT`.
pub const FILE_ALREADY_EXISTS: &str = "This file already exists.  Overwrite it?  [y/N]";
/// `IDS_FILE_IN_QUESTION_X_MILK`.
pub fn file_in_question(name: &str) -> String {
    format!("(file in question: {name}.milk)")
}
/// `IDS_WARNING_DO_NOT_FORGET_WARP_SHADER_WAS_LOCKED`.
pub const WARP_SHADER_WAS_LOCKED: &str = " WARNING: do not forget WARP shader was LOCKED! ";
/// `IDS_WARNING_DO_NOT_FORGET_COMPOSITE_SHADER_WAS_LOCKED`.
pub const COMP_SHADER_WAS_LOCKED: &str = " WARNING: do not forget COMPOSITE shader was LOCKED! ";
/// `IDS_WARP_AND_COMPOSITE_SHADERS_LOCKED`.
pub const WARP_AND_COMP_LOCKED: &str = "(keep in mind... warp and composite shaders were locked.)";
/// `IDS_WARP_SHADER_LOCKED`.
pub const WARP_LOCKED: &str = "(keep in mind... warp shader was locked.)";
/// `IDS_COMPOSITE_SHADER_LOCKED`.
pub const COMP_LOCKED: &str = "(keep in mind... composite shader was locked.)";
/// `IDS_PRESS_F9_TO_HIDE_SHADER_QREF`.
pub const PRESS_F9_HIDE_QREF: &str = "-- press F9 to hide shader Quick Reference --\n";
/// `IDS_PRESS_F9_TO_SHOW_SHADER_QREF`.
pub const PRESS_F9_SHOW_QREF: &str = "-- press F9 to show shader Quick Reference --\n";
/// `IDS_PRESS_ESC_TO_RETURN`.
pub const PRESS_ESC_TO_RETURN: &str = "Press ESC to return.";
/// `IDS_PRESET_USES_HIGHEST_PIXEL_SHADER_VERSION`.
pub fn highest_ps_version(v: i32) -> String {
    format!("This preset already uses the highest pixel shader version ({v}) supported by your graphics chip.")
}
/// `IDS_PRESET_HAS_MIXED_VERSIONS_OF_SHADERS`.
pub const MIXED_SHADERS: &str = "This preset has mixed versions of shaders in it.";
/// `IDS_UPGRADE_SHADERS_TO_USE_PS2`.
pub const UPGRADE_ALL_PS2: &str =
    "Do you want to upgrade all shaders to use (at least) pixel shader 2?  [y/N]";
/// `IDS_UPGRADE_SHADERS_TO_USE_PS2X`.
pub const UPGRADE_ALL_PS2X: &str =
    "Do you want to upgrade all shaders to use (at least) pixel shader 2.X?  [y/N]";
/// `IDS_UPGRADE_SHADERS_TO_USE_PS3`.
pub const UPGRADE_ALL_PS3: &str =
    "Do you want to upgrade all shaders to use (at least) pixel shader 3?  [y/N]";
/// `IDS_PRESET_DOES_NOT_USE_PIXEL_SHADERS`.
pub const NO_PIXEL_SHADERS: &str = "This preset does not currently use pixel shaders.";
/// `IDS_UPGRADE_TO_USE_PS2`.
pub const UPGRADE_PS2: &str = "Do you want to upgrade it to use pixel shader 2?  [y/N]";
/// `IDS_UPGRADE_TO_USE_PS2X`.
pub const UPGRADE_PS2X: &str = "Do you want to upgrade it to use pixel shader 2.X?  [y/N]";
/// `IDS_UPGRADE_TO_USE_PS3`.
pub const UPGRADE_PS3: &str = "Do you want to upgrade it to use pixel shader 3?  [y/N]";
/// `IDS_UPGRADE_TO_USE_PS4`.
pub const UPGRADE_PS4: &str = "Do you want to upgrade it to use pixel shader 4?  [y/N]";
/// `IDS_WARNING_OLD_GPU_MIGHT_NOT_WORK_WITH_PRESET`.
pub const OLD_GPU_WARNING: &str = "  (WARNING: if you save it, other users with older graphics chips might not be able to use the preset.";
/// `IDS_PRESET_CURRENTLY_USES_PS2`.
pub const USES_PS2: &str = "This preset currently uses pixel shader 2.";
/// `IDS_PRESET_CURRENTLY_USES_PS2X`.
pub const USES_PS2X: &str = "This preset currently uses pixel shader 2.X.";
/// `IDS_PRESET_CURRENTLY_USES_PS3`.
pub const USES_PS3: &str = "This preset currently uses pixel shader 3.";
/// `IDS_PRESET_MASH_UP_TEXT1..4`.
pub const MASHUP_TEXT: [&str; 4] = [
    "Preset Mash-Up:          use keys 1-5 to change bin;",
    "                         up/down/a-z to browse presets for current bin;",
    "                         h to randomize preset (or H = all bins); ",
    "                         esc exits.",
];
/// `IDS_MASHUP_GENERAL_POSTPROC` .. `IDS_MASHUP_COMP_SHADER`.
pub const MASHUP_NAMES: [&str; 5] = [
    " 1. general, postproc: ",
    " 2. motion, equations: ",
    " 3. waveforms, shapes: ",
    " 4. warp shader:       ",
    " 5. comp shader:       ",
];
/// `IDS_SAVE_SUCCESSFUL` (shown by the host after a save).
pub const SAVE_SUCCESSFUL: &str = "[save successful]";
/// `IDS_ERROR_UNABLE_TO_SAVE_THE_FILE`.
pub const ERROR_UNABLE_TO_SAVE: &str = "ERROR: unable to save the file";
/// `IDS_ERROR_UNABLE_TO_DELETE_THE_FILE`.
pub const ERROR_UNABLE_TO_DELETE: &str = "ERROR: unable to delete the file";
/// `IDS_PRESET_X_DELETED`.
pub fn preset_deleted(name: &str) -> String {
    format!("[preset \"{name}\" deleted]")
}
/// `IDS_ERROR_A_FILE_ALREADY_EXISTS_WITH_THAT_FILENAME`.
pub const ERROR_FILE_EXISTS: &str = "ERROR: a file already exists with that filename";
/// `IDS_ERROR_UNABLE_TO_RENAME_FILE`.
pub const ERROR_UNABLE_TO_RENAME: &str = "ERROR: unable to rename the file";
/// `IDS_RENAME_SUCCESSFUL`.
pub const RENAME_SUCCESSFUL: &str = "[rename successful]";

/// `IDS_STRING615` .. `IDS_STRING635`: the shader quick reference (F9 while
/// editing a shader, plugin.cpp:4259-4308). The first is the widest line, the
/// box is sized by it; then the six lines both shaders show; then the seven
/// of the warp shader or the seven of the composite shader.
pub const QREF_WIDEST: &str =
    "           tex2D(sampler_billy, uv)    sample pixel from billy.jpg (must be loaded)\n";
/// `IDS_STRING616..621`.
pub const QREF_COMMON: [&str; 6] = [
    " pre-body: sampler sampler_billy;    //loads billy.jpg",
    "           float4  texsize_billy;    //.xy = width and height, .zw = 1/w, 1/h",
    " sampling textures:",
    "           tex2D(sampler_main, uv)   //sample pixel from prior frame",
    "           tex2D(sampler_billy, uv)  //sample pixel from billy.jpg (must be loaded)",
    "           GetBlur1(uv)              //sample blurred prior-frame pixel (also 2,3)",
];
/// `IDS_STRING622..628`.
pub const QREF_WARP: [&str; 7] = [
    " inputs:   float2 uv;        //warped UV coordinates [0..1]",
    "           float2 uv_orig;   //original UV coordinates [0..1]",
    "           float  rad;       //radius [0..1]",
    "           float  ang;       //angle [0..PI*2]",
    "           // + texsize, aspect, time, fps, rand_frame, rand_preset,",
    "           //   q1-q32/_qa-_qh, bass, treb_att, vol... - see docs",
    " output:   float3 ret;       //the new pixel color (r,g,b)",
];
/// `IDS_STRING629..635`.
pub const QREF_COMP: [&str; 7] = [
    " inputs:   float2 uv;          //UV coordinates [0..1]",
    "           float  rad;         //radius [0..1]",
    "           float  ang;         //angle [0..PI*2]",
    "           float3 hue_shader   //for MilkDrop 1 compatibility",
    "           // + texsize, aspect, time, fps, rand_frame, rand_preset,",
    "           //   q1-q32/_qa-_qh, bass, treb_att, vol... - see docs",
    " output:   float3 ret;         //display pixel color (r,g,b)",
];
