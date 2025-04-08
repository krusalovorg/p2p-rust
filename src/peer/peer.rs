use colored::*;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::connection::Connection;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::tunnel::Tunnel;
use crate::db::P2PDatabase;

pub struct Peer {
    connection_manager: Arc<ConnectionManager>,
    connection: Arc<Connection>,
    tunnel: Arc<Mutex<Tunnel>>,
    my_public_addr: String,
    db: Arc<P2PDatabase>,
}

impl Peer {
    pub async fn new(db: &P2PDatabase) -> Self {
        let config: Config = Config::from_file("config.toml");
        let tunnel = Arc::new(Mutex::new(Tunnel::new().await));
        let connection_manager = Arc::new(ConnectionManager::new(db).await);

        {
            let tunnel_guard = tunnel.lock().await;
            println!(
                "{}",
                format!(
                    "[Peer] You public ip:port: {}:{}",
                    tunnel_guard.get_public_ip(),
                    tunnel_guard.get_public_port()
                )
                .yellow()
            );
        }

        let (tunnel_public_ip, tunnel_public_port) = {
            let tunnel_guard = tunnel.lock().await;
            (tunnel_guard.get_public_ip(), tunnel_guard.get_public_port())
        };

        let my_public_addr = format!("{}:{}", tunnel_public_ip, tunnel_public_port);

        let connection = Arc::new(
            Connection::new(
                config.signal_server_ip.clone(),
                config.signal_server_port,
                tunnel_public_ip,
                tunnel_public_port,
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
            tunnel,
            my_public_addr,
            db: Arc::new(db.clone()),
        }
    }

    pub async fn run(&self) {
        let peer_id = self.db.get_or_create_peer_id().unwrap();
        println!("{}", format!("[Peer] Your UUID: {}", peer_id).yellow());

        println!("[Peer] Starting peer...");

        self.connection_manager.handle_incoming_packets().await;
    }
}
