use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::Config;
use crate::signal::SignalServer;
use crate::peer::Peer;
use crate::db::P2PDatabase;
use std::path::PathBuf;
use std::collections::HashMap;

pub struct NetworkSimulation {
    signal_servers: HashMap<String, Arc<SignalServer>>,
    peers: HashMap<String, Arc<Mutex<Peer>>>,
    db_paths: HashMap<String, PathBuf>,
}

impl NetworkSimulation {
    pub fn new() -> Self {
        Self {
            signal_servers: HashMap::new(),
            peers: HashMap::new(),
            db_paths: HashMap::new(),
        }
    }

    pub async fn add_signal_server(&mut self, config: &Config, id: String, db_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if !db_path.exists() {
            std::fs::create_dir_all(&db_path)?;
        }
        let db = P2PDatabase::new(db_path.to_str().unwrap())?;
        let signal_server = SignalServer::new(config, &db).await;
        self.signal_servers.insert(id.clone(), signal_server);
        self.db_paths.insert(id, db_path);
        Ok(())
    }

    pub async fn add_peer(&mut self, config: &Config, id: String, db_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if !db_path.exists() {
            std::fs::create_dir_all(&db_path)?;
        }
        let db = P2PDatabase::new(db_path.to_str().unwrap())?;
        let peer = Peer::new(config, &db).await;
        self.peers.insert(id.clone(), Arc::new(Mutex::new(peer)));
        self.db_paths.insert(id, db_path);
        Ok(())
    }

    pub async fn run_simulation(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut handles = vec![];

        // Запускаем сигнальные серверы
        for (id, server) in &self.signal_servers {
            let server = server.clone();
            let id = id.clone();
            handles.push(tokio::spawn(async move {
                println!("Starting signal server: {}", id);
                server.run().await;
            }));
        }

        // Запускаем пиры
        for (id, peer) in &self.peers {
            let peer = peer.clone();
            let id = id.clone();
            handles.push(tokio::spawn(async move {
                println!("Starting peer: {}", id);
                peer.lock().await.run().await;
            }));
        }

        // Ждем завершения всех задач
        for handle in handles {
            handle.await?;
        }

        Ok(())
    }

    pub fn get_peer(&self, id: &str) -> Option<&Arc<Mutex<Peer>>> {
        self.peers.get(id)
    }
} 