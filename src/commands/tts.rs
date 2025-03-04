use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_STANDARD};
use poise::CreateReply;
use serde::Deserialize;
use serenity::CreateAttachment;

use crate::prelude::*;

#[poise::command(context_menu_command = "Läs upp")]
pub async fn läs_upp(ctx: Context<'_>, msg: serenity::Message) -> Result<()> {
    let content = if msg.content.is_empty() {
        let Some(embed) = msg.embeds.first() else {
            ctx.reply("Någonting gick fel här... #2").await?;
            return Ok(());
        };
        let Some(ref content) = embed.description else {
            ctx.reply("Någonting gick fel här... #2").await?;
            return Ok(());
        };
        content.to_string()
    } else {
        msg.content.to_string()
    };
    let bytes = send_tts_request(content).await?;
    let attachment = CreateAttachment::bytes(bytes, format!("{}.mp3", ctx.id()));
    ctx.send(CreateReply::new().attachment(attachment)).await?;
    Ok(())
}

pub async fn send_tts_request(content: impl AsRef<str>) -> Result<Vec<u8>> {
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
        .await?
        .json::<Response>()
        .await?;
    Ok(BASE64_STANDARD.decode(response.v_data)?)
}

#[derive(Deserialize)]
struct Response {
    #[allow(dead_code)]
    status: bool,
    v_data: String,
}
