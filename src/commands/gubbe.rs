use crate::{
    character::{Avatar, Emoji},
    prelude::*,
};

#[poise::command(
    slash_command,
    subcommand_required,
    subcommands("skapa", "ändra", "döda")
)]
#[allow(clippy::unused_async)]
pub async fn gubbe(_: Context<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(slash_command)]
async fn skapa(
    ctx: Context<'_>,
    #[description = "Gubbens namn"] namn: String,
    #[description = "Gubbens hälsning"] hälsning: Option<String>,
    #[description = "Gubbens beskrivning"] beskrivning: Option<String>,
    #[description = "Gubbens emoji"] emoji: Option<String>,
    #[description = "Gubbens profilbild (URL)"] profilbild: Option<String>,
) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let character = Character::new(namn.clone(), hälsning, beskrivning, emoji, profilbild);
    ctx.say(format!("Hurra! Gubben {character} skapades."))
        .await?;
    ctx.data().insert_character(character);
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

#[poise::command(slash_command)]
async fn döda(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
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
