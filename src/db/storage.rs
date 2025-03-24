use crate::db::{models::Storage, tables::STORAGE_TABLE};
use redb::{Error, ReadableTable};

use super::P2PDatabase;

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
} 