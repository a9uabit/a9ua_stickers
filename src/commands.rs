use poise::{CreateReply, serenity_prelude::CreateMessage};

use crate::{Context, Error};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let ping = ctx.ping().await;
    ctx.say(format!("Pong :ping_pong: ! {}ms", ping.as_millis()))
        .await?;

    Ok(())
}

