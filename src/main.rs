use clap::{Arg, Command};
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

use crate::signal::SignalServer;
use crate::db::P2PDatabase;
use crate::peer::run_peer;
use crate::ui::print_all_files;

lazy_static! {
    pub static ref GLOBAL_DB: P2PDatabase = {
        let matches = Command::new("P2P Server")
            .arg(Arg::new("db-path")
                .long("db-path")
                .action(clap::ArgAction::Set)
                .value_name("FILE")
                .help("Path to the database directory"))
            .get_matches();

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
    let matches = Command::new("P2P Server")
        .arg(Arg::new("signal")
            .long("signal")
            .action(clap::ArgAction::SetTrue)
            .help("Run as signal server"))
        .arg(Arg::new("db-path")
            .long("db-path")
            .action(clap::ArgAction::Set)
            .value_name("FILE")
            .help("Path to the database directory"))
        .get_matches();

    let db = &*GLOBAL_DB;

    print_all_files();

    if matches.get_flag("signal") {
        let signal_server = SignalServer::new();
        signal_server.run().await;
    } else {
        run_peer(db).await;
    }
}