use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const WEB_CATALOG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Italian,
    Turkish,
}

impl Language {
    pub const fn registry() -> &'static [(Language, &'static str, &'static str)] {
        &[
            (Language::English, "en", "English"),
            (Language::Spanish, "es", "Español"),
            (Language::French, "fr", "Français"),
            (Language::German, "de", "Deutsch"),
            (Language::Italian, "it", "Italiano"),
            (Language::Turkish, "tr", "Türkçe"),
        ]
    }

    pub const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::French => "fr",
            Language::German => "de",
            Language::Italian => "it",
            Language::Turkish => "tr",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Italian => "Italiano",
            Language::Turkish => "Türkçe",
        }
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

pub fn web(language: Language) -> &'static WebStrings {
    match language {
        Language::English => EN_WEB.get_or_init(|| parse(include_str!("../strings/en/web.toml"))),
        Language::Spanish => ES_WEB.get_or_init(|| parse(include_str!("../strings/es/web.toml"))),
        Language::French => FR_WEB.get_or_init(|| parse(include_str!("../strings/fr/web.toml"))),
        Language::German => DE_WEB.get_or_init(|| parse(include_str!("../strings/de/web.toml"))),
        Language::Italian => IT_WEB.get_or_init(|| parse(include_str!("../strings/it/web.toml"))),
        Language::Turkish => TR_WEB.get_or_init(|| parse(include_str!("../strings/tr/web.toml"))),
    }
}

fn parse(catalog: &str) -> WebStrings {
    toml::from_str(catalog).expect("failed to parse web locale catalog")
}
