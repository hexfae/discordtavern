pub use crate::character::Character;
pub use crate::config::CONFIG;
pub use crate::discord::autocomplete_character_name;
pub use crate::super_message::AVATAR;
pub use crate::super_message::History;
pub use crate::super_message::SuperMessage;
pub use crate::super_message::TruncateMiddle;
pub use poise::serenity_prelude as serenity;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, crate::discord::Data, Error>;
pub type FrameworkError<'a> = poise::FrameworkError<'a, crate::discord::Data, Error>;

use strsim::normalized_damerau_levenshtein;

#[derive(Debug, snafu::Snafu)]
pub enum DiscordError {
    #[snafu(whatever, display("{message}"))]
    Whatever {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error+ Send + Sync>, Some)))]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

pub fn most_similar_name_to(input: impl AsRef<str>, ctx: Context<'_>) -> Option<String> {
    ctx.data()
        .characters()
        .into_iter()
        .map(|character| character.name.to_string())
        .map(|character_name| {
            (
                normalized_damerau_levenshtein(input.as_ref(), &character_name),
                character_name,
            )
        })
        .max_by(|(a, _), (b, _)| f64::total_cmp(a, b))
        .map(|(_, character_name)| character_name)
}

pub fn substitute_name(input: impl AsRef<str>) -> String {
    CONFIG
        .name_substitutes()
        .iter()
        .find(|(from, _)| input.as_ref() == from)
        .map_or_else(|| "User".into(), |(_, to)| to.into())
}
