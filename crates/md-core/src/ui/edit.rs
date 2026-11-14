//! The string editor (`td_waitstr`, plugin.h:70-79, and the `WaitString_*`
//! functions, plugin.cpp:7877-8154): the text, cursor, selection and
//! overtype state of every mode that asks the user to type
//! (`UI_SAVEAS`, `UI_LOAD_RENAME`, `UI_EDIT_MENU_STRING`, `UI_CHANGEDIR` and
//! the wave and shape import/export prompts).
//!
//! Departures: the text is a Rust string of `char`s (the C++ keeps `wchar_t`
//! for file names and 8-bit `char` for code); a line break of a code string is
//! `'\n'` where the C++ uses `LINEFEED_CONTROL_CHAR` (1, md_defines.h:53);
//! the clipboard is the caller's (`UiCtx::clipboard`, `UiAction::Copy`), where
//! the C++ calls the Win32 clipboard; the key repeat count `nRepeat` is 1.

use crate::keymap::accepts_text_char;

/// `IsAlphanumericChar` (plugin.cpp:553): letters, digits and `.`.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.'
}

/// What typing a character could not do (the toasts of plugin.cpp:5196-5201).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeError {
    /// A character a file name cannot hold: `IDS_ILLEGAL_CHARACTER`.
    Illegal,
    /// The text is at its limit: `IDS_STRING_TOO_LONG`.
    TooLong,
}

/// `m_waitstring`.
#[derive(Clone, Debug, Default)]
pub struct WaitString {
    /// `bActive`: keys edit the text.
    pub active: bool,
    /// `bFilterBadChars`: refuse the characters a file name cannot hold.
    pub filter_bad_chars: bool,
    /// `bDisplayAsCode`: multi-line, Enter inserts a line break and Ctrl+Enter
    /// accepts.
    pub display_as_code: bool,
    /// `nMaxLen`: the text holds at most `max_len - 1` characters.
    pub max_len: usize,
    /// `nCursorPos`, in characters.
    pub cursor: usize,
    /// `nSelAnchorPos`: the other end of the selection, if any.
    pub anchor: Option<usize>,
    /// `bOvertypeMode`.
    pub overtype: bool,
    /// `szText`.
    pub text: Vec<char>,
    /// `szPrompt`.
    pub prompt: String,
    /// `szToolTip`.
    pub tooltip: String,
}

impl WaitString {
    /// The text as a string.
    pub fn as_string(&self) -> String {
        self.text.iter().collect()
    }

    /// Replace the text and put the cursor at its end.
    pub fn set_text(&mut self, s: &str) {
        self.text = s.chars().collect();
        self.cursor = self.text.len();
        self.anchor = None;
    }

    /// The selection as `[start, end)` when one is made
    /// (`nSelAnchorPos != -1`; the C++ treats an empty selection as one).
    fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        Some((a.min(self.cursor), a.max(self.cursor)))
    }

    /// Whether a non-empty selection is shown (`bBrackets`, plugin.cpp:4329).
    pub fn has_visible_selection(&self) -> bool {
        self.anchor.is_some_and(|a| a != self.cursor)
    }

    /// `WaitString_NukeSelection`: delete the selection.
    pub fn nuke_selection(&mut self) {
        if let Some((s, e)) = self.selection() {
            self.text.drain(s..e.min(self.text.len()));
            self.cursor = s;
            self.anchor = None;
        }
    }

    /// `WaitString_Copy`: the selected text, if any.
    pub fn copy(&self) -> Option<String> {
        let (s, e) = self.selection()?;
        Some(self.text[s..e.min(self.text.len())].iter().collect())
    }

    /// `WaitString_Cut`: the selected text, removed.
    pub fn cut(&mut self) -> Option<String> {
        let c = self.copy()?;
        self.nuke_selection();
        Some(c)
    }

    /// `WaitString_Paste`: replace the selection with `clip`, up to the
    /// limit. `CR LF` and `CR` become the line break (`ConvertCRsToLFC`).
    pub fn paste(&mut self, clip: &str) -> Result<(), TypeError> {
        self.nuke_selection();
        let clip = clip.replace("\r\n", "\n").replace('\r', "\n");
        let mut ins: Vec<char> = clip.chars().collect();
        let len = self.text.len();
        let mut result = Ok(());
        if len + ins.len() + 1 >= self.max_len {
            ins.truncate(self.max_len.saturating_sub(len + 1));
            result = Err(TypeError::TooLong);
        }
        let at = self.cursor.min(len);
        self.text.splice(at..at, ins.iter().copied());
        self.cursor = at + ins.len();
        result
    }

    /// A typed character (`WM_CHAR` in the editor, plugin.cpp:5171-5285).
    /// Characters the C++ does not take ([`accepts_text_char`]) are absorbed
    /// without effect: `Ok(())`.
    pub fn type_char(&mut self, c: char) -> Result<(), TypeError> {
        if !accepts_text_char(c) {
            return Ok(());
        }
        if self.filter_bad_chars && "\"\\/:*?|<>&".contains(c) {
            return Err(TypeError::Illegal);
        }
        if self.text.len() + 1 >= self.max_len {
            return Err(TypeError::TooLong);
        }
        self.nuke_selection();
        let len = self.text.len();
        let at = self.cursor.min(len);
        if self.overtype && at < len {
            self.text[at] = c;
        } else {
            self.text.insert(at, c);
        }
        self.cursor = at + 1;
        Ok(())
    }

    /// `VK_BACK`: the selection, else the character left of the cursor.
    pub fn backspace(&mut self) {
        if self.anchor.is_some() {
            self.nuke_selection();
        } else if self.cursor > 0 {
            let at = self.cursor.min(self.text.len());
            if at > 0 {
                self.text.remove(at - 1);
            }
            self.cursor = at.saturating_sub(1);
        }
    }

    /// `VK_DELETE`: the selection, else the character under the cursor.
    pub fn delete(&mut self) {
        if self.anchor.is_some() {
            self.nuke_selection();
        } else if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Enter in a code string: a line break (plugin.cpp:5748-5769); `Err`
    /// when the text is full.
    pub fn insert_line_break(&mut self) -> Result<(), TypeError> {
        self.nuke_selection();
        if self.text.len() + 1 < self.max_len {
            let at = self.cursor.min(self.text.len());
            self.text.insert(at, '\n');
            self.cursor = at + 1;
            Ok(())
        } else {
            Err(TypeError::TooLong)
        }
    }

    /// The shift handling before a cursor key (plugin.cpp:5501-5514): Shift
    /// starts a selection at the cursor, no Shift clears it.
    pub fn begin_move(&mut self, shift: bool) {
        if shift {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
    }

    /// `VK_LEFT`.
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// `VK_RIGHT`.
    pub fn right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    /// `WaitString_SeekLeftWord`.
    pub fn seek_left_word(&mut self) {
        while self.cursor > 0 && !is_word_char(self.text[self.cursor - 1]) {
            self.cursor -= 1;
        }
        while self.cursor > 0 && is_word_char(self.text[self.cursor - 1]) {
            self.cursor -= 1;
        }
    }

    /// `WaitString_SeekRightWord`.
    pub fn seek_right_word(&mut self) {
        let len = self.text.len();
        while self.cursor < len && is_word_char(self.text[self.cursor]) {
            self.cursor += 1;
        }
        while self.cursor < len && !is_word_char(self.text[self.cursor]) {
            self.cursor += 1;
        }
    }

    /// `WaitString_GetCursorColumn`: for code, the distance from the line
    /// start; for a one-line string, the whole position.
    pub fn cursor_column(&self) -> usize {
        if !self.display_as_code {
            return self.cursor;
        }
        let mut col = 0;
        while col < self.cursor && self.text.get(self.cursor - col - 1) != Some(&'\n') {
            col += 1;
        }
        col
    }

    /// `WaitString_GetLineLength`: the length of the cursor's line.
    pub fn line_length(&self) -> usize {
        let start = self.cursor - self.cursor_column();
        self.text[start.min(self.text.len())..]
            .iter()
            .take_while(|&&c| c != '\n')
            .count()
    }

    /// `VK_HOME`: the start of the line.
    pub fn home(&mut self) {
        self.cursor -= self.cursor_column();
    }

    /// `VK_END`: the end of the line.
    pub fn end(&mut self) {
        self.cursor += self.line_length() - self.cursor_column().min(self.line_length());
    }

    /// Ctrl+Home.
    pub fn home_of_text(&mut self) {
        self.cursor = 0;
    }

    /// Ctrl+End: after the last character (plugin.cpp:5536).
    pub fn end_of_text(&mut self) {
        self.cursor = self.text.len();
    }

    /// `WaitString_SeekUpOneLine`.
    pub fn up(&mut self) {
        let column = self.cursor_column();
        if column != self.cursor {
            // the very end of the previous line
            self.cursor -= column + 1;
            let new_column = self.cursor_column();
            if new_column > column {
                self.cursor -= new_column - column;
            }
        }
    }

    /// `WaitString_SeekDownOneLine`.
    pub fn down(&mut self) {
        if !self.display_as_code {
            return;
        }
        let mut column = self.cursor_column();
        let len = self.text.len();
        let mut new = self.cursor;
        while new < len && self.text[new] != '\n' {
            new += 1;
        }
        if new < len {
            self.cursor = new + 1;
            while column > 0 && self.cursor < len && self.text[self.cursor] != '\n' {
                self.cursor += 1;
                column -= 1;
            }
        }
    }

    /// The text as drawn (plugin.cpp:4328-4461): with a selection, `[` `]`
    /// around it; otherwise the character under the cursor swaps with `_`
    /// while `blink` is on and with itself (a space at the end of the text
    /// or of a line) while it is off.
    pub fn display_text(&self, blink: bool) -> String {
        let mut buf = self.text.clone();
        let cur = self.cursor.min(buf.len());
        if let Some(a) = self.anchor.filter(|&a| a != self.cursor) {
            let (s, e) = (a.min(cur), a.max(cur));
            buf.insert(e.min(buf.len()), ']');
            buf.insert(s.min(buf.len()), '[');
        } else if blink {
            match buf.get(cur) {
                None => buf.push('_'),
                Some('\n') => buf.insert(cur, '_'),
                Some('_') => buf[cur] = ' ',
                Some(_) => buf[cur] = '_',
            }
        } else {
            match buf.get(cur) {
                None => buf.push(' '),
                Some('\n') => buf.insert(cur, ' '),
                Some(_) => {}
            }
        }
        buf.into_iter().collect()
    }
}
