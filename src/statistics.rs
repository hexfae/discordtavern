use crate::prelude::History;
use dashmap::DashMap;
use poise::serenity_prelude::MessageId;

#[derive(Debug, Default)]
pub struct Statistics {
    pub characters: DashMap<String, CharacterStatistic>,
}

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct CharacterStatistic {
    pub times_spawned: u32,
}

/// Number of messages automatically added upon spawning a new chat
const INITIAL_MESSAGES: usize = 9;

impl From<&DashMap<MessageId, History>> for Statistics {
    fn from(input: &DashMap<MessageId, History>) -> Self {
        let statistics = Self::default();
        for history in input {
            if history.history.len() != INITIAL_MESSAGES {
                continue;
            }
            if let Some(mut character) = statistics
                .characters
                .get_mut(&history.character.name.to_string())
            {
                character.times_spawned += 1;
            } else {
                statistics.characters.insert(
                    history.character.name.to_string(),
                    CharacterStatistic { times_spawned: 1 },
                );
            }
        }
        statistics
    }
}
