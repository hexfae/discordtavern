use crate::prelude::*;

use derive_more::Into;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
use serenity::UserId;
use std::fs::{read_to_string, write};
use tracing::warn;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    read_to_string("config.ron").map_or_else(
        |_| Config::create(),
        |string| match Config::load(string) {
            Ok(config) => config,
            Err(why) => panic!("{}", why),
        },
    )
});

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    bot_id: UserId,
    #[serde(default)]
    bot_token: BotToken,
    #[serde(default)]
    openai_url: OpenAiUrl,
    #[serde(default)]
    openai_key: OpenAiKey,
    #[serde(default)]
    openai_model: OpenAiModel,
    #[serde(default)]
    name_substitutes: NameSubstitutes,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Into)]
pub struct BotToken(pub String);

#[derive(Debug, Serialize, Deserialize, Clone, Into)]
pub struct OpenAiUrl(pub String);

#[derive(Debug, Default, Serialize, Deserialize, Clone, Into)]
pub struct OpenAiKey(pub String);

#[derive(Debug, Serialize, Deserialize, Clone, Into)]
pub struct OpenAiModel(pub String);

#[derive(Debug, Default, Serialize, Deserialize, Clone, Into)]
pub struct NameSubstitutes(pub Vec<(String, String)>);

impl Config {
    fn new() -> RwLock<Self> {
        RwLock::new(Self::default())
    }

    fn create() -> RwLock<Self> {
        let config = Self::new();
        if let Err(why) = config.read().save() {
            warn!("could not save config! {why}");
        };
        config
    }

    fn load(input: impl AsRef<str>) -> Result<RwLock<Self>> {
        let config = ron::from_str::<Self>(input.as_ref())?;
        if let Err(why) = config.save() {
            warn!("could not save config! {why}");
        };
        Ok(RwLock::new(config))
    }

    fn save(&self) -> Result<()> {
        Ok(write(
            "config.ron",
            to_string_pretty(self, PrettyConfig::default())?,
        )?)
    }

    #[inline]
    pub const fn bot_id(&self) -> UserId {
        self.bot_id
    }

    #[inline]
    pub fn bot_token(&self) -> BotToken {
        self.bot_token.clone()
    }

    #[inline]
    pub fn openai_url(&self) -> OpenAiUrl {
        self.openai_url.clone()
    }

    #[inline]
    pub fn openai_key(&self) -> OpenAiKey {
        self.openai_key.clone()
    }

    #[inline]
    pub fn openai_model(&self) -> OpenAiModel {
        self.openai_model.clone()
    }
}

impl BotToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for OpenAiUrl {
    fn default() -> Self {
        Self("https://api.openai.com/v1".into())
    }
}

impl Default for OpenAiModel {
    fn default() -> Self {
        Self("gpt-3.5-turbo-1106".into())
    }
}

pub fn substitute_name(input: impl AsRef<str>) -> String {
    CONFIG
        .read()
        .name_substitutes
        .0
        .iter()
        .find(|(from, _)| input.as_ref() == from)
        .map_or_else(|| "User".into(), |(_, to)| to.into())
}
