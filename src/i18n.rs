//! UI translations plus the shared Neko Legends language packs.
//!
//! MarkRush ships English only. Fonts for other scripts are installed once by
//! the Neko Legends Control Center into a shared folder (see
//! `neko-legends-control-center/languages/README.md`):
//!
//! ```text
//! <config dir>/NekoLegends/languages/<id>/pack.json
//! ```
//!
//! Every installed pack is added as a fallback font, so documents in those
//! scripts render even while the UI stays English. A UI language becomes
//! selectable once its pack is installed. The UI strings themselves are small
//! and app specific, so they are embedded here.

use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock},
};

pub const ENGLISH: &str = "en";

pub struct UiLanguage {
    pub code: &'static str,
    pub native_name: &'static str,
    pub english_name: &'static str,
    strings: &'static str,
}

/// UI translations shipped with MarkRush. Arabic is intentionally absent:
/// egui has no right-to-left shaping yet, so its pack is only used as a
/// document fallback font.
pub const UI_LANGUAGES: &[UiLanguage] = &[
    UiLanguage {
        code: "ja",
        native_name: "日本語",
        english_name: "Japanese",
        strings: include_str!("../assets/i18n/ja.json"),
    },
    UiLanguage {
        code: "zh-Hans",
        native_name: "简体中文",
        english_name: "Chinese (Simplified)",
        strings: include_str!("../assets/i18n/zh-Hans.json"),
    },
    UiLanguage {
        code: "ko",
        native_name: "한국어",
        english_name: "Korean",
        strings: include_str!("../assets/i18n/ko.json"),
    },
    UiLanguage {
        code: "ru",
        native_name: "Русский",
        english_name: "Russian",
        strings: include_str!("../assets/i18n/ru.json"),
    },
    UiLanguage {
        code: "th",
        native_name: "ไทย",
        english_name: "Thai",
        strings: include_str!("../assets/i18n/th.json"),
    },
];

/// Preferred fallback order when several packs cover the same characters
/// (Han characters exist in the JP, SC and KR fonts with different shapes).
const FALLBACK_ORDER: &[&str] = &["ja", "zh-Hans", "ko", "ru", "th", "ar"];

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePack {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub native_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub rtl: bool,
    #[serde(default)]
    pub complex_shaping: bool,
    pub fonts: Vec<PackFont>,
    #[serde(skip)]
    pub dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PackFont {
    pub file: String,
    #[serde(default)]
    pub family: String,
}

static ACTIVE_STRINGS: OnceLock<RwLock<Option<HashMap<String, String>>>> = OnceLock::new();

fn active_strings() -> &'static RwLock<Option<HashMap<String, String>>> {
    ACTIVE_STRINGS.get_or_init(|| RwLock::new(None))
}

/// Translate a UI string. The English text is the key, so untranslated
/// strings fall back to English automatically.
pub fn tr(text: &str) -> String {
    active_strings()
        .read()
        .ok()
        .and_then(|strings| strings.as_ref()?.get(text).cloned())
        .unwrap_or_else(|| text.to_owned())
}

/// Translate a template with `{0}`, `{1}` ... placeholders, then fill them.
pub fn trf(template: &str, args: &[&dyn Display]) -> String {
    fill_template(&tr(template), args)
}

fn fill_template(template: &str, args: &[&dyn Display]) -> String {
    let mut output = template.to_owned();
    for (index, arg) in args.iter().enumerate() {
        output = output.replace(&format!("{{{index}}}"), &arg.to_string());
    }
    output
}

pub fn ui_language(code: &str) -> Option<&'static UiLanguage> {
    UI_LANGUAGES.iter().find(|language| language.code == code)
}

fn parse_strings(language: &UiLanguage) -> HashMap<String, String> {
    serde_json::from_str::<HashMap<String, String>>(language.strings)
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, value)| !key.starts_with('_') && !value.trim().is_empty())
        .collect()
}

/// Switch the UI language. Returns the code that is actually active, which
/// is English when the language is unknown or its font pack is missing.
pub fn apply_ui_language(code: &str, packs: &[LanguagePack]) -> &'static str {
    let selected = ui_language(code).filter(|_| packs.iter().any(|pack| pack.id == code));
    let strings = selected.map(parse_strings);
    if let Ok(mut active) = active_strings().write() {
        *active = strings;
    }
    selected.map_or(ENGLISH, |language| language.code)
}

/// `<config dir>/NekoLegends/languages`. This matches the Control Center:
/// %APPDATA% on Windows, ~/Library/Application Support on macOS and
/// $XDG_CONFIG_HOME (or ~/.config) on Linux.
pub fn shared_languages_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("NekoLegends").join("languages"))
}

pub fn scan_language_packs() -> Vec<LanguagePack> {
    shared_languages_dir()
        .map(|root| scan_language_packs_in(&root))
        .unwrap_or_default()
}

pub fn scan_language_packs_in(root: &Path) -> Vec<LanguagePack> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut packs: Vec<LanguagePack> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.is_dir())
        .filter(|dir| {
            dir.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
        })
        .filter_map(|dir| {
            let raw = fs::read_to_string(dir.join("pack.json")).ok()?;
            let mut pack: LanguagePack = serde_json::from_str(&raw).ok()?;
            let complete = !pack.fonts.is_empty()
                && pack.fonts.iter().all(|font| {
                    is_plain_file_name(&font.file) && dir.join(&font.file).is_file()
                });
            complete.then(|| {
                pack.dir = dir;
                pack
            })
        })
        .collect();
    packs.sort_by_key(|pack| fallback_rank(&pack.id, ""));
    packs
}

fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

fn fallback_rank(id: &str, preferred: &str) -> (usize, usize, String) {
    let preferred_rank = usize::from(id != preferred);
    let order = FALLBACK_ORDER
        .iter()
        .position(|candidate| *candidate == id)
        .unwrap_or(FALLBACK_ORDER.len());
    (preferred_rank, order, id.to_owned())
}

/// Identity of the installed packs, used to notice installs and removals.
pub fn packs_fingerprint(packs: &[LanguagePack]) -> Vec<(String, String)> {
    packs
        .iter()
        .map(|pack| (pack.id.clone(), pack.version.clone()))
        .collect()
}

/// Append every installed pack font as a fallback to every font family, so
/// the existing UI and document fonts keep priority for the characters they
/// already cover. The active UI language's pack goes first among the packs.
pub fn add_language_pack_fonts(
    fonts: &mut egui::FontDefinitions,
    packs: &[LanguagePack],
    preferred: &str,
) {
    let mut ordered: Vec<&LanguagePack> = packs.iter().collect();
    ordered.sort_by_key(|pack| fallback_rank(&pack.id, preferred));

    let mut keys = Vec::new();
    for pack in ordered {
        for (index, font) in pack.fonts.iter().enumerate() {
            let Ok(bytes) = fs::read(pack.dir.join(&font.file)) else {
                continue;
            };
            let key = format!("neko-language-{}-{index}", pack.id);
            fonts
                .font_data
                .insert(key.clone(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            keys.push(key);
        }
    }

    for family_fonts in fonts.families.values_mut() {
        for key in &keys {
            if !family_fonts.contains(key) {
                family_fonts.push(key.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "markrush-i18n-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_pack(root: &Path, id: &str, font_file: &str, create_font: bool) {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        if create_font {
            fs::write(dir.join(font_file), b"not really a font").unwrap();
        }
        fs::write(
            dir.join("pack.json"),
            format!(
                r#"{{"schemaVersion":1,"id":"{id}","name":"{id}","nativeName":"{id}","version":"26.10.21","fonts":[{{"file":"{font_file}","family":"Test"}}]}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn translations_cover_the_same_keys() {
        let english_keys: Vec<String> = {
            let mut keys: Vec<String> = parse_strings(&UI_LANGUAGES[0]).into_keys().collect();
            keys.sort();
            keys
        };
        assert!(english_keys.len() > 50);
        for language in UI_LANGUAGES {
            let strings = parse_strings(language);
            let mut keys: Vec<String> = strings.keys().cloned().collect();
            keys.sort();
            assert_eq!(keys, english_keys, "{} keys differ", language.code);
            for (key, value) in &strings {
                for index in 0..4 {
                    let placeholder = format!("{{{index}}}");
                    assert_eq!(
                        key.contains(&placeholder),
                        value.contains(&placeholder),
                        "{} placeholder mismatch for {key:?}",
                        language.code
                    );
                }
            }
        }
    }

    #[test]
    fn template_fill_and_fallback() {
        assert_eq!(fill_template("{0} of {1}", &[&3, &9]), "3 of 9");
        assert_eq!(tr("A string nobody translated"), "A string nobody translated");
    }

    #[test]
    fn scans_complete_packs_only_and_gates_ui_language() {
        let root = temp_root("scan");
        write_pack(&root, "ko", "ko.ttf", true);
        write_pack(&root, "ja", "ja.ttf", true);
        write_pack(&root, "ru", "missing.ttf", false);
        write_pack(&root, "th", "../escape.ttf", true);
        fs::create_dir_all(root.join(".zh-Hans.partial")).unwrap();

        let packs = scan_language_packs_in(&root);
        let ids: Vec<&str> = packs.iter().map(|pack| pack.id.as_str()).collect();
        assert_eq!(ids, ["ja", "ko"]);

        assert_eq!(apply_ui_language("ru", &packs), ENGLISH);
        assert_eq!(apply_ui_language("ja", &packs), "ja");
        assert_ne!(tr("Open Folder"), "Open Folder");
        assert_eq!(apply_ui_language(ENGLISH, &packs), ENGLISH);
        assert_eq!(tr("Open Folder"), "Open Folder");

        let mut fonts = egui::FontDefinitions::default();
        add_language_pack_fonts(&mut fonts, &packs, "ko");
        let proportional = &fonts.families[&egui::FontFamily::Proportional];
        let ko = proportional.iter().position(|key| key == "neko-language-ko-0").unwrap();
        let ja = proportional.iter().position(|key| key == "neko-language-ja-0").unwrap();
        assert!(ko < ja, "preferred pack should come first");
        assert!(ja > 0, "pack fonts are fallbacks, not primary fonts");

        let _ = fs::remove_dir_all(&root);
    }
}
