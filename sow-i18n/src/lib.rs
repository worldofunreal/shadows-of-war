use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const WEB_CATALOG_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Italian,
    Turkish,
    SimplifiedChinese,
    Japanese,
    Korean,
    Arabic,
    Russian,
    Vietnamese,
    Indonesian,
    Filipino,
    BrazilianPortuguese,
}

impl Language {
    /// `(language, BCP-47 code, native display name, enabled for players)`.
    /// This is the only language registry consumed by the web bundles.
    pub const fn registry() -> &'static [(Language, &'static str, &'static str, bool)] {
        &[
            (Language::English, "en", "English", true),
            (Language::Spanish, "es", "Español", true),
            (Language::French, "fr", "Français", true),
            (Language::German, "de", "Deutsch", true),
            (Language::Italian, "it", "Italiano", true),
            (Language::Turkish, "tr", "Türkçe", true),
            (Language::SimplifiedChinese, "zh-CN", "简体中文", true),
            (Language::Japanese, "ja", "日本語", true),
            (Language::Korean, "ko", "한국어", true),
            (Language::Arabic, "ar", "العربية", true),
            (Language::Russian, "ru", "Русский", true),
            (Language::Vietnamese, "vi", "Tiếng Việt", true),
            (Language::Indonesian, "id", "Bahasa Indonesia", true),
            (Language::Filipino, "fil", "Filipino", true),
            (
                Language::BrazilianPortuguese,
                "pt-BR",
                "Português (Brasil)",
                true,
            ),
        ]
    }

    pub fn published_registry() -> impl Iterator<Item = (Language, &'static str, &'static str)> {
        Self::registry()
            .iter()
            .filter(|(_, _, _, published)| *published)
            .map(|(language, code, name, _)| (*language, *code, *name))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebStrings {
    pub menu: WebDomain,
    pub auth: WebDomain,
    pub lobbies: WebDomain,
    pub heroes: WebDomain,
    pub profile: WebDomain,
    pub store: WebDomain,
    pub hud: WebDomain,
    pub endgame: WebDomain,
    pub tutorial: WebDomain,
    pub site: WebDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDomain {
    #[serde(flatten)]
    pub values: BTreeMap<String, String>,
}

static EN_WEB: OnceLock<WebStrings> = OnceLock::new();
static ES_WEB: OnceLock<WebStrings> = OnceLock::new();
static FR_WEB: OnceLock<WebStrings> = OnceLock::new();
static DE_WEB: OnceLock<WebStrings> = OnceLock::new();
static IT_WEB: OnceLock<WebStrings> = OnceLock::new();
static TR_WEB: OnceLock<WebStrings> = OnceLock::new();
static ZH_CN_WEB: OnceLock<WebStrings> = OnceLock::new();
static JA_WEB: OnceLock<WebStrings> = OnceLock::new();
static KO_WEB: OnceLock<WebStrings> = OnceLock::new();
static AR_WEB: OnceLock<WebStrings> = OnceLock::new();
static RU_WEB: OnceLock<WebStrings> = OnceLock::new();
static VI_WEB: OnceLock<WebStrings> = OnceLock::new();
static ID_WEB: OnceLock<WebStrings> = OnceLock::new();
static FIL_WEB: OnceLock<WebStrings> = OnceLock::new();
static PT_BR_WEB: OnceLock<WebStrings> = OnceLock::new();

pub fn web(language: Language) -> &'static WebStrings {
    match language {
        Language::English => EN_WEB.get_or_init(|| parse(include_str!("../strings/en/web.toml"))),
        Language::Spanish => ES_WEB.get_or_init(|| parse(include_str!("../strings/es/web.toml"))),
        Language::French => FR_WEB.get_or_init(|| parse(include_str!("../strings/fr/web.toml"))),
        Language::German => DE_WEB.get_or_init(|| parse(include_str!("../strings/de/web.toml"))),
        Language::Italian => IT_WEB.get_or_init(|| parse(include_str!("../strings/it/web.toml"))),
        Language::Turkish => TR_WEB.get_or_init(|| parse(include_str!("../strings/tr/web.toml"))),
        Language::SimplifiedChinese => {
            ZH_CN_WEB.get_or_init(|| parse(include_str!("../strings/zh-cn/web.toml")))
        }
        Language::Japanese => JA_WEB.get_or_init(|| parse(include_str!("../strings/ja/web.toml"))),
        Language::Korean => KO_WEB.get_or_init(|| parse(include_str!("../strings/ko/web.toml"))),
        Language::Arabic => AR_WEB.get_or_init(|| parse(include_str!("../strings/ar/web.toml"))),
        Language::Russian => RU_WEB.get_or_init(|| parse(include_str!("../strings/ru/web.toml"))),
        Language::Vietnamese => {
            VI_WEB.get_or_init(|| parse(include_str!("../strings/vi/web.toml")))
        }
        Language::Indonesian => {
            ID_WEB.get_or_init(|| parse(include_str!("../strings/id/web.toml")))
        }
        Language::Filipino => {
            FIL_WEB.get_or_init(|| parse(include_str!("../strings/fil/web.toml")))
        }
        Language::BrazilianPortuguese => {
            PT_BR_WEB.get_or_init(|| parse(include_str!("../strings/pt-br/web.toml")))
        }
    }
}

fn parse(catalog: &str) -> WebStrings {
    toml::from_str(catalog).expect("failed to parse web locale catalog")
}
