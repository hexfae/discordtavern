use crate::commands::{answer_as::svara_som, chat::prata, gubbar::gubbar, gubbe::gubbe};
use crate::event_handler::Handler;
use crate::prelude::*;
use crate::statistics::Statistics;
use async_openai::{Client, config::OpenAIConfig};
use dashmap::DashMap;
use miette::Diagnostic;
use poise::PrefixFrameworkOptions;
use poise::serenity_prelude::{
    ActivityData, ActivityType, AutocompleteChoice, CreateAutocompleteResponse, MessageId,
};
use poise::{
    Framework, FrameworkOptions,
    serenity_prelude::{ClientBuilder, GatewayIntents, Message},
};
use ron::ser::PrettyConfig;
use snafu::{ResultExt, Snafu};
use std::fs::{read, write};
use std::sync::Arc;
use tracing::error;

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
        self.characters.iter().map(|c| c.clone()).collect()
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
        let characters: DashMap<String, Character> = read("characters.ron").map_or_else(
            |_| DashMap::new(),
            |bytes| ron::de::from_bytes(&bytes).expect("valid characters file"),
        );
        let chats = read("chats.ron").map_or_else(
            |_| DashMap::new(),
            |bytes| ron::de::from_bytes(&bytes).expect("valid chats file"),
        );
        let config = OpenAIConfig::default()
            .with_api_key(CONFIG.openai_key())
            .with_api_base(CONFIG.openai_url());
        let ai = Client::with_config(config);

        let statistics = Statistics::from(&chats);
        for mut character in characters.iter_mut() {
            let name = character.key();
            if let Some(statistic) = statistics.characters.get(name) {
                character.times_spawned = statistic.times_spawned;
            }
        }

        Self {
            characters,
            chats,
            ai,
        }
    }

    pub fn save(&self) {
        let serialized_characters =
            ron::ser::to_string_pretty(&self.characters, PrettyConfig::new());
        let serialized_chats = ron::ser::to_string_pretty(&self.chats, PrettyConfig::default());

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
    let bot_token = CONFIG.bot_token().parse()?;

    let bot_commands = vec![prata(), gubbe(), gubbar(), svara_som(), register()];

    let framework_options = FrameworkOptions {
        commands: bot_commands,
        prefix_options: PrefixFrameworkOptions {
            prefix: Some(std::borrow::Cow::Borrowed("+")),
            ..Default::default()
        },
        on_error: |error| Box::pin(error_handler(error)),
        ..Default::default()
    };

    let framework = Framework::builder().options(framework_options).build();

    ClientBuilder::new(bot_token, GATEWAY_INTENTS)
        .framework(framework)
        .event_handler(Handler)
        .activity(ActivityData {
            name: "Heroes of the Storm".parse().expect("invalid str length"),
            kind: ActivityType::Playing,
            state: Some("0-1-13".parse().expect("invalid str length")),
            url: None,
        })
        .data(Arc::new(data))
        .await?
        .start()
        .await?;

    Ok(())
}

#[derive(Debug, Snafu, Diagnostic)]
struct RegisterError {
    source: poise::serenity_prelude::Error,
}

#[poise::command(prefix_command)]
async fn register(ctx: Context<'_>) -> Result<()> {
    poise::builtins::register_application_commands_buttons(ctx)
        .await
        .context(RegisterSnafu)?;
    Ok(())
}

async fn error_handler(error: FrameworkError<'_>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            let log_channel = CONFIG.log_channel();
            let command_name = &ctx.command().name;
            let error_message = format!("error in command: `{command_name}`: {error:?}");
            if let Err(why) = log_channel.say(ctx.http(), &error_message).await {
                error!("Error sending error message: {why}");
            }
            ctx.reply(error_message).await.expect("a");
        }
        other => {
            if let Err(why) = poise::builtins::on_error(other).await {
                error!("Error while handling error: {why}");
            }
        }
    }
}

pub async fn autocomplete_character_name<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> CreateAutocompleteResponse<'a> {
    let mut characters = ctx.data().characters();
    characters.sort_unstable();
    characters.reverse();

    let character_names = characters
        .into_iter()
        .filter(|character| {
            character
                .name
                .to_string()
                .to_lowercase()
                .starts_with(&partial.to_lowercase())
        })
        .take(25)
        .map(|character| AutocompleteChoice::new(character.to_string(), character.name.to_string()))
        .collect::<Vec<AutocompleteChoice>>();
    CreateAutocompleteResponse::new().set_choices(character_names)
}
