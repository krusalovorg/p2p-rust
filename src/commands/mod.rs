use clap::{Arg, Command};

pub fn create_base_commands() -> Command {
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

pub fn get_db_path(matches: &clap::ArgMatches) -> String {
    matches.get_one::<String>("db-path")
        .map(|s| s.as_str())
        .unwrap_or("./storage").to_string()
}

pub fn get_path_blobs(matches: &clap::ArgMatches) -> String {
    matches.get_one::<String>("db-path")
        .map(|s| format!("{}/blobs", s))
        .unwrap_or("./storage/blobs".to_string())
}

