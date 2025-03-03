use base64::{prelude::BASE64_STANDARD, Engine};
use poise::CreateReply;
use serenity::CreateAttachment;

use crate::prelude::*;

#[poise::command(context_menu_command = "Läs upp")]
pub async fn läs_upp(ctx: Context<'_>, msg: serenity::Message) -> Result<()> {
    let bytes = send_tts_request(msg.content).await?;
    let attachment = CreateAttachment::bytes(bytes, format!("{}.mp3", ctx.id()));
    ctx.send(CreateReply::new().attachment(attachment)).await?;
    Ok(())
}

async fn send_tts_request(content: impl AsRef<str>) -> Result<Vec<u8>> {
    let response = reqwest::Client::new()
        .post("https://countik.com/api/text/speech")
        .body(format!(
            r#"{{"text": "{}", "voice": "en_us_001"}}"#,
            content.as_ref()
        ))
        .send()
        .await?
        .bytes()
        .await?;
    Ok(BASE64_STANDARD.decode(response)?)
}
