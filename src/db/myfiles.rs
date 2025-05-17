use crate::db::{models::Fragment, tables::MYFILES_TABLE};
use redb::{Error, ReadableTable};
use uuid::Uuid;
use std::collections::HashMap;

use super::P2PDatabase;

impl P2PDatabase {
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

    pub fn get_myfile_fragments(&self) -> Result<HashMap<String, Fragment>, Error> {
        let db = self.db.lock().unwrap();
        let tx = db.begin_read()?;
        let table = tx.open_table(MYFILES_TABLE)?;

        let mut result = HashMap::new();
        for item in table.iter()? {
            let (key, value) = item?;
            let data = String::from_utf8(value.value().to_vec()).unwrap();
            let fragment: Fragment = serde_json::from_str(&data).unwrap();
            result.insert(key.value().to_string(), fragment);
        }

        Ok(result)
    }
} 