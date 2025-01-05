#![allow(clippy::unreadable_literal)]
use crate::prelude::*;
use crate::{
    commands::{chat::prata, gubbar::gubbar, gubbe::gubbe},
    event_handler::event_handler,
};
use async_openai::{config::OpenAIConfig, Client};
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use itertools::Itertools;
use poise::serenity_prelude::{ActivityData, ActivityType, MessageId};
use poise::PrefixFrameworkOptions;
use poise::{
    serenity_prelude::{ClientBuilder, GatewayIntents, Message},
    Framework, FrameworkOptions,
};
use small_fixed_array::FixedString;
use std::fs::{read, write};
use std::sync::Arc;

const GATEWAY_INTENTS: GatewayIntents =
    GatewayIntents::non_privileged().union(GatewayIntents::MESSAGE_CONTENT);

#[derive(Debug)]
pub struct Data {
    pub characters: DashMap<String, Character>,
    pub chats: DashMap<MessageId, History>,
    pub ai: Client<OpenAIConfig>,
}

impl Data {
    pub async fn start_bot() -> Result<()> {
        let data = Self::load();
        start_bot(data).await?;
        Ok(())
    }

    pub fn character(&self, character_name: &str) -> Option<Character> {
        self.characters.get(character_name).map(|c| c.clone())
    }

    pub fn characters(&self) -> Vec<Character> {
        self.characters.iter().map(|c| c.clone()).collect_vec()
    }

    pub fn history(&self, message: &Message) -> Option<History> {
        self.chats.get(&message.id).map(|c| c.clone())
    }

    pub fn insert_history(&self, history: History) {
        self.chats.insert(history.id, history);
        self.save();
    }

    pub fn insert_character(&self, character: Character) {
        let character_name = character.name.to_string();
        self.characters.insert(character_name, character);
        self.save();
    }

    pub fn remove_character(&self, character_name: &str) -> Option<()> {
        self.characters.remove(character_name).map(|_| ())
    }

    pub fn load() -> Self {
        let characters = read("characters.ron").map_or_else(
            |_| DashMap::new(),
            |bytes| ron::de::from_bytes(&bytes).expect("valid characters file"),
        );
        let chats = read("chats.ron").map_or_else(
            |_| DashMap::new(),
            |bytes| ron::de::from_bytes(&bytes).expect("valid chats file"),
        );
        let config = OpenAIConfig::default()
            .with_api_key(CONFIG.read().openai_key())
            .with_api_base(CONFIG.read().openai_url());
        let ai = Client::with_config(config);
        Self {
            characters,
            chats,
            ai,
        }
    }

    pub fn save(&self) {
        let serialized_characters = ron::to_string(&self.characters);
        let serialized_chats = ron::to_string(&self.chats);

        match (serialized_characters, serialized_chats) {
            (Ok(characters), Ok(chats)) => {
                write("characters.ron", characters).ok();
                write("chats.ron", chats).ok();
            }
            _ => {
                tracing::warn!("Failed to serialize characters or chats.");
            }
        }
    }
}

async fn start_bot(data: Data) -> Result<()> {
    let bot_token = CONFIG.read().bot_token();

    let bot_commands = vec![prata(), gubbe(), gubbar(), register()];

    let framework_options = FrameworkOptions {
        commands: bot_commands,
        prefix_options: PrefixFrameworkOptions {
            prefix: Some(std::borrow::Cow::Borrowed("+")),
            ..Default::default()
        },
        event_handler: |ctx, event| Box::pin(event_handler(ctx, event)),
        on_error: |error| Box::pin(error_handler(error)),
        ..Default::default()
    };

    let framework = Framework::builder().options(framework_options).build();

    ClientBuilder::new(bot_token.as_str(), GATEWAY_INTENTS)
        .framework(framework)
        .activity(ActivityData {
            name: FixedString::from_str_trunc("Heroes of the Storm"),
            kind: ActivityType::Playing,
            state: Some(FixedString::from_str_trunc("0-1-13")),
            url: None,
        })
        .data(Arc::new(data))
        .await?
        .start()
        .await?;

    Ok(())
}

#[poise::command(prefix_command)]
async fn register(ctx: Context<'_>) -> Result<()> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

async fn error_handler(error: FrameworkError<'_>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            let command_name = &ctx.command().name;
            let error_message = format!("error in command: `{command_name}`: {error:?}");
            ctx.reply(error_message).await.expect("a");
        }
        error => {
            if let Err(error) = poise::builtins::on_error(error).await {
                tracing::error!("Error while handling error: {error}");
            }
        }
    };
}

pub async fn autocomplete_character_name<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let character_names = ctx
        .data()
        .characters()
        .into_iter()
        .map(|a| a.name.to_string())
        .collect_vec();
    futures::stream::iter(character_names).filter(move |name| {
        futures::future::ready(name.to_lowercase().starts_with(&partial.to_lowercase()))
    })
}
