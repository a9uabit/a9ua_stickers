use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};

use dashmap::{DashMap, try_result::TryResult};
use poise::serenity_prelude::{ChannelId, GuildId, WebhookId};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

const FOLDER_PATH: &str = "./guild_data";

pub struct GuildDataFileManager {
    cached_guild_data: DashMap<GuildId, Arc<RwLock<GuildDataFile>>>,
    load_locks: DashMap<GuildId, Arc<Mutex<()>>>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct GuildDataFile {
    pub webhooks: HashMap<ChannelId, WebhookId>,
}

impl GuildDataFileManager {
    pub async fn new() -> std::io::Result<Self> {
        tokio::fs::create_dir_all(FOLDER_PATH).await?;

        Ok(Self {
            cached_guild_data: Default::default(),
            load_locks: Default::default(),
        })
    }

    async fn load_data(&self, guild_id: GuildId) -> std::io::Result<Arc<RwLock<GuildDataFile>>> {
        let load_lock = {
            let load_lock = self.load_locks.entry(guild_id).or_default();

            Arc::clone(load_lock.value())
        };

        // avoid waiting for another load when a cached value is already available
        // if not, wait for the ongoing load and check the cache again
        let _lock = match load_lock.try_lock() {
            Ok(lock) => lock,
            Err(_) => {
                if let TryResult::Present(cached) = self.cached_guild_data.try_get(&guild_id) {
                    let cached_file = Arc::clone(cached.value());

                    return Ok(cached_file);
                }

                let lock = load_lock.lock().await;

                if let TryResult::Present(cached) = self.cached_guild_data.try_get(&guild_id) {
                    let cached_file = Arc::clone(cached.value());

                    return Ok(cached_file);
                }

                lock
            }
        };

        let path = PathBuf::from_str(FOLDER_PATH).unwrap();
        let path = path.join(format!("{}.json", guild_id.get()));
        let data = match tokio::fs::read_to_string(path.clone()).await {
            Ok(content) => {
                let data: GuildDataFile = serde_json::from_str(&content)?;

                data
            }

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let data = GuildDataFile::default();

                let json = serde_json::to_string_pretty(&data).unwrap();
                tokio::fs::write(path, json).await?;

                data
            }

            Err(error) => {
                return Err(error);
            }
        };

        let data = Arc::new(RwLock::new(data));

        self.cached_guild_data.insert(guild_id, Arc::clone(&data));

        Ok(data)
    }

    pub async fn data<R>(&self, guild_id: GuildId, reader: impl FnOnce(&GuildDataFile) -> R) -> std::io::Result<R> {
        let data = self.load_data(guild_id).await?;
        let data = data.read().await;

        Ok(reader(&data))
    }

    pub async fn data_mut<R>(&self, guild_id: GuildId, writer: impl FnOnce(&mut GuildDataFile) -> R) -> std::io::Result<R> {
        let data = self.load_data(guild_id).await?;
        let mut data = data.write().await;

        let result = writer(&mut data);

        let path = PathBuf::from_str(FOLDER_PATH).unwrap();
        let path = path.join(format!("{}.json", guild_id.get()));

        let json = serde_json::to_string_pretty(&*data).unwrap();
        tokio::fs::write(path, json).await?;

        Ok(result)
    }
}
