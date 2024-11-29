use std::time::Duration;

use poise::{execute_modal_on_component_interaction, CreateReply, Modal};
use serenity::{ComponentInteractionCollector, CreateActionRow, CreateButton, ReactionType};

use crate::prelude::*;

#[derive(Debug, Clone, Modal)]
#[name = "Skapa en gubbe"]
struct CreateCharacterModal {
    #[name = "Namn"]
    #[placeholder = "Gubbens namn…"]
    name: String,
    #[name = "Hälsning"]
    #[placeholder = "Gubbens hälsning…"]
    greeting: Option<String>,
    #[name = "Beskrivning"]
    #[placeholder = "Gubbens beskrivning…"]
    description: Option<String>,
    #[name = "Emoji"]
    #[placeholder = "Gubbens emoji…"]
    emoji: Option<String>,
    #[name = "Profilbild"]
    #[placeholder = "Gubbens profilbild…"]
    avatar: Option<String>,
}

#[poise::command(prefix_command, subcommand_required, subcommands("skapa"))]
#[allow(clippy::unused_async)]
pub async fn gubbe(_: Context<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(prefix_command)]
async fn skapa(ctx: Context<'_>) -> Result<()> {
    let ctx_id = ctx.id();
    {
        let button = CreateButton::new(ctx_id.to_string())
            .emoji(ReactionType::try_from("✏️").expect("valid emoji"));
        let component = CreateActionRow::Buttons(vec![button]);
        let reply = CreateReply::default()
            .content("Var snäll och klicka på nedanstående knapp!")
            .components(vec![component])
            .reply(true);
        ctx.send(reply).await?;
    }
    while let Some(interaction) =
        ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
            .filter(move |interaction| interaction.data.custom_id.as_str() == ctx_id.to_string())
            .author_id(ctx.author().id)
            .timeout(Duration::from_secs(60 * 60 * 24))
            .await
    {
        let Some(modal) = execute_modal_on_component_interaction::<CreateCharacterModal>(
            ctx.serenity_context(),
            interaction,
            None,
            None,
        )
        .await?
        else {
            return Ok(());
        };
        ctx.defer_ephemeral().await?;
        let character = Character::new(
            modal.name,
            modal.greeting,
            modal.description,
            modal.emoji,
            modal.avatar,
        );
        ctx.say(format!("Hurra! Gubben {character} skapades."))
            .await?;
        ctx.data().insert_character(character);
    }
    Ok(())
}
