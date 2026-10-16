use anyhow::anyhow;
use poise::{
    CreateReply,
    serenity_prelude::{
        ChannelId, CreateAutocompleteResponse, CreateMessage, ExecuteWebhook, GuildId, StickerId,
        Webhook,
    },
};
use serde_json::json;
use slotmap::{Key, KeyData};

use crate::{
    Context, Error,
    data::{Sticker, StickerKey},
    guild_data::GuildDataFileManager,
};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let ping = ctx.ping().await;
    ctx.say(format!("Pong :ping_pong: ! {}ms", ping.as_millis()))
        .await?;

    Ok(())
}

async fn stickers(ctx: Context<'_>, partial: &str) -> CreateAutocompleteResponse {
    let mut response = CreateAutocompleteResponse::new();

    let Some(guild_id) = ctx.guild_id() else {
        response = response.add_string_choice("エラーが発生しました(`can't get guild id`)", "");

        return response;
    };

    let Ok(stickers) = guild_id.stickers(ctx).await else {
        response = response.add_string_choice("エラーが発生しました(`can't get stickers`)", "");

        return response;
    };

    for sticker in stickers
        .iter()
        .filter(|sticker| sticker.name.contains(partial))
    {
        response = response.add_string_choice(sticker.name.clone(), sticker.id.to_string());
    }

    response
}

/// スタンプを登録します
#[poise::command(slash_command)]
pub async fn register_sticker(
    ctx: Context<'_>,
    #[description = "追加するスタンプ"]
    #[autocomplete = "stickers"]
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
            .send_message(
                ctx,
                CreateMessage::new()
                    .content("↓プレビュー")
                    .sticker_id(sticker_id),
            )
            .await?;
    } else {
        ctx.say("スタンプは既に追加されています").await?;
    }

    Ok(())
}

async fn sticker_keys(ctx: Context<'_>, partial: &str) -> CreateAutocompleteResponse {
    let mut response = CreateAutocompleteResponse::new();

    let data = ctx.data();

    let stickers = match data
        .data_file_manager
        .data(|data| data.stickers.clone())
        .await
    {
        Ok(stickers) => stickers,
        Err(error) => {
            tracing::error!("{:?}", error);

            response =
                response.add_string_choice("エラーが発生しました(`コンソールをご覧ください`)", "");

            return response;
        }
    };

    for (key, sticker) in stickers
        .iter()
        .filter(|(_, sticker)| sticker.name.contains(partial))
    {
        response =
            response.add_string_choice(sticker.name.clone(), key.data().as_ffi().to_string());
    }

    response
}

/// スタンプを送信します
#[poise::command(slash_command)]
pub async fn send_sticker(
    ctx: Context<'_>,
    #[description = "スタンプを選択"]
    #[autocomplete = "sticker_keys"]
    #[rename = "sticker"]
    sticker_key_id: String,
) -> Result<(), Error> {
    let Ok(sticker_key_id) = sticker_key_id.parse::<u64>() else {
        ctx.say("数字で入力してください").await?;

        return Ok(());
    };

    let guild_data_file_manager = &ctx.data().guild_data_file_manager;
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

    let guild_id = ctx.guild_id().ok_or(anyhow!("can't get guild id"))?;
    let channel_id = ctx.channel_id();

    let webhook = {
        async fn create_webhook(
            ctx: &Context<'_>,
            guild_data_file_manager: &GuildDataFileManager,
            guild_id: GuildId,
            channel_id: ChannelId,
        ) -> Result<Webhook, Error> {
            ctx.defer().await?;

            let bot_name = {
                let user = ctx.cache().current_user();
                user.display_name().to_owned()
            };

            let map = json!({
                "name": bot_name,
            });

            let webhook = ctx.http().create_webhook(channel_id, &map, None).await?;

            guild_data_file_manager
                .data_mut(guild_id, |data| {
                    data.webhooks.insert(channel_id, webhook.id)
                })
                .await?;

            Ok(webhook)
        }

        let webhook_id = guild_data_file_manager
            .data(guild_id, |data| data.webhooks.get(&channel_id).copied())
            .await?;
        if let Some(webhook_id) = webhook_id {
            match webhook_id.to_webhook(ctx).await {
                Ok(webhook) => webhook,
                Err(_) => {
                    create_webhook(&ctx, guild_data_file_manager, guild_id, channel_id).await?
                }
            }
        } else {
            create_webhook(&ctx, guild_data_file_manager, guild_id, channel_id).await?
        }
    };

    let Ok(sticker) = sticker_id.to_sticker(ctx).await else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("スタンプが見つかりません"),
        )
        .await?;

        return Ok(());
    };

    let Some(sticker_url) = sticker.image_url() else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("不明なスタンプのタイプです"),
        )
        .await?;

        return Ok(());
    };

    let author = ctx.author();
    let mut execute_webhook = ExecuteWebhook::new()
        .username(author.display_name())
        .content(format!("[{}]({})", sticker.name, sticker_url));

    if let Some(avatar_url) = author.avatar_url() {
        execute_webhook = execute_webhook.avatar_url(avatar_url);
    }

    webhook.execute(ctx, false, execute_webhook).await?;

    let handle = ctx
        .send(CreateReply::new().ephemeral(true).content("ok"))
        .await?;

    handle.delete(ctx).await?;

    Ok(())
}
