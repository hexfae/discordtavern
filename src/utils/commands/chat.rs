use crate::{prelude::*, utils::event_handler::EditMessageModal};
use itertools::Itertools;
use poise::{
    execute_modal_on_component_interaction,
    serenity_prelude::{
        self as serenity, ComponentInteractionCollector, CreateActionRow, CreateButton,
        CreateEmbed, ReactionType,
    },
    CreateReply,
};
use strsim::levenshtein;

#[poise::command(slash_command, prefix_command)]
pub async fn prata(
    ctx: Context<'_>,
    #[description = "Gubbens namn"]
    #[autocomplete = "autocomplete_character_name"]
    #[rest]
    namn: String,
) -> Result<()> {
    let names = ctx
        .data()
        .characters()
        .into_iter()
        .map(|c| c.name.to_string())
        .map(|name| (levenshtein(&namn, &name), name))
        .sorted()
        .map(|(_, name)| name)
        .collect_vec();
    let most_similar_name = names.first().ok_or(Error::CharacterNotFound)?;
    let character = ctx.data().character(most_similar_name)?;
    let ctx_id = ctx.id();
    let character_name = character.to_string();
    let avatar = character.avatar.to_string();
    let components = vec![CreateActionRow::Buttons(vec![CreateButton::new(format!(
        "{ctx_id}edit"
    ))
    .emoji(ReactionType::try_from("✏️".to_string()).expect("valid emoji"))])];

    let message = {
        let embed = serenity::CreateEmbed::new()
            .title(&character_name)
            .description(character.greeting.to_string())
            .thumbnail(&avatar);
        ctx.send(CreateReply::default().embed(embed).components(&components))
            .await?
    };

    let history = character.into_history(message.message().await?.id);
    ctx.data().insert_history(history);

    while let Some(interaction) =
        ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
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
        .await?
        {
            let embed = CreateEmbed::new()
                .title(&character_name)
                .description(modal.message)
                .thumbnail(&avatar);
            let edit_message = CreateReply::new().embed(embed).components(&components);
            message.edit(ctx, edit_message).await?;
        }
    }

    Ok(())
}
