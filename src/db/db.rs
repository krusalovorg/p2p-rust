use leveldb::snapshots::Snapshots;
use uuid::Uuid;
use leveldb::database::Database;
use leveldb::options::{Options, ReadOptions, WriteOptions};
use leveldb::kv::KV;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fragment {
    pub uuid_peer: String,
    pub session_key: String,
    pub session: String,
    pub filename: String,
    // pub fragment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MyFiles {
    filename: HashMap<String, Vec<Fragment>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Storage {
    pub uuid_peer: String,
    pub session_key: String,
    pub session: String,
    // pub fragment: String,
    pub filename: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DatabaseSchema {
    myfiles: MyFiles,
    storage: Vec<Storage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecretKeyInfo {
    pub owner_id: String,
    pub access_key: String,
    pub size: usize,
}

pub struct P2PDatabase {
    db: Arc<Mutex<Database<i32>>>,
    pub path: PathBuf,
}

impl Clone for P2PDatabase {
    fn clone(&self) -> Self {
        let mut options = Options::new();
        options.create_if_missing = true;
        P2PDatabase {
            db: Arc::new(Mutex::new(Database::open(self.path.as_path(), options).unwrap())),
            path: self.path.clone(),
        }
    }
}

impl P2PDatabase {
    /// Создает новый экземпляр базы данных P2PDatabase
    pub fn new(path: &Path) -> Self {
        let mut options = Options::new();
        options.create_if_missing = true;
        let db = Database::open(path, options).unwrap();
        P2PDatabase { db: Arc::new(Mutex::new(db)), path: path.to_path_buf() }
    }

    /// Получает идентификатор пира (UUID) из базы данных
    pub fn get_peer_id(&self) -> String {
        let read_opts = ReadOptions::new();
        let write_opts = WriteOptions::new();
        let key = self.hash_key("uuid");

        let db = self.db.lock().unwrap();
        match db.get(read_opts, key) {
            Ok(Some(uuid)) => String::from_utf8(uuid).expect("Invalid UUID format"),
            Ok(None) => {
                let new_uuid = Uuid::new_v4().to_string();
                db.put(write_opts, key, new_uuid.as_bytes()).expect("Failed to save UUID");
                new_uuid
            }
            Err(e) => panic!("Failed to access peer_info: {}", e),
        }
    }

    /// Очищает информацию о пире из базы данных
    pub fn clear_peer_info(&self) {
        let write_opts = WriteOptions::new();
        let key = self.hash_key("uuid");
        let db = self.db.lock().unwrap();
        db.delete(write_opts, key).expect("Failed to clear peer_info");
    }

    /// Получает информацию о пире (UUID) из базы данных
    pub fn get_peer_info(&self) -> Option<String> {
        let read_opts = ReadOptions::new();
        let key = self.hash_key("uuid");
        let db = self.db.lock().unwrap();
        match db.get(read_opts, key) {
            Ok(Some(uuid)) => Some(String::from_utf8(uuid).expect("Invalid UUID format")),
            Ok(None) => None,
            Err(e) => panic!("Failed to access peer_info: {}", e),
        }
    }

    /// Устанавливает новую информацию о пире (UUID) в базе данных
    pub fn set_peer_info(&self, new_uuid: &str) {
        let write_opts = WriteOptions::new();
        let key = self.hash_key("uuid");
        let db = self.db.lock().unwrap();
        db.put(write_opts, key, new_uuid.as_bytes()).expect("Failed to save UUID");
    }

    /// Добавляет фрагмент файла в базу данных
    pub fn add_myfile_fragment(&self, filename: &str, fragment: Fragment) {
        let read_opts = ReadOptions::new();
        let write_opts = WriteOptions::new();

        let key = format!("myfiles:{}", filename);
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        let mut myfiles: MyFiles = match db.get(read_opts, key_hash) {
            Ok(Some(data)) => serde_json::from_slice(&data).unwrap(),
            Ok(None) => MyFiles { filename: HashMap::new() },
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        };

        myfiles.filename.entry(filename.to_string()).or_insert(Vec::new()).push(fragment);
        let serialized = serde_json::to_vec(&myfiles).unwrap();
        db.put(write_opts, key_hash, &serialized).unwrap();
        db.snapshot();
    }

    /// Добавляет фрагмент хранилища в базу данных
    pub fn add_storage_fragment(&self, storage: Storage) {
        let read_opts = ReadOptions::new();
        let write_opts = WriteOptions::new();

        let key = "storage";
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        let mut storage_data: Vec<Storage> = match db.get(read_opts, key_hash) {
            Ok(Some(data)) => serde_json::from_slice(&data).unwrap(),
            Ok(None) => Vec::new(),
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        };

        storage_data.push(storage);
        let serialized = serde_json::to_vec(&storage_data).unwrap();
        db.put(write_opts, key_hash, &serialized).expect("Failed to save storage");
        db.snapshot();
    }

    /// Получает фрагменты файла из базы данных
    pub fn get_myfile_fragments(&self, filename: &str) -> Option<Vec<Fragment>> {
        let read_opts = ReadOptions::new();
        let key = format!("myfiles:{}", filename);
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        match db.get(read_opts, key_hash) {
            Ok(Some(data)) => {
                let myfiles: MyFiles = serde_json::from_slice(&data).unwrap();
                myfiles.filename.get(filename).cloned()
            },
            Ok(None) => None,
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        }
    }

    /// Получает фрагменты хранилища из базы данных
    pub fn get_storage_fragments(&self) -> Vec<Storage> {
        let read_opts = ReadOptions::new();
        let key = "storage";
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        match db.get(read_opts, key_hash) {
            Ok(Some(data)) => serde_json::from_slice(&data).unwrap(),
            Ok(None) => Vec::new(),
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        }
    }

    /// Получает все файлы из таблицы myfiles
    pub fn get_all_myfiles(&self) -> HashMap<String, Vec<Fragment>> {
        let read_opts = ReadOptions::new();
        let key = "myfiles";
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        match db.get(read_opts, key_hash) {
            Ok(Some(data)) => {
                let myfiles: MyFiles = serde_json::from_slice(&data).unwrap();
                myfiles.filename
            },
            Ok(None) => HashMap::new(),
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        }
    }

    /// Генерирует и сохраняет секретный ключ в базе данных
    pub fn generate_and_store_secret_key(&self, peer_id: &str) -> String {
        let write_opts = WriteOptions::new();
        let secret_key = Uuid::new_v4().to_string();
        let secret_key_info = SecretKeyInfo {
            owner_id: peer_id.to_string(),
            access_key: secret_key.clone(),
            size: 1024, // 1KB
        };

        let key = format!("secret_key:{}", secret_key);
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        let serialized = serde_json::to_vec(&secret_key_info).unwrap();
        db.put(write_opts, key_hash, &serialized).unwrap();

        secret_key
    }

    /// Получает информацию о секретном ключе из базы данных
    pub fn get_secret_key_info(&self, peer_id: &str, access_key: &str) -> Option<SecretKeyInfo> {
        let read_opts = ReadOptions::new();
        let key = format!("secret_key:{}", access_key);
        let key_hash = self.hash_key(&key);
        let db = self.db.lock().unwrap();
        match db.get(read_opts, key_hash) {
            Ok(Some(data)) => {
                let secret_key_info: SecretKeyInfo = serde_json::from_slice(&data).unwrap();
                if secret_key_info.owner_id == peer_id {
                    Some(secret_key_info)
                } else {
                    None
                }
            },
            Ok(None) => None,
            Err(e) => panic!("failed to retrieve value: {:?}", e),
        }
    }

    /// Хэш-функция для преобразования строки в i32
    fn hash_key(&self, key: &str) -> i32 {
        // Простая хэш-функция для преобразования строки в i32
        key.bytes().fold(0, |acc, b| acc.wrapping_add(b as i32))
    }
}