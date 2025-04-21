use std::time::{Duration, Instant};

use async_openai::{
    error::OpenAIError,
    types::{CreateChatCompletionRequest, CreateChatCompletionRequestArgs},
};
use futures::StreamExt;
use miette::Diagnostic;
use poise::{
    CreateReply, Modal, execute_modal_on_component_interaction,
    serenity_prelude::{
        ComponentInteractionCollector, ComponentInteractionDataKind, CreateActionRow, CreateEmbed,
        CreateInteractionResponse, CreateMessage, CreateSelectMenu, CreateSelectMenuOption,
        EditMessage, Http, Message, ReactionType,
    },
};
use snafu::{ResultExt, Snafu};

use crate::{event_handler::EditMessageModal, prelude::*};

#[derive(Debug, Clone, Modal)]
#[name = "Vilken gubbe?"]
struct CharacterNameModal {
    #[name = "Namn"]
    #[placeholder = "Gubbens namn…"]
    name: String,
}

#[derive(Debug, Snafu, Diagnostic)]
pub enum AnswerAsError {
    SendMessage {
        source: poise::serenity_prelude::Error,
    },
    EditMessage {
        source: poise::serenity_prelude::Error,
    },
    DeleteMessage {
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

#[poise::command(context_menu_command = "Svara som…")]
#[allow(clippy::too_many_lines)]
pub async fn svara_som(ctx: Context<'_>, msg: serenity::Message) -> Result<()> {
    ctx.defer_ephemeral().await.context(DeferSnafu)?;
    let data = ctx.data();
    let http = ctx.http();
    let Some(mut history) = data.history(&msg) else {
        ctx.reply("Någonting gick fel här! Meddelandet hittades inte i databasen.")
            .await
            .context(SendMessageSnafu)?;
        return Ok(());
    };

    let mut characters = data.characters();
    characters.sort();
    characters.reverse();
    characters.truncate(25);
    let mut chosen_character_name = String::new();
    let ctx_id = ctx.id();

    {
        let create_select_menu_options = characters
            .iter()
            .map(|character| {
                CreateSelectMenuOption::new(character.to_string(), character.name.to_string())
            })
            .collect();
        let select_menu = vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
            ctx_id.to_string(),
            serenity::CreateSelectMenuKind::String {
                options: create_select_menu_options,
            },
        ))];
        let msg = CreateReply::new()
            .content("Var snäll och klicka på nedanstående knapp!")
            .components(select_menu);
        let sent = ctx.send(msg).await.context(SendMessageSnafu)?;

        'outer: while let Some(interaction) =
            ComponentInteractionCollector::new(ctx.serenity_context())
                .filter(move |interaction| {
                    interaction.data.custom_id.starts_with(&ctx_id.to_string())
                })
                .timeout(Duration::from_secs(60 * 60 * 24))
                .await
        {
            if interaction.data.custom_id == ctx_id.to_string() {
                if let ComponentInteractionDataKind::StringSelect { values } =
                    &interaction.data.kind
                {
                    let selection = values[0].clone();
                    chosen_character_name = selection;
                    interaction
                        .create_response(http, CreateInteractionResponse::Acknowledge)
                        .await
                        .context(AcknowledgeSnafu)?;
                    break 'outer;
                }
            }
        }
        sent.delete(ctx).await.context(DeleteMessageSnafu)?;
    };

    let Some(new_character) = data.character(&chosen_character_name) else {
        ctx.say("Gubben kunde inte hittas!")
            .await
            .context(SendMessageSnafu)?;
        return Ok(());
    };

    history.character = new_character;
    history.push_message(history.choices[history.current_page].clone());
    history.replace_message(6, history.character.description.clone());

    let mut message = create_initial_message(http, &history, &msg).await?;
    let (prev_button_id, next_button_id, pin_button_id, edit_button_id) =
        create_button_ids(&message);
    let (enabled_buttons, disabled_buttons) = create_buttons(&message);
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
        .await
        .context(EditMessageSnafu)?;
    let super_message = SuperMessage::new_assistant(history.clone().character.name, output.clone());
    history.reset_choices();
    history.update(super_message.clone(), message.id, elapsed);
    data.insert_history(history.clone());

    let mut current_page: usize = 0;
    while let Some(interaction) = ComponentInteractionCollector::new(ctx.serenity_context())
        .filter(move |interaction| {
            interaction
                .data
                .custom_id
                .starts_with(&message.id.to_string())
        })
        .timeout(Duration::from_secs(60 * 60 * 24))
        .await
    {
        if interaction.data.custom_id == pin_button_id {
            let channel_id = msg.channel_id;
            let character_name = history.character.to_string();
            let message_content = history.choices[current_page].message.to_string();
            let avatar = history.character.avatar.to_string();

            let embed = CreateEmbed::new()
                .title(&character_name)
                .description(&message_content)
                .thumbnail(&avatar);
            let message = CreateMessage::new().embed(embed).reference_message(&msg);

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
                ctx.serenity_context(),
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

            if let Some(content) = modal.message {
                history.update_choice(&content, current_page);
                data.insert_history(history.clone());

                let footer = format!(
                    "{}/{} | tog {}s | {}/4096 (redigerad)",
                    current_page + 1,
                    history.choices.len(),
                    &history.seconds_taken[current_page],
                    content.len(),
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
            } else {
                let description = history.choices[current_page].message.to_string();
                let footer = format!(
                    "{}/{} | tog {}s | {}/4096 (redigerad)",
                    current_page + 1,
                    history.choices.len(),
                    &history.seconds_taken[current_page],
                    description.len(),
                );

                let history = history.clone();
                let name = history.character.to_string();
                let thumbnail = history.character.avatar.to_string();
                message.edit(
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
                ).await.context(EditMessageSnafu)?;
            }
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
                .await
                .context(EditMessageSnafu)?;
            data.insert_history(history.clone());
        }
    }

    Ok(())
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
) -> Result<Message, AnswerAsError> {
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

fn create_request(history: History) -> Result<CreateChatCompletionRequest, AnswerAsError> {
    CreateChatCompletionRequestArgs::default()
        .model(CONFIG.openai_model())
        .max_tokens(2048_u16)
        .temperature(1.3)
        // .frequency_penalty(0.5)
        // .presence_penalty(0.5)
        .messages(history)
        .build()
        .context(OpenAiRequestSnafu)
}
