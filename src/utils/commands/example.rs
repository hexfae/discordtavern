use crate::prelude::*;
use poise::serenity_prelude::Message;

#[poise::command(context_menu_command = "Lägg till som exempel")]
pub async fn add_example(ctx: Context<'_>, meddelande: Message) -> Result<()> {
    let data = ctx.data();
    let history = data.get_history(&meddelande)?;
    let character_name = history.character.name.to_string();
    let mut character = data.character(&character_name)?;
    character.push_example_message(meddelande);
    data.insert_character(character);
    ctx.say("Exempel tillagt!").await?;
    Ok(())
}
