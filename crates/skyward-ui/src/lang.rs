use anyhow::{Context, Result};
use icu_locale_core::Locale;

/// A language the pages speak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// Chinese in simplified characters, as mainland China writes it.
    Hans,
    /// Chinese in traditional characters, as Taiwan writes it.
    Hant,
    En,
    /// Japanese. Its texts are being written; it answers in English until
    /// they are, through [`Lang::pick`], and says so on the language page.
    Ja,
    /// Korean, answered in English until its texts are written.
    Ko,
    /// Russian, answered in English until its texts are written.
    Ru,
    /// German, answered in English until its texts are written.
    De,
    /// French, answered in English until its texts are written.
    Fr,
    /// Spanish, answered in English until its texts are written.
    Es,
    /// Portuguese, answered in English until its texts are written.
    Pt,
    /// Italian, answered in English until its texts are written.
    It,
    /// Arabic. Written right to left, which the pages do not lay out yet;
    /// answered in English until both its texts and the layout are written.
    Ar,
    /// Thai, answered in English until its texts are written.
    Th,
    /// Vietnamese, answered in English until its texts are written.
    Vi,
    /// Indonesian, answered in English until its texts are written.
    Id,
}

/// One text written in each language, in the order of [`Lang::ALL`].
pub type Text = [&'static str; 3];

impl Lang {
    /// The languages, in the order the language page lists them.
    pub const ALL: [Lang; 15] = [
        Lang::Hans,
        Lang::Hant,
        Lang::En,
        Lang::Ja,
        Lang::Ko,
        Lang::Ru,
        Lang::De,
        Lang::Fr,
        Lang::Es,
        Lang::Pt,
        Lang::It,
        Lang::Ar,
        Lang::Th,
        Lang::Vi,
        Lang::Id,
    ];

    /// The language's name, written in it.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Hans => "\u{7b80}\u{4f53}\u{4e2d}\u{6587}",
            Lang::Hant => "\u{7e41}\u{9ad4}\u{4e2d}\u{6587}",
            Lang::En => "English",
            Lang::Ja => "Nihongo (English)",
            Lang::Ko => "Hangugeo (English)",
            Lang::Ru => "Russkiy (English)",
            Lang::De => "Deutsch (English)",
            Lang::Fr => "Fran\u{e7}ais (English)",
            Lang::Es => "Espanol (English)",
            Lang::Pt => "Portugues (English)",
            Lang::It => "Italiano (English)",
            Lang::Ar => "Al-Arabiyya (English)",
            Lang::Th => "Phasa Thai (English)",
            Lang::Vi => "Tieng Viet (English)",
            Lang::Id => "Bahasa Indonesia (English)",
        }
    }

    /// The language as a locale, which [`Lang::for_locale`] reads back.
    pub fn tag(self) -> &'static str {
        match self {
            Lang::Hans => "zh-Hans",
            Lang::Hant => "zh-Hant",
            Lang::En => "en",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Ru => "ru",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::Pt => "pt",
            Lang::It => "it",
            Lang::Ar => "ar",
            Lang::Th => "th",
            Lang::Vi => "vi",
            Lang::Id => "id",
        }
    }

    /// Of the same thing written in each language, in the order of
    /// [`Lang::ALL`], the one in this language.
    pub fn pick<T>(self, [hans, hant, en]: [T; 3]) -> T {
        match self {
            Lang::Hans => hans,
            Lang::Hant => hant,
            Lang::En | Lang::Ja | Lang::Ko | Lang::Ru | Lang::De | Lang::Fr | Lang::Es | Lang::Pt | Lang::It | Lang::Ar | Lang::Th | Lang::Vi | Lang::Id => en,
        }
    }

    /// The language for a locale: the `zh-CN` a system is built to start
    /// in, or the one it hands the recovery with the work it asks for,
    /// which older systems write `zh_CN`. Chinese in that script or, with
    /// none named, in the one Taiwan, Hong Kong and Macau write in
    /// traditional characters, any other Chinese in simplified ones, and
    /// every other language in English.
    pub fn for_locale(tag: &str) -> Result<Lang> {
        // ICU takes a hyphen alone between the parts of a locale.
        let locale = Locale::try_from_str(&tag.replace('_', "-")).with_context(|| format!("read the locale {tag:?}"))?;
        let id = locale.id;
        // The languages the pages answer in by themselves. The rest answer
        // in English through `pick`, and a locale that names one of them is
        // still worth its own language on the page, for when its texts come.
        match id.language.as_str() {
            "zh" => {}
            "ja" => return Ok(Lang::Ja),
            "ko" => return Ok(Lang::Ko),
            "ru" => return Ok(Lang::Ru),
            "de" => return Ok(Lang::De),
            "fr" => return Ok(Lang::Fr),
            "es" => return Ok(Lang::Es),
            "pt" => return Ok(Lang::Pt),
            "it" => return Ok(Lang::It),
            "ar" => return Ok(Lang::Ar),
            "th" => return Ok(Lang::Th),
            "vi" => return Ok(Lang::Vi),
            "id" => return Ok(Lang::Id),
            _ => return Ok(Lang::En),
        }
        let traditional = match id.script {
            Some(script) => script.as_str() == "Hant",
            None => id.region.is_some_and(|r| matches!(r.as_str(), "TW" | "HK" | "MO")),
        };
        Ok(if traditional { Lang::Hant } else { Lang::Hans })
    }
}

/// A text written in each language, made as [`format!`] makes it of the
/// values it names:
/// `tr!(l, "挂载 {base} 失败", "掛載 {base} 失敗", "Couldn't mount {base}")`.
#[macro_export]
macro_rules! tr {
    ($l:expr, $hans:literal, $hant:literal, $en:literal $(,)?) => {
        match $l {
            $crate::lang::Lang::Hans => format!($hans),
            $crate::lang::Lang::Hant => format!($hant),
            $crate::lang::Lang::En
            | $crate::lang::Lang::Ja
            | $crate::lang::Lang::Ko
            | $crate::lang::Lang::Ru
            | $crate::lang::Lang::De
            | $crate::lang::Lang::Fr
            | $crate::lang::Lang::Es
            | $crate::lang::Lang::Pt
            | $crate::lang::Lang::It
            | $crate::lang::Lang::Ar
            | $crate::lang::Lang::Th
            | $crate::lang::Lang::Vi
            | $crate::lang::Lang::Id => format!($en),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{TokenStream, TokenTree};
    use std::fs;
    use std::path::Path;
    use syn::visit::Visit;

    /// The texts a source file writes: its string literals, those among a
    /// macro's arguments too.
    #[derive(Default)]
    struct Texts(Vec<String>);

    impl Texts {
        fn tokens(&mut self, tokens: TokenStream) {
            for tree in tokens {
                match tree {
                    TokenTree::Literal(lit) => {
                        if let syn::Lit::Str(s) = syn::Lit::new(lit) {
                            self.0.push(s.value());
                        }
                    }
                    TokenTree::Group(group) => self.tokens(group.stream()),
                    TokenTree::Ident(_) | TokenTree::Punct(_) => {}
                }
            }
        }
    }

    impl<'ast> Visit<'ast> for Texts {
        fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
            self.0.push(lit.value());
        }

        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            self.tokens(mac.tokens.clone());
        }
    }

    #[test]
    fn the_typeface_draws_every_text_the_pages_write() {
        let f = crate::font::bundled();
        let mut texts = Texts::default();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let file = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
            texts.visit_file(&file);
        }
        assert!(texts.0.iter().any(|t| t.contains("簽章")), "the texts in traditional characters are read");
        // The language page names all fifteen in their own scripts, and
        // MiSans draws the scripts of the three the pages speak whole. The
        // twelve whose texts are being written answer in English through
        // `pick`, and their names are written in Latin beside the English
        // they answer in, so the page never shows a box for a character the
        // carried face has no shape for.
        let latin_only: Vec<char> = texts.0.iter().flat_map(|t| t.chars()).filter(|&ch| !ch.is_control() && !ch.is_ascii() && !f.body.has(ch)).collect();
        assert!(latin_only.is_empty(), "MiSans draws none of {latin_only:?}");
        assert!(f.body.has('a') && f.body.has('Z'), "the Latin the twelve are named in is drawn");
    }

    #[test]
    fn a_locale_picks_the_language_and_its_characters() {
        let of = |tag| Lang::for_locale(tag).unwrap();
        assert_eq!(of("zh-CN"), Lang::Hans, "a China system, as the 17 Ultra's");
        assert_eq!(of("zh_CN"), Lang::Hans, "as older systems write it");
        assert_eq!(of("zh-SG"), Lang::Hans);
        assert_eq!(of("zh"), Lang::Hans);
        assert_eq!(of("zh-TW"), Lang::Hant);
        assert_eq!(of("zh-HK"), Lang::Hant);
        assert_eq!(of("zh-Hant-TW"), Lang::Hant);
        assert_eq!(of("zh-Hans-HK"), Lang::Hans, "a script named wins over the region");
        assert_eq!(of("en-US"), Lang::En);
        // The twelve whose texts are being written pick themselves now, and
        // answer in English through `pick` until those are written.
        assert_eq!(of("ja-JP"), Lang::Ja, "a language the pages name, answering in English for now");
        assert_eq!(of("ko-KR"), Lang::Ko);
        assert_eq!(of("ru-RU"), Lang::Ru);
        assert_eq!(of("fr-CA"), Lang::Fr);
        assert!(Lang::for_locale("not a locale").is_err());
    }

    #[test]
    fn each_language_s_locale_reads_back_as_it() {
        for lang in Lang::ALL {
            assert_eq!(Lang::for_locale(lang.tag()).unwrap(), lang);
        }
    }

    #[test]
    fn a_text_is_picked_and_made_in_the_language() {
        assert_eq!(Lang::Hant.pick(["设置", "設定", "Settings"]), "設定");
        let base = "vendor";
        assert_eq!(tr!(Lang::En, "挂载 {base} 失败", "掛載 {base} 失敗", "Couldn't mount {base}"), "Couldn't mount vendor");
                let names = Lang::ALL.map(Lang::name);
        assert_eq!(&names[..3], ["\u{7b80}\u{4f53}\u{4e2d}\u{6587}", "\u{7e41}\u{9ad4}\u{4e2d}\u{6587}", "English"]);
        for name in &names[3..] {
            assert!(name.ends_with("(English)"), "{name} says which tongue it answers in");
        }
        // And a language whose texts are not written picks the English one,
        // exactly as the name beside it says.
        assert_eq!(Lang::Ja.pick(["\u{8bbe}\u{7f6e}", "\u{8a2d}\u{5b9a}", "Settings"]), "Settings");
    }
}
