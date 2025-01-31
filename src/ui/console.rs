use colored::*;

use crate::GLOBAL_DB;

pub fn print_all_files() {
    let myfiles = GLOBAL_DB.get_all_myfiles();
    
    println!("{}", "My Files:".bold().underline().blue());

    for (filename, fragments) in myfiles {
        println!("{}", format!("Filename: {}", filename).bold().green());
        for fragment in fragments {
            println!("  {}: {}", "UUID Peer".yellow(), fragment.uuid_peer);
            println!("  {}: {}", "Session Key".yellow(), fragment.session_key);
            println!("  {}: {}", "Session".yellow(), fragment.session);
            println!("  {}: {}", "Filename".yellow(), fragment.filename);
        }
        println!();
    }
}