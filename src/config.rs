use crate::prelude::*;

use derive_more::Into;
use serde::{Deserialize, Serialize};
use serenity::UserId;
use std::{fs::read_to_string, sync::LazyLock};

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let path = std::env::var("CONFIG_FILE").expect("CONFIG_FILE environment variable existing");
    let string = read_to_string(path).expect("CONFIG_FILE pointing to a readable file");
    Config::load(string).expect("CONFIG_FILE pointing to a valid config")
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
    fn load(input: impl AsRef<str>) -> Result<Self> {
        Ok(ron::from_str::<Self>(input.as_ref())?)
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
    #[allow(clippy::missing_const_for_fn)] // no it can't
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
        Self("gpt-4o-mini".into())
    }
}

pub fn substitute_name(input: impl AsRef<str>) -> String {
    CONFIG
        .name_substitutes
        .0
        .iter()
        .find(|(from, _)| input.as_ref() == from)
        .map_or_else(|| "User".into(), |(_, to)| to.into())
}
