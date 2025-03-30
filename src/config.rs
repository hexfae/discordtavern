use crate::prelude::*;

use derive_more::Into;
use poise::serenity_prelude::ChannelId;
use serde::{Deserialize, Serialize};
use std::{fs::read_to_string, sync::LazyLock};

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let path = std::env::var("CONFIG_FILE").expect("CONFIG_FILE environment variable existing");
    let string = read_to_string(path).expect("CONFIG_FILE pointing to a readable file");
    Config::load(string).expect("CONFIG_FILE pointing to a valid config")
});

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
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
    #[serde(default)]
    log_channel: ChannelId,
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

    pub fn bot_token(&self) -> String {
        self.bot_token.0.clone()
    }

    pub fn openai_url(&self) -> String {
        self.openai_url.0.clone()
    }

    pub fn openai_key(&self) -> String {
        self.openai_key.0.clone()
    }

    pub fn openai_model(&self) -> String {
        self.openai_model.0.clone()
    }

    pub fn name_substitutes(&self) -> Vec<(String, String)> {
        self.name_substitutes.0.clone()
    }

    pub const fn log_channel(&self) -> ChannelId {
        self.log_channel
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
        Self("gpt-4o-mini".into())
    }
}
