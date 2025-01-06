use std::time::Duration;

use poise::{execute_modal_on_component_interaction, CreateReply, Modal};
use serenity::{
    ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed, ReactionType,
};

use crate::{
    character::{Avatar, Emoji},
    prelude::*,
};

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

#[poise::command(
    slash_command,
    prefix_command,
    subcommand_required,
    subcommands("visa", "skapa", "ändra", "döda")
)]
#[allow(clippy::unused_async)]
pub async fn gubbe(_: Context<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn visa(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
    namn: String,
) -> Result<()> {
    let Some(most_similar_name) = most_similar_name_to(&namn, ctx) else {
        ctx.say("Gubben hittades inte!").await?;
        return Ok(());
    };
    let Some(character) = ctx.data().character(&most_similar_name) else {
        ctx.say("Gubben hittades inte!").await?;
        return Ok(());
    };

    let reply = {
        let character_name = character.to_string();
        let greeting = character.greeting.to_string();
        let description = character.description.to_string();
        let avatar = character.avatar.to_string();

        let embed = CreateEmbed::default()
            .title(character_name)
            .description(description)
            .field("Hälsning", greeting, false)
            .thumbnail(avatar);

        CreateReply::default().embed(embed)
    };

    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn skapa(
    ctx: Context<'_>,
    #[description = "Gubbens namn"] namn: Option<String>,
    #[description = "Gubbens hälsning"] hälsning: Option<String>,
    #[description = "Gubbens beskrivning"] beskrivning: Option<String>,
    #[description = "Gubbens emoji"] emoji: Option<String>,
    #[description = "Gubbens profilbild (URL)"] profilbild: Option<String>,
) -> Result<()> {
    match ctx {
        poise::Context::Application(_) => {
            ctx.defer_ephemeral().await?;
            let Some(namn) = namn else {
                ctx.say("Hörrudu, jag tror att du glömde ett namn där!")
                    .await?;
                return Ok(());
            };
            let character = Character::new(namn.clone(), hälsning, beskrivning, emoji, profilbild);
            ctx.say(format!("Hurra! Gubben {character} skapades."))
                .await?;
            ctx.data().insert_character(character);
        }
        poise::Context::Prefix(_) => {
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
                    .filter(move |interaction| {
                        interaction.data.custom_id.as_str() == ctx_id.to_string()
                    })
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
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
async fn ändra(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
    namn: String,
    #[description = "Gubbens hälsning"] hälsning: Option<String>,
    #[description = "Gubbens beskrivning"] beskrivning: Option<String>,
    #[description = "Gubbens emoji"] emoji: Option<String>,
    #[description = "Gubbens profilbild (URL)"] profilbild: Option<String>,
) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let data = ctx.data();
    let Some(mut character) = data.character(&namn) else {
        ctx.say("ingen gubbe hittades!").await?;
        return Ok(());
    };
    if let Some(greeting) = hälsning {
        character.greeting = SuperMessage::new_assistant(namn.clone(), greeting);
    };
    if let Some(description) = beskrivning {
        character.description = SuperMessage::new_assistant(namn, description);
    }
    if let Some(emoji) = emoji {
        character.emoji = Emoji::from(emoji);
    }
    if let Some(avatar) = profilbild {
        character.avatar = Avatar::from(avatar);
    }
    ctx.say(format!("Hurra! Gubben {character} ändrades."))
        .await?;
    data.insert_character(character);
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn döda(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
    #[rest]
    namn: String,
) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let Some(character) = ctx.data().character(&namn) else {
        ctx.say("ingen gubbe hittades!").await?;
        return Ok(());
    };
    ctx.data().remove_character(&namn);
    ctx.say(format!("Hurra! Gubben {character} dödades."))
        .await?;
    Ok(())
}
