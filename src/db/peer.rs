use crate::db::tables::PEER_INFO_TABLE;
use redb::Error;
use uuid::Uuid;

use super::P2PDatabase;

impl P2PDatabase {
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
} 