use crate::{
    config::Config,
    db::{models::Storage, tables::STORAGE_TABLE},
};
use async_std::fs::File;
use async_std::io::ReadExt;
use async_std::stream::StreamExt;
use redb::{Error, ReadableTable};

use super::P2PDatabase;

#[derive(Debug)]
pub enum StorageError {
    Redb(redb::Error),
    Io(std::io::Error),
}

impl From<redb::Error> for StorageError {
    fn from(err: redb::Error) -> Self {
        StorageError::Redb(err)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::Io(err)
    }
}

impl P2PDatabase {
    pub fn add_storage_fragment(&self, fragment: Storage) -> Result<(), Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;

        {
            let mut table = tx.open_table(STORAGE_TABLE)?;
            let data = serde_json::to_string(&fragment).unwrap();
            table.insert(fragment.session_key.as_str(), data.as_bytes())?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_storage_fragments(&self) -> Result<Vec<Storage>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(STORAGE_TABLE)?;

        let mut result = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let fragment: Storage = serde_json::from_str(&data).unwrap();
            result.push(fragment);
        }

        Ok(result)
    }

    pub fn get_storage_fragments_by_key(&self, key: &str) -> Result<Vec<Storage>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(STORAGE_TABLE)?;

        let mut result = Vec::new();
        for item in table.iter()? {
            let (k, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let fragment: Storage = serde_json::from_str(&data).unwrap();
            if k.value().to_string() == key {
                result.push(fragment);
            }
        }

        Ok(result)
    }

    pub async fn get_storage_size(&self) -> Result<u64, StorageError> {
        let path = format!("{}/files", self.path);

        if !std::path::Path::new(&path).exists() {
            std::fs::create_dir_all(&path).unwrap();
            return Ok(0);
        }

        let mut total_size = 0u64;

        let mut entries = async_std::fs::read_dir(&path).await?;
        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }

    pub async fn get_storage_free_space(&self) -> Result<u64, StorageError> {
        let config = Config::from_file("config.toml");
        let free_space = config.storage_size - self.get_storage_size().await?;
        Ok(free_space)
    }
}
