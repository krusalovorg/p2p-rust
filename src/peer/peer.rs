use std::sync::Arc;
use crate::logger;

use crate::config::Config;
use crate::connection::Connection;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::db::P2PDatabase;

pub struct Peer {
    connection_manager: Arc<ConnectionManager>,
    connection: Arc<Connection>,
    db: Arc<P2PDatabase>,
}

impl Peer {
    pub async fn new(config: &Config, db: &P2PDatabase) -> Self {
        let connection_manager = Arc::new(ConnectionManager::new(db).await);

        let connection = Arc::new(
            Connection::new(
                config.signal_server_ip.clone(),
                config.signal_server_port,
                db,
            )
            .await,
        );

        connection_manager
            .add_connection(
                format!("{}:{}", config.signal_server_ip, config.signal_server_port),
                connection.clone(),
            )
            .await;

        Peer {
            connection_manager,
            connection,
            db: Arc::new(db.clone()),
        }
    }

    pub async fn run(&self) {
        let peer_id = self.db.get_or_create_peer_id().unwrap();
        logger::info(&format!("[Peer] Your UUID: {}", peer_id));

        logger::info("[Peer] Starting peer...");

        self.connection_manager.handle_incoming_packets().await;
    }
}
