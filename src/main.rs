use clap::{Arg, Command};
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

use crate::signal::SignalServer;
use crate::db::P2PDatabase;
use crate::ui::print_all_files;
use crate::contract::runtime::hardcoded_test_contract;

fn create_command() -> Command {
    Command::new("P2P Server")
        .arg(Arg::new("signal")
            .long("signal")
            .action(clap::ArgAction::SetTrue)
            .help("Run as signal server"))
        .arg(Arg::new("db-path")
            .long("db-path")
            .action(clap::ArgAction::Set)
            .value_name("FILE")
            .help("Path to the database directory"))
}

#[tokio::main]
async fn main() {
    hardcoded_test_contract();
    let matches = create_command().get_matches();

    let db_path = matches.get_one::<String>("db-path")
        .map(|s| s.as_str())
        .unwrap_or("./storage");

    let path = PathBuf::from(db_path);
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }
    let db = P2PDatabase::new(path.to_str().unwrap()).unwrap();
    let config: Config = Config::from_file("config.toml");

    print_all_files(&db);

    if matches.get_flag("signal") {
        let signal_server = SignalServer::new(&config, &db).await;
        signal_server.run().await;
    } else {
        let peer = Peer::new(&config, &db).await;
        peer.run().await;
    }
}