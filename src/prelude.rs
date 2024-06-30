pub use crate::error::Error;
pub use crate::utils::character::Character;
pub use crate::utils::config::substitute_name;
pub use crate::utils::config::CONFIG;
pub use crate::utils::discord::autocomplete_character_name;
pub use crate::utils::super_message::SuperMessage;
pub use crate::utils::super_message::{History, AVATAR};
pub use poise::serenity_prelude as serenity;
pub type Result<T, E = crate::error::Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, crate::utils::discord::Data, crate::error::Error>;
pub type FrameworkContext<'a> =
    poise::FrameworkContext<'a, crate::utils::discord::Data, crate::error::Error>;
pub type FrameworkError<'a> =
    poise::FrameworkError<'a, crate::utils::discord::Data, crate::error::Error>;
