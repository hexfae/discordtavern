use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::discord::Data;
use crate::prelude::*;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{CreateChatCompletionRequest, CreateChatCompletionRequestArgs};
use futures::StreamExt;
use miette::Diagnostic;
use poise::serenity_prelude::{
    ComponentInteractionCollector, Context, CreateActionRow, CreateEmbed, CreateEmbedAuthor,
    CreateEmbedFooter, CreateInteractionResponse, CreateMessage, EditMessage, EventHandler,
    FullEvent, Http, Mentionable, Message, ReactionType, async_trait,
};
use poise::{Modal, execute_modal_on_component_interaction};
use snafu::{ResultExt, Snafu};
use tracing::{info, instrument, warn};

#[allow(clippy::too_many_lines)]
#[instrument(skip_all)]
pub async fn event_handler(
    ctx: &Context,
    new_message: Message,
    mut history: History,
) -> Result<()> {
    let data = ctx.data::<Data>();
    let http = &ctx.http;
    let super_message = SuperMessage::from(new_message.clone());

    history.push_message(history.choices[history.current_page].clone());
    let log_message = log_user_response(http, &new_message, &super_message, &history).await?;
    history.push_message(super_message);

    let (prev_button_id, next_button_id, pin_button_id, edit_button_id) =
        create_button_ids(&new_message);
    let (enabled_buttons, disabled_buttons) = create_buttons(&new_message);
    let mut message = create_initial_message(http, &history, &new_message).await?;
    let request = create_request(history.clone())?;

    let (response, elapsed) =
        stream_response_edit_message(http, &data.ai, request, &mut message, &history).await?;
    let super_message =
        SuperMessage::new_assistant(history.clone().character.name, response.clone());
    finish_response_edit_message(
        http,
        elapsed,
        enabled_buttons.clone(),
        super_message.clone(),
        &history,
        &mut message,
    )
    .await?;

    log_bot_response(
        http,
        log_message,
        &history.character,
        &super_message,
        elapsed,
    )
    .await?;
    history.reset_choices();
    history.update(super_message.clone(), message.id, elapsed);
    data.insert_history(history.clone());

    let mut current_page: usize = 0;
    while let Some(interaction) = ComponentInteractionCollector::new(ctx)
        .filter(move |interaction| {
            interaction
                .data
                .custom_id
                .starts_with(&new_message.id.to_string())
        })
        .timeout(Duration::from_secs(60 * 60 * 24))
        .await
    {
        if interaction.data.custom_id == pin_button_id {
            let channel_id = new_message.channel_id;
            let character_name = history.character.to_string();
            let message_content = history.choices[current_page].message.to_string();
            let avatar = history.character.avatar.to_string();

            let embed = CreateEmbed::new()
                .title(&character_name)
                .description(&message_content)
                .thumbnail(&avatar);
            let message = CreateMessage::new()
                .embed(embed)
                .reference_message(&new_message);

            let pinned_message = channel_id
                .send_message(http, message)
                .await
                .context(SendMessageSnafu)?;

            pinned_message
                .pin(http, None)
                .await
                .context(PinMessageSnafu)?;

            interaction
                .create_response(http, CreateInteractionResponse::Acknowledge)
                .await
                .context(AcknowledgeSnafu)?;
        } else if interaction.data.custom_id == edit_button_id {
            let footer = format!(
                "{}/{} | tog {}s | {}/4096",
                current_page + 1,
                history.choices.len(),
                &history.seconds_taken[current_page],
                history.choices[current_page].message.len(),
            );
            let user_name = substitute_name(interaction.clone().user.name);
            let name = history.character.to_string();
            let thumbnail = history.character.avatar.to_string();
            message
                .edit(
                    &http,
                    EditMessage::new()
                        .embed(
                            serenity::CreateEmbed::new()
                                .title(name.clone())
                                .field(
                                    "Meddelandet redigeras…",
                                    format!("Meddelandet håller på att redigeras av {user_name}.",),
                                    false,
                                )
                                .description(history.choices[current_page].message.to_string())
                                .thumbnail(thumbnail.clone())
                                .footer(serenity::CreateEmbedFooter::new(footer)),
                        )
                        .components(disabled_buttons.clone()),
                )
                .await
                .context(EditMessageSnafu)?;
            let Some(modal) = execute_modal_on_component_interaction::<EditMessageModal>(
                ctx,
                interaction.clone(),
                None,
                None,
            )
            .await
            .context(ModalSnafu)?
            else {
                message
                    .edit(
                        &http,
                        EditMessage::new().components(enabled_buttons.clone()),
                    )
                    .await
                    .context(EditMessageSnafu)?;
                interaction
                    .create_response(http, CreateInteractionResponse::Acknowledge)
                    .await
                    .context(AcknowledgeSnafu)?;
                continue;
            };

            history.update_choice(&modal.message, current_page);
            data.insert_history(history.clone());

            let footer = format!(
                "{}/{} | tog {}s | {}/4096 (redigerad)",
                current_page + 1,
                history.choices.len(),
                &history.seconds_taken[current_page],
                modal.message.len(),
            );

            let history = history.clone();
            let name = history.character.to_string();
            let description = history.choices[current_page].message.to_string();
            let thumbnail = history.character.avatar.to_string();
            message
                .edit(
                    &http,
                    EditMessage::new()
                        .embed(
                            serenity::CreateEmbed::new()
                                .title(name)
                                .description(description)
                                .thumbnail(thumbnail)
                                .footer(serenity::CreateEmbedFooter::new(footer)),
                        )
                        .components(enabled_buttons.clone()),
                )
                .await
                .context(EditMessageSnafu)?;
        } else if interaction.data.custom_id == prev_button_id {
            interaction.defer(http).await.context(DeferSnafu)?;
            current_page = current_page
                .checked_sub(1)
                .unwrap_or_else(|| &history.choices.len() - 1);
            history.current_page = current_page;

            let name = history.character.to_string();
            let description = history.choices[current_page].message.to_string();
            let thumbnail = history.character.avatar.to_string();
            let footer = format!(
                "{}/{} | tog {}s | {}/4096",
                current_page + 1,
                history.clone().choices.len(),
                &history.clone().seconds_taken[current_page],
                description.len()
            );

            message
                .edit(
                    &http,
                    EditMessage::default()
                        .embed(
                            serenity::CreateEmbed::new()
                                .title(name)
                                .description(description)
                                .thumbnail(thumbnail)
                                .footer(serenity::CreateEmbedFooter::new(footer)),
                        )
                        .components(enabled_buttons.clone()),
                )
                .await
                .context(DeferSnafu)?;
            data.insert_history(history.clone());
        } else if interaction.data.custom_id == next_button_id {
            interaction.defer(http).await.context(DeferSnafu)?;
            current_page += 1;
            history.current_page = current_page;

            if current_page >= history.choices.len() {
                let footer = format!("{}/{}", current_page + 1, history.choices.len() + 1);
                let name = history.character.to_string();
                let thumbnail = history.character.avatar.to_string();

                message
                    .edit(
                        &http,
                        EditMessage::default()
                            .embed(
                                serenity::CreateEmbed::new()
                                    .title(name.clone())
                                    .description("…")
                                    .thumbnail(thumbnail.clone())
                                    .footer(serenity::CreateEmbedFooter::new(footer.clone())),
                            )
                            .components(disabled_buttons.clone()),
                    )
                    .await
                    .context(EditMessageSnafu)?;
                let now = std::time::Instant::now();
                let request = create_request(history.clone())?;
                let mut output = String::new();
                let mut stream = data
                    .ai
                    .chat()
                    .create_stream(request)
                    .await
                    .context(OpenAiStreamSnafu)?;
                let mut one_second_timer = Instant::now();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(response) => {
                            for chat_choice in &response.choices {
                                if let Some(ref content) = chat_choice.delta.content {
                                    output = format!("{output}{content}");
                                    if one_second_timer.elapsed() > Duration::from_secs(1) {
                                        let elapsed = now.elapsed().as_secs_f64().to_one_decimal();
                                        let footer = format!(
                                            "{}/{} | tog {}s | {}/4096",
                                            current_page + 1,
                                            history.clone().choices.len() + 1,
                                            elapsed,
                                            output.len()
                                        );
                                        message
                                            .edit(
                                                &http,
                                                EditMessage::default().embed(
                                                    serenity::CreateEmbed::new()
                                                        .title(history.character.to_string())
                                                        .description(output.clone())
                                                        .thumbnail(
                                                            history.character.avatar.to_string(),
                                                        )
                                                        .footer(serenity::CreateEmbedFooter::new(
                                                            footer.clone(),
                                                        )),
                                                ),
                                            )
                                            .await
                                            .context(EditMessageSnafu)?;
                                        one_second_timer = Instant::now();
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if let OpenAIError::StreamError(ref why) = err {
                                if why == "Stream ended" {
                                    break;
                                }
                                output = format!("Någonting gick fel, skyll inte på mig: {err}");
                                message
                                    .edit(
                                        &http,
                                        EditMessage::default().embed(
                                            serenity::CreateEmbed::new()
                                                .title(history.character.to_string())
                                                .description(output.clone())
                                                .thumbnail(history.character.avatar.to_string())
                                                .footer(serenity::CreateEmbedFooter::new("1/1")),
                                        ),
                                    )
                                    .await
                                    .context(EditMessageSnafu)?;
                            }
                        }
                    }
                }
                let output = SuperMessage::new_assistant(history.clone().character.name, output);
                let elapsed = now.elapsed().as_secs_f64().to_one_decimal();
                history.update(output.clone(), message.id, elapsed);
            }
            let footer = format!(
                "{}/{} | tog {}s | {}/4096",
                current_page + 1,
                history.clone().choices.len(),
                &history.clone().seconds_taken[current_page],
                response.len()
            );
            let name = history.character.to_string();
            let description = history.choices[current_page].message.to_string();
            let thumbnail = history.character.avatar.to_string();
            message
                .edit(
                    &http,
                    EditMessage::default()
                        .embed(
                            serenity::CreateEmbed::new()
                                .title(name)
                                .description(description)
                                .thumbnail(thumbnail)
                                .footer(serenity::CreateEmbedFooter::new(footer)),
                        )
                        .components(enabled_buttons.clone()),
                )
                .await
                .context(EditMessageSnafu)?;
            data.insert_history(history.clone());
        }
    }

    Ok(())
}

#[derive(Debug, Snafu, Diagnostic)]
enum EventHandlerError {
    SendMessage {
        source: poise::serenity_prelude::Error,
    },
    EditMessage {
        source: poise::serenity_prelude::Error,
    },
    PinMessage {
        source: poise::serenity_prelude::Error,
    },
    Defer {
        source: poise::serenity_prelude::Error,
    },
    Modal {
        source: poise::serenity_prelude::Error,
    },
    Acknowledge {
        source: poise::serenity_prelude::Error,
    },
    OpenAiRequest {
        source: async_openai::error::OpenAIError,
    },
    OpenAiStream {
        source: async_openai::error::OpenAIError,
    },
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        let Some(message) = event.message() else {
            return;
        };
        if message.author.bot() {
            return;
        }
        let Some(history) = message.history(&ctx.data()) else {
            return;
        };
        let mention = message.author.mention();
        if let Err(why) = event_handler(ctx, message, history).await {
            warn!("Error in event handler: {why}");
            let message = format!("{mention} Någonting gick fel där: {why}");
            if let Err(why) = CONFIG.log_channel().say(&ctx.http, message).await {
                warn!("Error logging event handler error in Discord: {why}");
            }
        }
    }
}

#[derive(Debug, Clone, Modal)]
#[name = "Redigera meddelandet"]
pub struct EditMessageModal {
    #[name = "Innehåll"]
    #[placeholder = "Meddelandets innehåll…"]
    pub message: String,
}

async fn finish_response_edit_message(
    http: &Http,
    elapsed: f64,
    enabled_buttons: Vec<CreateActionRow<'static>>,
    super_message: SuperMessage,
    history: &History,
    message: &mut Message,
) -> Result<()> {
    let length = super_message.message.len();
    let footer = format!("1/1 | tar {elapsed}s | {length}/4096");
    let name = history.character.to_string();
    let thumbnail = history.character.avatar.to_string();
    message
        .edit(
            &http,
            EditMessage::default()
                .embed(
                    serenity::CreateEmbed::new()
                        .title(name.clone())
                        .description(super_message.message)
                        .thumbnail(thumbnail.clone())
                        .footer(serenity::CreateEmbedFooter::new(footer.clone())),
                )
                .components(enabled_buttons),
        )
        .await
        .context(EditMessageSnafu)?;
    Ok(())
}

async fn stream_response_edit_message(
    http: &Http,
    client: &Client<OpenAIConfig>,
    request: CreateChatCompletionRequest,
    message: &mut Message,
    history: &History,
) -> Result<(String, f64)> {
    let now = Instant::now();
    let mut output = String::new();
    let mut one_second_timer = Instant::now();
    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .context(OpenAiStreamSnafu)?;
    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                for chat_choice in &response.choices {
                    if let Some(ref content) = chat_choice.delta.content {
                        output.push_str(content);
                        if one_second_timer.elapsed() > Duration::from_secs(1) {
                            let elapsed = now.elapsed().as_secs_f64().to_one_decimal();
                            let length = output.len();
                            let footer = format!("1/1 | tog {elapsed}s | {length}/4096");
                            message
                                .edit(
                                    &http,
                                    EditMessage::default().embed(
                                        CreateEmbed::new()
                                            .title(history.character.to_string())
                                            .description(output.clone())
                                            .thumbnail(history.character.avatar.to_string())
                                            .footer(CreateEmbedFooter::new(footer)),
                                    ),
                                )
                                .await
                                .context(EditMessageSnafu)?;
                            one_second_timer = Instant::now();
                        }
                    }
                }
            }
            Err(err) => {
                if let OpenAIError::StreamError(ref why) = err {
                    if why == "Stream ended" {
                        break;
                    }
                    output = format!("Någonting gick fel, skyll inte på mig: {err}");
                    message
                        .edit(
                            &http,
                            EditMessage::default().embed(
                                serenity::CreateEmbed::new()
                                    .title(history.character.to_string())
                                    .description(output.clone())
                                    .thumbnail(history.character.avatar.to_string())
                                    .footer(serenity::CreateEmbedFooter::new("1/1")),
                            ),
                        )
                        .await
                        .context(EditMessageSnafu)?;
                }
            }
        }
    }
    Ok((output, now.elapsed().as_secs_f64().to_one_decimal()))
}

async fn log_user_response(
    http: &Http,
    user_response: &Message,
    super_user_response: &SuperMessage,
    history: &History,
) -> Result<Message> {
    let truncated_message = super_user_response.message.truncate_middle();
    let author = &super_user_response.author;
    let character = history.character.to_string();

    info!("New message from {author} to {character}.\n👤 {truncated_message}");
    info!("History:\n{history}");
    info!("Frågar roboten efter ett svar...");

    let author = format!("Nytt meddelande från {author} till {character}");
    let embed_author = match user_response.author.avatar_url() {
        Some(icon_url) => CreateEmbedAuthor::new(author).icon_url(icon_url),
        None => CreateEmbedAuthor::new(author),
    };
    let footer = CreateEmbedFooter::new("Frågar roboten efter ett svar…");
    let embed = CreateEmbed::new()
        .author(embed_author)
        .title("Historia")
        .description(history.to_string())
        .field("Meddelande", format!("👤 {truncated_message}"), false)
        .footer(footer);
    let message = CreateMessage::new().embed(embed);
    Ok(CONFIG.log_channel().send_message(http, message).await?)
}

async fn log_bot_response(
    http: &Http,
    log_message: Message,
    character: &Character,
    bot_response: &SuperMessage,
    elapsed: f64,
) -> Result<Message> {
    let truncated_message = bot_response.message.truncate_middle();

    info!("New response from {character}.\n🤖 {truncated_message}");

    let author = format!("Nytt svar från {character}");
    let embed_author = CreateEmbedAuthor::new(author).icon_url(character.avatar.to_string());
    let footer = CreateEmbedFooter::new(format!("Tog {elapsed} sekunder."));
    let embed = CreateEmbed::new()
        .author(embed_author)
        .description(truncated_message)
        .footer(footer);
    let message = CreateMessage::new()
        .embed(embed)
        .reference_message(&log_message);

    Ok(CONFIG.log_channel().send_message(http, message).await?)
}

fn create_request(history: History) -> Result<CreateChatCompletionRequest, EventHandlerError> {
    CreateChatCompletionRequestArgs::default()
        .model(CONFIG.openai_model())
        .max_tokens(2048_u16)
        .temperature(1.3)
        .frequency_penalty(0.5)
        .presence_penalty(0.5)
        .messages(history)
        .build()
        .context(OpenAiRequestSnafu)
}

fn create_button(
    emoji: impl Into<String>,
    id: impl Into<String>,
    disabled: bool,
) -> serenity::CreateButton<'static> {
    serenity::CreateButton::new(id.into())
        .emoji(ReactionType::try_from(emoji.into()).expect("valid emoji"))
        .disabled(disabled)
}

fn create_buttons(msg: &Message) -> (Vec<CreateActionRow<'static>>, Vec<CreateActionRow<'static>>) {
    let msg_id = msg.id;
    (
        vec![CreateActionRow::Buttons(
            vec![
                create_button('◀', format!("{msg_id}prev"), false),
                create_button('▶', format!("{msg_id}next"), false),
                create_button('📌', format!("{msg_id}pin"), false),
                create_button("✏️", format!("{msg_id}edit"), false),
            ]
            .into(),
        )],
        vec![CreateActionRow::Buttons(
            vec![
                create_button('◀', format!("{msg_id}prev"), true),
                create_button('▶', format!("{msg_id}next"), true),
                create_button('📌', format!("{msg_id}pin"), true),
                create_button("✏️", format!("{msg_id}edit"), true),
            ]
            .into(),
        )],
    )
}

fn create_button_ids(msg: &Message) -> (String, String, String, String) {
    let msg_id = msg.id;
    (
        format!("{msg_id}prev"),
        format!("{msg_id}next"),
        format!("{msg_id}pin"),
        format!("{msg_id}edit"),
    )
}

async fn create_initial_message(
    http: &Http,
    history: &History,
    new_message: &Message,
) -> Result<Message, EventHandlerError> {
    let (_, disabled_buttons) = create_buttons(new_message);

    let character_name = history.character.name.to_string();
    let character_avatar = history.character.avatar.to_string();

    let initial_embed = serenity::CreateEmbed::new()
        .title(character_name)
        .description("…")
        .thumbnail(character_avatar)
        .footer(serenity::CreateEmbedFooter::new("1/1"));

    let initial_message = CreateMessage::default()
        .embed(initial_embed)
        .components(disabled_buttons.clone())
        .reference_message(new_message);

    new_message
        .channel_id
        .send_message(http, initial_message)
        .await
        .context(SendMessageSnafu)
}

trait ToOneDecimal {
    fn to_one_decimal(self) -> f64;
}

impl ToOneDecimal for f64 {
    fn to_one_decimal(self) -> f64 {
        (self * 10.0).floor() / 10.0
    }
}

trait MessageFromEvent {
    fn message(&self) -> Option<Message>;
}

trait ReplyFromMessage {
    fn get_reply(&self) -> Option<&Message>;
}

trait HistoryFromMessage {
    fn history(&self, data: &Arc<Data>) -> Option<History>;
}

impl MessageFromEvent for FullEvent {
    fn message(&self) -> Option<Message> {
        if let Self::Message { new_message, .. } = self {
            Some(new_message.to_owned())
        } else {
            None
        }
    }
}

impl ReplyFromMessage for Message {
    fn get_reply(&self) -> Option<&Message> {
        self.referenced_message.as_deref()
    }
}

impl HistoryFromMessage for Message {
    fn history(&self, data: &Arc<Data>) -> Option<History> {
        let reply = self.get_reply()?;
        data.history(reply)
    }
}
