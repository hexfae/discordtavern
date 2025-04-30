use crate::prelude::*;
use miette::Diagnostic;
use poise::{
    CreateReply,
    serenity_prelude::{self as serenity},
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu, Diagnostic)]
enum TalkError {
    #[snafu(display(
        "Kunde inte skicka ett meddelande: \"{}\"\nFör att: {}",
        message,
        source
    ))]
    #[diagnostic(
        code(discordtavern::commands::chat::prata),
        help("Kanske Discord är nere?")
    )]
    SendMessage {
        source: poise::serenity_prelude::Error,
        message: String,
    },
    #[snafu(display("Kunde inte hämta ett meddelande.\nFör att: {}", source))]
    #[diagnostic(
        code(discordtavern::commands::chat::prata),
        help("Kanske Discord är nere?")
    )]
    GetMessage {
        source: poise::serenity_prelude::Error,
    },
}

#[poise::command(slash_command, prefix_command)]
pub async fn hälsa(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
    #[rest]
    namn: String,
) -> Result<()> {
    let Some(most_similar_name) = most_similar_name_to(&namn, ctx) else {
        let message = "Gubben hittades inte!".to_owned();
        ctx.say(&message)
            .await
            .context(SendMessageSnafu { message })?;
        return Ok(());
    };
    let Some(character) = ctx.data().character(&most_similar_name) else {
        let message = "Gubben hittades inte!".to_owned();
        ctx.say(&message)
            .await
            .context(SendMessageSnafu { message })?;
        return Ok(());
    };
    let character_name = character.to_string();
    let avatar = character.avatar.to_string();

    let sent_message = {
        let embed = serenity::CreateEmbed::new()
            .title(&character_name)
            .description(character.greeting.message.clone())
            .thumbnail(&avatar);
        ctx.send(CreateReply::default().embed(embed))
            .await
            .context(SendMessageSnafu {
                message: character.greeting.message.clone(),
            })?
    };

    let history = character
        .into_history_with_greeting(sent_message.message().await.context(GetMessageSnafu)?.id);
    ctx.data().insert_history(history);
    Ok(())
}
