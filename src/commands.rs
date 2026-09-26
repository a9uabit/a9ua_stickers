use poise::{
    CreateReply,
    serenity_prelude::{CreateMessage, StickerId},
};
use slotmap::KeyData;

use crate::{
    Context, Error,
    data::{Sticker, StickerKey},
};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let ping = ctx.ping().await;
    ctx.say(format!("Pong :ping_pong: ! {}ms", ping.as_millis()))
        .await?;

    Ok(())
}

/// スタンプを登録します
#[poise::command(slash_command)]
pub async fn register_sticker(
    ctx: Context<'_>,
    #[description = "追加するスタンプ"]
    #[rename = "sticker"]
    sticker_id: String,
) -> Result<(), Error> {
    let Ok(sticker_id) = sticker_id.parse::<u64>() else {
        ctx.say("数字で入力してください").await?;

        return Ok(());
    };

    let sticker_id = StickerId::from(sticker_id);

    let data = ctx.data();
    let user_id = data.user_id;

    if ctx.author().id != user_id {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("指定されたユーザー以外は使用できません"),
        )
        .await?;

        return Ok(());
    }

    let Ok(sticker) = sticker_id.to_sticker(ctx).await else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("スタンプが見つかりません"),
        )
        .await?;

        return Ok(());
    };

    let sticker_name = sticker.name.clone();

    ctx.defer().await?;

    let res = data
        .data_file_manager
        .data_mut(|data| {
            if data.stickers_set.insert(sticker_id) {
                data.stickers.insert(Sticker {
                    name: sticker_name,
                    id: sticker_id,
                });

                true
            } else {
                false
            }
        })
        .await?;

    if res {
        ctx.send(
            CreateReply::new().content(format!("スタンプを登録しました\n名前: `{}`", sticker.name)),
        )
        .await?;
        ctx.channel_id()
            .send_message(ctx, CreateMessage::new().sticker_id(sticker_id))
            .await?;
    } else {
        ctx.say("スタンプは既に追加されています").await?;
    }

    Ok(())
}

/// スタンプを送信します
#[poise::command(slash_command)]
pub async fn send_sticker(
    ctx: Context<'_>,
    #[description = "スタンプを選択"]
    #[rename = "sticker"]
    sticker_key_id: u64,
) -> Result<(), Error> {
    let data_file_manager = &ctx.data().data_file_manager;
    let key = StickerKey::from(KeyData::from_ffi(sticker_key_id));

    let sticker_id = data_file_manager
        .data(|data_file| data_file.stickers.get(key).map(|sticker| sticker.id))
        .await?;

    let Some(sticker_id) = sticker_id else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("スタンプが見つかりません"),
        )
        .await?;

        return Ok(());
    };

    ctx.channel_id()
        .send_message(ctx, CreateMessage::new().sticker_id(sticker_id))
        .await?;

    Ok(())
}
