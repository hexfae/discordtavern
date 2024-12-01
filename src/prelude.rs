pub use crate::character::Character;
pub use crate::config::substitute_name;
pub use crate::config::CONFIG;
pub use crate::discord::autocomplete_character_name;
pub use crate::super_message::SuperMessage;
pub use crate::super_message::{History, AVATAR};
pub use poise::serenity_prelude as serenity;
pub type Result<T, E = crate::error::Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, crate::discord::Data, crate::error::Error>;
pub type FrameworkContext<'a> =
    poise::FrameworkContext<'a, crate::discord::Data, crate::error::Error>;
pub type FrameworkError<'a> = poise::FrameworkError<'a, crate::discord::Data, crate::error::Error>;

use itertools::Itertools;
use strsim::levenshtein;

pub fn most_similar_name_to(input: impl AsRef<str>, ctx: Context<'_>) -> Option<String> {
    ctx.data()
        .characters()
        .into_iter()
        .map(|character| character.name.to_string())
        .map(|character_name| (levenshtein(input.as_ref(), &character_name), character_name))
        .sorted()
        .map(|(_, character_name)| character_name)
        .collect_vec()
        .first()
        .cloned()
}
