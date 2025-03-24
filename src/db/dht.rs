use crate::db::{models::DHTEntry, tables::DHT_TABLE};
use redb::{Error, ReadableTable};
use std::time::{SystemTime, UNIX_EPOCH};

use super::P2PDatabase;

impl P2PDatabase {
    pub fn add_dht_entry(&self, peer_id: &str, session_key: &str) -> Result<(), Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;

        let entry = DHTEntry {
            peer_id: peer_id.to_string(),
            session_key: session_key.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        {
            let mut table = tx.open_table(DHT_TABLE)?;
            let key = format!("{}:{}", peer_id, session_key);
            let data = serde_json::to_string(&entry).unwrap();
            table.insert(key.as_str(), data.as_bytes())?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_peers_by_session_key(&self, session_key: &str) -> Result<Vec<String>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(DHT_TABLE)?;

        let mut peers = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let entry: DHTEntry = serde_json::from_str(&data).unwrap();
            if entry.session_key == session_key {
                peers.push(entry.peer_id);
            }
        }

        Ok(peers)
    }

    pub fn get_files_by_peer(&self, peer_id: &str) -> Result<Vec<String>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(DHT_TABLE)?;

        let mut files = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let entry: DHTEntry = serde_json::from_str(&data).unwrap();
            if entry.peer_id == peer_id {
                files.push(entry.session_key);
            }
        }

        Ok(files)
    }

    pub fn remove_dht_entry(&self, peer_id: &str, session_key: &str) -> Result<(), Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_write()?;
        
        {
            let mut table = tx.open_table(DHT_TABLE)?;
            let key = format!("{}:{}", peer_id, session_key);
            table.remove(key.as_str())?;
        }

        tx.commit()?;
        Ok(())
    }
} 