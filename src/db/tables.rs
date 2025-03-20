use redb::TableDefinition;

pub const DHT_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("dht");
pub const STORAGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("storage");
pub const MYFILES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("myfiles");
pub const PEER_INFO_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("peer_info");
pub const SECRET_KEYS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("secret_keys"); 