use clap::{Arg, Command};
use peer::Peer;
#[macro_use]
extern crate lazy_static;
use std::path::PathBuf;

mod config;
mod connection;
mod signal;
mod tunnel;
mod db;
mod peer;
mod ui;
mod packets;
mod manager;

use crate::signal::SignalServer;
use crate::db::P2PDatabase;
use crate::ui::print_all_files;

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

lazy_static! {
    pub static ref GLOBAL_DB: P2PDatabase = {
        let matches = create_command().get_matches();

        let db_path = matches.get_one::<String>("db-path")
            .map(|s| s.as_str())
            .unwrap_or("./storage");

        let path = PathBuf::from(db_path);
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        P2PDatabase::new(path.to_str().unwrap()).unwrap()
    };
}

#[tokio::main]
async fn main() {
    let matches = create_command().get_matches();

    print_all_files();

    if matches.get_flag("signal") {
        let signal_server = SignalServer::new().await;
        signal_server.run().await;
    } else {
        let peer = Peer::new().await;
        peer.run().await;
        // run_peer().await;
    }
}