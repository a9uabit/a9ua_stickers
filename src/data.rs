use std::{collections::HashSet, sync::Arc};
use poise::serenity_prelude::StickerId;
use serde::{Deserialize, Serialize};
use slotmap::{SlotMap, new_key_type};
use tokio::sync::{Mutex, RwLock};

const FILE_PATH: &str = "./data.json";

pub struct DataFileManager {
    cached_file: RwLock<Option<Arc<RwLock<DataFile>>>>,
    load_locks: Arc<Mutex<()>>,
}

new_key_type! {
    pub struct StickerKey;
}

#[derive(Default, Serialize, Deserialize)]
pub struct DataFile {
    // it is not too verbose
    // or rather, it is optimized because it uses to detect duplications
    pub stickers_set: HashSet<StickerId>,
    pub stickers: SlotMap<StickerKey, Sticker>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Sticker {
    pub name: String,
    pub id: StickerId,
}

impl DataFileManager {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            cached_file: Default::default(),
            load_locks: Default::default(),
        })
    }

    async fn load_data(&self) -> std::io::Result<Arc<RwLock<DataFile>>> {
        let load_lock = Arc::clone(&self.load_locks);

        // avoid waiting for another load when a cached value is already available
        // if not, wait for the ongoing load and check the cache again
        let _lock = match load_lock.try_lock() {
            Ok(lock) => lock,
            Err(_) => {
                if let Ok(cached_file) = self.cached_file.try_read()
                    && let Some(cached_file) = &*cached_file
                {
                    let cached_file = Arc::clone(cached_file);

                    return Ok(cached_file);
                }

                let lock = load_lock.lock().await;

                if let Ok(cached_file) = self.cached_file.try_read()
                    && let Some(cached_file) = &*cached_file
                {
                    let cached_file = Arc::clone(cached_file);

                    return Ok(cached_file);
                }

                lock
            }
        };

        let sounds_file = match tokio::fs::read_to_string(FILE_PATH).await {
            Ok(contents) => {
                let data: DataFile = serde_json::from_str(&contents)?;

                data
            }

            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let data = DataFile::default();
                let json = serde_json::to_string_pretty(&data).unwrap();

                tokio::fs::write(FILE_PATH, json).await?;

                data
            }

            Err(e) => {
                return Err(e);
            }
        };

        let sounds_file = Arc::new(RwLock::new(sounds_file));

        *self.cached_file.write().await = Some(Arc::clone(&sounds_file));

        Ok(sounds_file)
    }

    pub async fn data<R>(&self, reader: impl FnOnce(&DataFile) -> R) -> std::io::Result<R> {
        let data = self.load_data().await?;
        let data = data.read().await;

        Ok(reader(&data))
    }

    pub async fn data_mut<R>(
        &self,
        writer: impl FnOnce(&mut DataFile) -> R,
    ) -> std::io::Result<R> {
        let data = self.load_data().await?;
        let mut data = data.write().await;

        let res = writer(&mut data);

        let json = serde_json::to_string_pretty(&*data).unwrap();

        tokio::fs::write(FILE_PATH, json).await?;

        Ok(res)
    }
}
