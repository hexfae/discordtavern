use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_STANDARD};
use miette::Diagnostic;
use poise::CreateReply;
use serde::Deserialize;
use serenity::CreateAttachment;
use snafu::{ResultExt, Snafu};

use crate::prelude::*;

#[derive(Debug, Snafu, Diagnostic)]
enum TtsError {
    SendMessage {
        source: poise::serenity_prelude::Error,
    },
    SendRequest {
        source: reqwest::Error,
    },
    DecodeResponse {
        source: reqwest::Error,
    },
    DecodeBase64 {
        source: base64::DecodeError,
    },
}

#[poise::command(context_menu_command = "Läs upp")]
pub async fn läs_upp(ctx: Context<'_>, msg: serenity::Message) -> Result<(), TtsError> {
    let content = if msg.content.is_empty() {
        let Some(embed) = msg.embeds.first() else {
            ctx.reply("Någonting gick fel här... #2")
                .await
                .context(SendMessageSnafu)?;
            return Ok(());
        };
        let Some(ref content) = embed.description else {
            ctx.reply("Någonting gick fel här... #2")
                .await
                .context(SendMessageSnafu)?;
            return Ok(());
        };
        content.to_string()
    } else {
        msg.content.to_string()
    };
    let bytes = send_tts_request(content).await?;
    let attachment = CreateAttachment::bytes(bytes, format!("{}.mp3", ctx.id()));
    ctx.send(CreateReply::new().attachment(attachment))
        .await
        .context(SendMessageSnafu)?;
    Ok(())
}

pub async fn send_tts_request(content: impl AsRef<str>) -> Result<Vec<u8>, TtsError> {
    let request = HashMap::from([("text", content.as_ref()), ("voice", "en_us_001")]);
    let response = reqwest::Client::new()
        .post("https://countik.com/api/text/speech")
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        )
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context(SendRequestSnafu)?
        .json::<Response>()
        .await
        .context(DecodeResponseSnafu)?;
    Ok(BASE64_STANDARD
        .decode(response.v_data)
        .context(DecodeBase64Snafu)?)
}

#[derive(Deserialize)]
struct Response {
    #[allow(dead_code)]
    status: bool,
    v_data: String,
}
