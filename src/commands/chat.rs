use crate::{event_handler::EditMessageModal, prelude::*};
use miette::Diagnostic;
use poise::{
    CreateReply, execute_modal_on_component_interaction,
    serenity_prelude::{
        self as serenity, ComponentInteractionCollector, CreateActionRow, CreateButton,
        CreateEmbed, ReactionType,
    },
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
    #[snafu(display(
        "Kunde inte redigera ett meddelande till: \"{}\"\nFör att: {}",
        message,
        source
    ))]
    #[diagnostic(
        code(discordtavern::commands::chat::prata),
        help("Kanske Discord är nere?")
    )]
    EditMessage {
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
    #[snafu(display("Kunde inte visa en modal.\nFör att: {}", source))]
    #[diagnostic(
        code(discordtavern::commands::chat::prata),
        help("Kanske Discord är nere?")
    )]
    Modal {
        source: poise::serenity_prelude::Error,
    },
}

#[poise::command(slash_command, prefix_command)]
pub async fn prata(
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
    let ctx_id = ctx.id();
    let character_name = character.to_string();
    let avatar = character.avatar.to_string();
    let components = vec![CreateActionRow::Buttons(
        vec![
            CreateButton::new(format!("{ctx_id}edit"))
                .emoji(ReactionType::try_from("✏️".to_string()).expect("valid emoji")),
        ]
        .into(),
    )];

    let sent_message = {
        let embed = serenity::CreateEmbed::new()
            .title(&character_name)
            .description(character.greeting.message.clone())
            .thumbnail(&avatar);
        ctx.send(CreateReply::default().embed(embed).components(&components))
            .await
            .context(SendMessageSnafu {
                message: character.greeting.message.clone(),
            })?
    };

    let history = character.into_history(sent_message.message().await.context(GetMessageSnafu)?.id);
    ctx.data().insert_history(history);

    while let Some(interaction) = ComponentInteractionCollector::new(ctx.serenity_context())
        .filter(move |interaction| interaction.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(std::time::Duration::from_secs(60 * 60 * 24))
        .await
    {
        if let Some(modal) = execute_modal_on_component_interaction::<EditMessageModal>(
            ctx.serenity_context(),
            interaction,
            None,
            None,
        )
        .await
        .context(ModalSnafu)?
        {
            let message = modal.message;
            let embed = CreateEmbed::new()
                .title(&character_name)
                .description(message.clone())
                .thumbnail(&avatar);
            let edit_message = CreateReply::new().embed(embed).components(&components);
            sent_message
                .edit(ctx, edit_message)
                .await
                .context(EditMessageSnafu { message })?;
        }
    }
    Ok(())
}
