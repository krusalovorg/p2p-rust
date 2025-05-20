use clap::{Arg, Command};
use commands::get_db_path;
use peer::Peer;
use std::path::PathBuf;
use crate::config::Config;

mod config;
mod connection;
mod signal;
mod tunnel;
mod db;
mod peer;
mod ui;
mod packets;
mod manager;
mod crypto;
mod contract;
mod http;
mod logger;
mod commands;

use crate::signal::SignalServer;
use crate::db::P2PDatabase;
use crate::ui::{print_all_files, print_welcome, print_all_fragments};
use crate::contract::runtime::hardcoded_test_contract;
use crate::commands::create_base_commands;

#[tokio::main]
async fn main() {
    hardcoded_test_contract();
    let matches = create_base_commands().get_matches();
    let db_path = get_db_path(&matches);

    let path = PathBuf::from(db_path);
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }

    let db = P2PDatabase::new(path.to_str().unwrap()).unwrap();
    let config: Config = Config::from_file("config.toml");

    print_welcome();
    print_all_files(&db);

    if matches.get_flag("signal") {
        print_all_fragments(&db);
        let signal_server = SignalServer::new(&config, &db).await;
        signal_server.run().await;
    } else {
        let peer = Peer::new(&config, &db).await;
        peer.run().await;
    }
}