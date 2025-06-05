use redb::TableDefinition;

pub const DHT_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("dht");
pub const STORAGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("storage");
pub const PEER_INFO_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("peer_info");
pub const SECRET_KEYS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("secret_keys");
pub const TOKENS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("tokens");
pub const VALIDATOR_STORAGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("validator_storage");
pub const PEER_STATS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("peer_stats");
pub const CONTRACT_METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("contract_metadata");
pub const CONTRACT_CONSENSUS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("contract_consensus"); 