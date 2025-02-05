use async_std::path::PathBuf;
use redb::{Database, Error, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fragment {
    pub uuid_peer: String,
    pub session_key: String,
    pub session: String,
    pub filename: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Storage {
    pub uuid_peer: String,
    pub session_key: String,
    pub session: String,
    pub filename: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SecretKeyInfo {
    pub owner_id: String,
    pub access_key: String,
    pub size: usize,
}

// Определение коллекций
const STORAGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("storage");
const MYFILES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("myfiles");
const PEER_INFO_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("peer_info");
const SECRET_KEYS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("secret_keys");

pub struct P2PDatabase {
    db: Arc<Mutex<Database>>,
    pub path: String,
}

impl P2PDatabase {
    pub fn new(path: &str) -> Result<Self, Error> {
        let db_file = PathBuf::from(path).join("db");

        let db = Database::create(db_file)?;
        {
            let write_txn = db.begin_write()?;
            {
                write_txn.open_table(STORAGE_TABLE)?;
                write_txn.open_table(MYFILES_TABLE)?;
                write_txn.open_table(PEER_INFO_TABLE)?;
                write_txn.open_table(SECRET_KEYS_TABLE)?;
            }
            write_txn.commit()?;
        }

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            path: path.to_string(),
        })
    }
    
    // Peer Info Management
    pub fn get_or_create_peer_id(&self) -> Result<String, Error> {
        let db = self.db.lock().unwrap();
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PEER_INFO_TABLE)?;

        if let Some(data) = table.get("peer_id")? {
            Ok(String::from_utf8(data.value().to_vec()).unwrap())
        } else {
            drop(read_txn);

            let new_uuid = Uuid::new_v4().to_string();
            let write_txn = db.begin_write()?;
            {
                let mut table = write_txn.open_table(PEER_INFO_TABLE)?;
                table.insert("peer_id", new_uuid.as_bytes())?;
            }
            write_txn.commit()?;

            Ok(new_uuid)
        }
    }
    pub fn add_storage_fragment(&self, fragment: Storage) -> Result<(), Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;
        // let key = Uuid::new_v4().to_string();

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

    // MyFiles Management
    pub fn add_myfile_fragment(&self, fragment: Fragment) -> Result<(), Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;
        let key = Uuid::new_v4().to_string();

        {
            let mut table = tx.open_table(MYFILES_TABLE)?;
            let data = serde_json::to_string(&fragment).unwrap();
            table.insert(key.as_str(), data.as_bytes())?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_myfile_fragments(&self) -> Result<Vec<Fragment>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(MYFILES_TABLE)?;

        let mut result = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let fragment: Fragment = serde_json::from_str(&data).unwrap();
            result.push(fragment);
        }

        Ok(result)
    }

    // Secret Keys Management
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
