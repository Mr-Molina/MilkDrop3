//! The fifteen UI modes (`ui_mode`, plugin.h:48) and what each one means for
//! the input and the screen.

/// `UI_REGULAR` ... `UI_MASHUP`, in the C++ order (plugin.h:48).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiMode {
    /// `UI_REGULAR`: no UI up; keys are hotkeys.
    #[default]
    Regular,
    /// `UI_MENU`: the preset-editing menu.
    Menu,
    /// `UI_LOAD`: the preset list.
    Load,
    /// `UI_LOAD_DEL`: "are you sure you want to delete" `[y/N]`.
    LoadDel,
    /// `UI_LOAD_RENAME`: typing the new file name.
    LoadRename,
    /// `UI_SAVEAS`: typing the file name to save under.
    SaveAs,
    /// `UI_SAVE_OVERWRITE`: "this file already exists, overwrite it?" `[y/N]`.
    SaveOverwrite,
    /// `UI_EDIT_MENU_STRING`: editing a menu item's code string.
    EditMenuString,
    /// `UI_CHANGEDIR`: typing a preset directory (its F8 key is commented
    /// out, plugin.cpp:5449-5470; the mode is still handled).
    ChangeDir,
    /// `UI_IMPORT_WAVE`: typing the file a custom wave is read from.
    ImportWave,
    /// `UI_EXPORT_WAVE`: typing the file a custom wave is written to.
    ExportWave,
    /// `UI_IMPORT_SHAPE`.
    ImportShape,
    /// `UI_EXPORT_SHAPE`.
    ExportShape,
    /// `UI_UPGRADE_PIXEL_SHADER`: the shader upgrade question `[y/N]`.
    UpgradePixelShader,
    /// `UI_MASHUP`: the preset mash-up screen.
    Mashup,
}

impl UiMode {
    /// Every mode, in the C++ order.
    pub const ALL: [UiMode; 15] = [
        UiMode::Regular,
        UiMode::Menu,
        UiMode::Load,
        UiMode::LoadDel,
        UiMode::LoadRename,
        UiMode::SaveAs,
        UiMode::SaveOverwrite,
        UiMode::EditMenuString,
        UiMode::ChangeDir,
        UiMode::ImportWave,
        UiMode::ExportWave,
        UiMode::ImportShape,
        UiMode::ExportShape,
        UiMode::UpgradePixelShader,
        UiMode::Mashup,
    ];

    /// The modes in which the string editor (`m_waitstring`) is active:
    /// typed characters edit text.
    pub fn is_text_entry(self) -> bool {
        matches!(
            self,
            UiMode::LoadRename
                | UiMode::SaveAs
                | UiMode::EditMenuString
                | UiMode::ChangeDir
                | UiMode::ImportWave
                | UiMode::ExportWave
                | UiMode::ImportShape
                | UiMode::ExportShape
        )
    }

    /// The `[y/N]` prompts, which any character answers
    /// (plugin.cpp:5287-5391).
    pub fn is_confirm(self) -> bool {
        matches!(
            self,
            UiMode::LoadDel | UiMode::SaveOverwrite | UiMode::UpgradePixelShader
        )
    }
}
