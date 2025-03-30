use std::time::Duration;

use crate::prelude::*;
use derive_more::Display;
use miette::Diagnostic;
use poise::{
    CreateReply,
    serenity_prelude::{
        ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu, Diagnostic)]
enum CharactersError {
    #[snafu(display("Kunde inte skicka ett meddelande: \"{}\"", message))]
    #[diagnostic(
        code(discordtavern::commands::chat::prata),
        help("Kanske Discord är nere?")
    )]
    SendMessage {
        source: poise::serenity_prelude::Error,
        message: MessageType,
    },
    Respond {
        source: poise::serenity_prelude::Error,
    },
}

#[derive(Debug, Display)]
enum MessageType {
    #[display("{_0}")]
    Text(String),
    #[display("(Modal)")]
    Modal,
}

#[poise::command(slash_command, prefix_command)]
pub async fn gubbar(ctx: Context<'_>) -> Result<()> {
    let characters = ctx.data().characters();

    if characters.is_empty() {
        let message = "Du har inga gubbar, verkar det som! Eller så hittades inga :(".to_owned();
        ctx.say(&message).await.context(SendMessageSnafu {
            message: MessageType::Text(message),
        })?;
        return Ok(());
    }

    let ctx_id = ctx.id();
    let prev_button_id = format!("{ctx_id}prev");
    let next_button_id = format!("{ctx_id}next");
    let mut current_page = 0;

    let reply = {
        let character = &characters[current_page];
        let name = character.to_string();
        let greeting = &character.greeting.message;
        let description = &character.description.message;
        let avatar = character.avatar.to_string();

        let embed = CreateEmbed::default()
            .title(name)
            .description(description)
            .field("Hälsning", greeting, false)
            .thumbnail(avatar);

        let components = CreateActionRow::Buttons(
            vec![
                CreateButton::new(&prev_button_id).emoji('◀'),
                CreateButton::new(&next_button_id).emoji('▶'),
            ]
            .into(),
        );

        CreateReply::default()
            .embed(embed)
            .components(vec![components])
    };

    ctx.send(reply).await.context(SendMessageSnafu {
        message: MessageType::Modal,
    })?;

    while let Some(press) = ComponentInteractionCollector::new(ctx.serenity_context())
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(Duration::from_secs(60 * 60 * 24))
        .await
    {
        if press.data.custom_id == next_button_id {
            current_page += 1;
            if current_page >= characters.len() {
                current_page = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            current_page = current_page.checked_sub(1).unwrap_or(characters.len() - 1);
        } else {
            continue;
        }

        let character = &characters[current_page];
        let name = character.to_string();
        let greeting = &character.greeting.message;
        let description = &character.description.message;
        let avatar = character.avatar.to_string();

        let embed = CreateEmbed::default()
            .title(name)
            .description(description)
            .field("Hälsning", greeting, false)
            .thumbnail(avatar);

        press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new().embed(embed),
                ),
            )
            .await
            .context(RespondSnafu)?;
    }
    Ok(())
}
