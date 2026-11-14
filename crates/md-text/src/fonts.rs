//! Font lookup: `fontdb` query by family/weight/style with the bundled font as fallback.

use crate::FontSpec;

/// Liberation Sans Regular 2.1.5 (SIL OFL 1.1; see `assets/fonts/LICENSE-LiberationSans.txt`).
pub const BUNDLED_FONT: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
pub const BUNDLED_FAMILY: &str = "Liberation Sans";

/// A fresh database holding only the bundled font; deterministic on every machine.
pub fn bundled_database() -> fontdb::Database {
    let mut db = fontdb::Database::new();
    db.load_font_data(BUNDLED_FONT.to_vec());
    db
}

/// A database with the system fonts plus the bundled font (what the app uses).
pub fn system_database() -> fontdb::Database {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    db.load_font_data(BUNDLED_FONT.to_vec());
    db
}

pub(crate) struct Loaded {
    pub font: fontdue::Font,
    pub fallback: bool,
}

fn from_db(spec: &FontSpec, db: &fontdb::Database) -> Option<fontdue::Font> {
    let families = [fontdb::Family::Name(&spec.face)];
    let query = fontdb::Query {
        families: &families,
        weight: if spec.bold {
            fontdb::Weight::BOLD
        } else {
            fontdb::Weight::NORMAL
        },
        stretch: fontdb::Stretch::Normal,
        style: if spec.italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        },
    };
    let id = db.query(&query)?;
    db.with_face_data(id, |data, index| {
        let settings = fontdue::FontSettings {
            collection_index: index,
            ..Default::default()
        };
        fontdue::Font::from_bytes(data, settings).ok()
    })?
}

/// Match `spec.face` (case-insensitive family name, as Win32 face names are) at the
/// requested weight (Win32 uses 900 for bold and 400 otherwise) and style; if the face is
/// missing or unusable, the bundled font. Bold/italic are not synthesized: if the family
/// only has other styles, `fontdb` picks the closest face.
pub(crate) fn load(spec: &FontSpec, db: &fontdb::Database) -> Loaded {
    match from_db(spec, db) {
        Some(font) => Loaded {
            font,
            fallback: false,
        },
        None => Loaded {
            font: fontdue::Font::from_bytes(BUNDLED_FONT, fontdue::FontSettings::default())
                .expect("bundled font parses"),
            fallback: true,
        },
    }
}
