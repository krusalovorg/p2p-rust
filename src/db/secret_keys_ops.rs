use crate::db::{models::SecretKeyInfo, tables::SECRET_KEYS_TABLE};
use redb::Error;
use uuid::Uuid;

use super::P2PDatabase;

impl P2PDatabase {
    pub fn generate_and_store_secret_key(&self, peer_id: &str) -> Result<String, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;
        let secret_key = Uuid::new_v4().to_string();

        let secret_key_info = SecretKeyInfo {
            owner_id: peer_id.to_string(),
            access_key: secret_key.clone(),
            size: 1024,
        };

        {
            let mut table = tx.open_table(SECRET_KEYS_TABLE)?;
            let data = serde_json::to_string(&secret_key_info).unwrap();
            table.insert(secret_key.as_str(), data.as_bytes())?;
        }

        tx.commit()?;
        Ok(secret_key)
    }

    pub fn get_secret_key_info(&self, access_key: &str) -> Result<Option<SecretKeyInfo>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(SECRET_KEYS_TABLE)?;

        match table.get(access_key)? {
            Some(data) => {
                let json_str = String::from_utf8(data.value().to_vec()).unwrap();
                let info: SecretKeyInfo = serde_json::from_str(&json_str).unwrap();
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }
} 