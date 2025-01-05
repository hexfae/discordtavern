use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::discord::Data;
use crate::prelude::*;
use async_openai::error::OpenAIError;
use async_openai::types::{CreateChatCompletionRequest, CreateChatCompletionRequestArgs};
use futures::StreamExt;
use poise::serenity_prelude::{
    ComponentInteractionCollector, CreateActionRow, CreateEmbed, CreateInteractionResponse,
    CreateMessage, EditMessage, FullEvent, Message, ReactionType, Http,
};
use poise::{execute_modal_on_component_interaction, Modal};

#[derive(Debug, Clone, Modal)]
#[name = "Redigera meddelandet"]
pub struct EditMessageModal {
    #[name = "Innehåll"]
    #[placeholder = "Meddelandets innehåll…"]
    pub message: String,
}

fn create_button_ids(msg: &Message) -> (String, String, String, String) {
    let msg_id = msg.id;
    (format!("{msg_id}prev"), format!("{msg_id}next"), format!("{msg_id}pin"), format!("{msg_id}edit"))
}

async fn create_initial_message(http: &Http, history: &History, new_message: &Message) -> Result<Message> {
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

    Ok(new_message.channel_id.send_message(http, initial_message).await?)
}

#[allow(clippy::too_many_lines)]
pub async fn event_handler(ctx: FrameworkContext<'_>, event: &FullEvent) -> Result<()> {
    let data = ctx.user_data();
    let Some((new_message, mut history)) = get_chat_message_and_history(event, &data) else {
        return Ok(());
    };
    let http = &ctx.serenity_context.http;
    history.push_message(history.choices[history.current_page].clone());
    history.push_message(new_message.clone());

    let (prev_button_id, next_button_id, pin_button_id, edit_button_id) = create_button_ids(&new_message);
    let (enabled_buttons, disabled_buttons) = create_buttons(&new_message);
    let mut message = create_initial_message(http, &history, &new_message).await?;
    let now = std::time::Instant::now();
    let request = create_request(history.clone())?;
    let mut output = String::new();
    let mut stream = data.ai.chat().create_stream(request).await?;
    let mut one_second_timer = Instant::now();
    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                for chat_choice in &response.choices {
                    if let Some(ref content) = chat_choice.delta.content {
                        output.push_str(content);
                        if one_second_timer.elapsed() > Duration::from_secs(1) {
                            let elapsed = format!("{:.1}", now.elapsed().as_secs_f64())
                                .parse::<f64>()
                                .expect("valid time taken");
                            let length = output.len();
                            let footer = format!("1/1 | tog {elapsed}s | {length}/4096");
                            message
                                .edit(
                                    &http,
                                    EditMessage::default().embed(
                                        serenity::CreateEmbed::new()
                                            .title(history.character.to_string())
                                            .description(output.clone())
                                            .thumbnail(history.character.avatar.to_string())
                                            .footer(serenity::CreateEmbedFooter::new(footer)),
                                    ),
                                )
                                .await?;
                            one_second_timer = Instant::now();
                        }
                    }
                }
            }
            Err(err) => {
                dbg!(&err);
                if let OpenAIError::StreamError(ref why) = err {
                    if why == "Stream ended" {
                        break;
                    }
                    output = format!("Någonting gick fel, skyll inte på mig!: {err}");
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
                        .await?;
                }
            }
        }
    }
    let elapsed = format!("{:.1}", now.elapsed().as_secs_f64())
        .parse::<f64>()
        .expect("valid time taken");
    let length = output.len();
    let footer = format!("1/1 | tog {elapsed}s | {length}/4096");
    let name = history.character.to_string();
    let thumbnail = history.character.avatar.to_string();
    message
        .edit(
            &http,
            EditMessage::default()
                .embed(
                    serenity::CreateEmbed::new()
                        .title(name.clone())
                        .description(output.clone())
                        .thumbnail(thumbnail.clone())
                        .footer(serenity::CreateEmbedFooter::new(footer.clone())),
                )
                .components(enabled_buttons.clone()),
        )
        .await?;
    let super_message = SuperMessage::new_assistant(history.clone().character.name, output.clone());
    history.reset_choices();
    history.update(super_message.clone(), message.id, elapsed);
    data.insert_history(history.clone());

    let mut current_page: usize = 0;
    while let Some(interaction) =
        ComponentInteractionCollector::new(ctx.serenity_context.shard.clone())
            .filter(move |interaction| interaction.data.custom_id.starts_with(&new_message.id.to_string()))
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

            let pinned_message = channel_id.send_message(http, message).await?;

            pinned_message.pin(http, None).await?;

            interaction
                .create_response(http, CreateInteractionResponse::Acknowledge)
                .await?;
        } else if interaction.data.custom_id == edit_button_id {
            let footer = format!(
                "{}/{} | tog {}s | {}/4096",
                current_page + 1,
                history.choices.len(),
                &history.seconds_taken[current_page],
                history.choices[current_page].message.len(),
            );
            let user_name = substitute_name(interaction.clone().user.name);
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
                .await?;
            let Some(modal) = execute_modal_on_component_interaction::<EditMessageModal>(
                ctx.serenity_context,
                interaction.clone(),
                None,
                None,
            )
            .await?
            else {
                message
                    .edit(
                        &http,
                        EditMessage::new().components(enabled_buttons.clone()),
                    )
                    .await?;
                interaction
                    .create_response(http, CreateInteractionResponse::Acknowledge)
                    .await?;
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
                .await?;
        } else if interaction.data.custom_id == prev_button_id {
            interaction.defer(http).await?;
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
                .await?;
            data.insert_history(history.clone());
        } else if interaction.data.custom_id == next_button_id {
            interaction.defer(http).await?;
            current_page += 1;
            history.current_page = current_page;

            if current_page >= history.choices.len() {
                let footer = format!("{}/{}", current_page + 1, history.choices.len() + 1);
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
                    .await?;
                let now = std::time::Instant::now();
                let request = create_request(history.clone())?;
                let mut output = String::new();
                let mut stream = data.ai.chat().create_stream(request).await?;
                let mut one_second_timer = Instant::now();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(response) => {
                            for chat_choice in &response.choices {
                                if let Some(ref content) = chat_choice.delta.content {
                                    output = format!("{output}{content}");
                                    if one_second_timer.elapsed() > Duration::from_secs(1) {
                                        let elapsed = format!("{:.1}", now.elapsed().as_secs_f64());
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
                                            .await?;
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
                                output = format!("Någonting gick fel, skyll på OpenAI!: {err}");
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
                                    .await?;
                            }
                        }
                    }
                }
                let output = SuperMessage::new_assistant(history.clone().character.name, output);
                let elapsed = format!("{:.1}", now.elapsed().as_secs_f64())
                    .parse::<f64>()
                    .expect("valid time taken");
                history.update(output.clone(), message.id, elapsed);
            }
            let footer = format!(
                "{}/{} | tog {}s | {}/4096",
                current_page + 1,
                history.clone().choices.len(),
                &history.clone().seconds_taken[current_page],
                output.len()
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
                .await?;
            data.insert_history(history.clone());
        }
    }

    Ok(())
}

fn create_request(history: History) -> Result<CreateChatCompletionRequest> {
    Ok(CreateChatCompletionRequestArgs::default()
        .model(CONFIG.read().openai_model())
        .max_tokens(2048_u16)
        .temperature(1.3)
        .frequency_penalty(0.5)
        .presence_penalty(0.5)
        .messages(history)
        .build()?)
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
        vec![CreateActionRow::Buttons(vec![
            create_button('◀', format!("{msg_id}prev"), false),
            create_button('▶', format!("{msg_id}next"), false),
            create_button('📌', format!("{msg_id}pin"), false),
            create_button("✏️", format!("{msg_id}edit"), false),
        ])],
        vec![CreateActionRow::Buttons(vec![
            create_button('◀', format!("{msg_id}prev"), true),
            create_button('▶', format!("{msg_id}next"), true),
            create_button('📌', format!("{msg_id}pin"), true),
            create_button("✏️", format!("{msg_id}edit"), true),
        ])],
    )
}

fn get_chat_message_and_history(event: &FullEvent, data: &Arc<Data>) -> Option<(Message, History)> {
    let message = event.message()?;
    let reply = message.get_reply()?;
    let history = data.history(reply)?;
    Some((message.to_owned(), history))
}

trait MessageFromEvent {
    fn message(&self) -> Option<&Message>;
}

impl MessageFromEvent for FullEvent {
    fn message(&self) -> Option<&Message> {
        if let Self::Message { new_message } = self {
            Some(new_message)
        } else {
            None
        }
    }
}

trait ReplyFromMessage {
    fn get_reply(&self) -> Option<&Message>;
}

impl ReplyFromMessage for Message {
    fn get_reply(&self) -> Option<&Message> {
        self.referenced_message.as_deref()
    }
}
