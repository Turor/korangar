//! CJK fallback for text cosmic-text cannot draw with Noto Sans.
//!
//! Hangul in stock `iteminfo.lub` is CP949. After decoding, those codepoints
//! are missing from the primary face and used to render as `.notdef` boxes.
//! The bundled OFL subsets are Noto Sans KR (Hangul) and Noto Sans JP
//! (kana and kanji). cosmic-text's Unix list asks for "Noto Sans CJK KR/JP";
//! the region-subset files are named "Noto Sans KR" and "Noto Sans JP", so
//! both names are listed.

use cosmic_text::{Fallback, PlatformFallback};
use unicode_script::Script;

const CJK_FAMILIES: &[&str] = &["Noto Sans KR", "Noto Sans JP", "Noto Sans CJK KR", "Noto Sans CJK JP"];

const EMBEDDED_FONTS: &[(&str, &[u8])] = &[
    ("NotoSansKR", include_bytes!("../../../archive/data/font/NotoSansKR.otf")),
    ("NotoSansJP", include_bytes!("../../../archive/data/font/NotoSansJP.otf")),
];

pub(super) fn embedded_font(name: &str) -> Option<&'static [u8]> {
    EMBEDDED_FONTS
        .iter()
        .find(|(font_name, _)| *font_name == name)
        .map(|(_, bytes)| *bytes)
}

/// First face that covers the codepoint. Earlier faces are the primary font.
#[cfg(test)]
fn face_for_codepoint<'a>(faces: &[(&'a str, bool)]) -> Option<&'a str> {
    faces.iter().find(|(_, covers)| *covers).map(|(family, _)| *family)
}

pub(crate) fn cjk_script(script: Script) -> bool {
    matches!(
        script,
        Script::Hangul | Script::Han | Script::Hiragana | Script::Katakana | Script::Bopomofo
    )
}

#[derive(Debug)]
pub(super) struct CjkFallback;

impl Fallback for CjkFallback {
    fn common_fallback(&self) -> &[&'static str] {
        PlatformFallback.common_fallback()
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        PlatformFallback.forbidden_fallback()
    }

    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        if cjk_script(script) {
            CJK_FAMILIES
        } else {
            PlatformFallback.script_fallback(script, locale)
        }
    }
}

#[cfg(test)]
mod tests {
    use ttf_parser::Face;
    use unicode_script::Script;

    use super::{CJK_FAMILIES, EMBEDDED_FONTS, cjk_script, face_for_codepoint};

    fn covers(font: &[u8], ch: char) -> bool {
        let face = Face::parse(font, 0).expect("parse face");
        face.glyph_index(ch).is_some_and(|glyph| glyph.0 != 0)
    }

    #[test]
    fn hangul_falls_back_to_the_cjk_face() {
        use cosmic_text::{Fallback, PlatformFallback};

        let fallback = super::CjkFallback;
        assert_eq!(fallback.script_fallback(Script::Hangul, "en-US"), CJK_FAMILIES);
        assert_eq!(fallback.script_fallback(Script::Han, "ko-KR"), CJK_FAMILIES);
        assert_eq!(fallback.script_fallback(Script::Hiragana, "ja-JP"), CJK_FAMILIES);
        assert_eq!(
            fallback.script_fallback(Script::Latin, "en-US"),
            PlatformFallback.script_fallback(Script::Latin, "en-US")
        );
        assert_ne!(fallback.script_fallback(Script::Latin, "en-US"), CJK_FAMILIES);

        assert!(cjk_script(Script::Hangul));
        assert!(CJK_FAMILIES.contains(&"Noto Sans KR"));

        let primary = include_bytes!("../../../archive/data/font/NotoSans.ttf");
        let kr = EMBEDDED_FONTS[0].1;
        let jp = EMBEDDED_FONTS[1].1;

        assert!(covers(primary, 'A'));
        assert!(!covers(primary, '가'));
        assert!(!covers(primary, 'あ'));
        assert!(!covers(primary, '刀'));
        assert!(covers(kr, '가'));
        assert!(covers(kr, '뭐'));
        assert!(covers(kr, 'あ') || covers(jp, 'あ'));
        assert!(covers(kr, 'ア') || covers(jp, 'ア'));
        assert!(covers(kr, '刀') || covers(jp, '刀'));

        let faces = [("Noto Sans", covers(primary, '가')), ("Noto Sans KR", covers(kr, '가'))];
        assert_eq!(face_for_codepoint(&faces), Some("Noto Sans KR"));

        let latin = [("Noto Sans", covers(primary, 'A')), ("Noto Sans KR", covers(kr, 'A'))];
        assert_eq!(face_for_codepoint(&latin), Some("Noto Sans"));

        let hangul = Face::parse(kr, 0).unwrap().glyph_index('가').unwrap();
        let mut outlines = 0;
        let mut sink = CountOutlines(&mut outlines);
        assert!(Face::parse(kr, 0).unwrap().outline_glyph(hangul, &mut sink).is_some());
        assert!(outlines > 0);
    }

    struct CountOutlines<'a>(&'a mut usize);

    impl ttf_parser::OutlineBuilder for CountOutlines<'_> {
        fn move_to(&mut self, _: f32, _: f32) {
            *self.0 += 1;
        }

        fn line_to(&mut self, _: f32, _: f32) {
            *self.0 += 1;
        }

        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
            *self.0 += 1;
        }

        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            *self.0 += 1;
        }

        fn close(&mut self) {}
    }
}
