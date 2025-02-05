use colored::*;

use crate::GLOBAL_DB;

pub fn print_all_files() {
    let myfiles = GLOBAL_DB.get_myfile_fragments();
    
    match myfiles {
        Ok(myfiles) => {
            println!("{}", "My Files:".bold().underline().blue());
            for fragment in myfiles {
                println!("  {}: {}", "UUID Peer".yellow(), fragment.uuid_peer);
                println!("  {}: {}", "Session Key".yellow(), fragment.session_key);
                println!("  {}: {}", "Session".yellow(), fragment.session);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!();
            }
        },
        Err(_) => (()),
    }
}