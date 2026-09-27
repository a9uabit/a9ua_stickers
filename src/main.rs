mod commands;
mod data;
mod guild_data;

use poise::{
    FrameworkError, FrameworkOptions, PrefixFrameworkOptions,
    serenity_prelude::{ClientBuilder, GatewayIntents, UserId},
};
use serde::Deserialize;

use crate::data::DataFileManager;

#[derive(Deserialize)]
struct Config {
    token: String,
    user_id: UserId,
}

struct Data {
    data_file_manager: DataFileManager,
    user_id: UserId,
}

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let config = tokio::fs::read_to_string("./config.json").await?;
    let config: Config = serde_json::from_str(&config)?;

    let framework_options = FrameworkOptions {
        commands: vec![commands::ping(), commands::register_sticker(), commands::send_sticker()],
        on_error: |error| Box::pin(on_error(error)),
        prefix_options: PrefixFrameworkOptions {
            mention_as_prefix: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let data_file_manager = DataFileManager::default();
    let user_id = config.user_id;

    let framework = poise::Framework::builder()
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                tracing::info!("logged in as {}", ready.user.tag());
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(Data {
                    data_file_manager,
                    user_id,
                })
            })
        })
        .options(framework_options)
        .build();

    let mut client = ClientBuilder::new(config.token, GatewayIntents::empty())
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

async fn on_error(error: FrameworkError<'_, Data, Error>) {
    match error {
        FrameworkError::Setup { error, .. } => {
            tracing::error!("Failed to start bot: {:?}", error);
            panic!("{:?}", error);
        }
        FrameworkError::Command { error, ctx, .. } => {
            tracing::error!("Error in command {}: {:?}", ctx.command().name, error)
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                tracing::error!("Error while handling error: {}", e);
            }
        }
    }
}
