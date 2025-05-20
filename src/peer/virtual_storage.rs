use super::peer_api::PeerAPI;
use std::collections::HashMap;
use colored::Colorize;

#[derive(Debug, Clone)]
pub struct FileGroup {
    pub name: String,
    pub files: Vec<String>,
    pub tags: Vec<String>,
}

impl PeerAPI {
    fn display_storage(&self) -> Result<(), String> {
        let files = self.db.get_my_fragments().map_err(|e| e.to_string())?;
        let groups = self.db.get_all_groups().map_err(|e| e.to_string())?;

        let mut file_tree: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut group_tree: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for file in files {
            let path = std::path::Path::new(&file.filename);
            let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("/");

            file_tree
                .entry(parent.to_string())
                .or_insert_with(Vec::new)
                .push((
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file.filename)
                        .to_string(),
                    file.file_hash,
                ));
        }

        for group_name in groups {
            let group_files = self.db.get_files_by_group(&group_name).map_err(|e| e.to_string())?;
            group_tree
                .entry(group_name)
                .or_insert_with(Vec::new)
                .extend(group_files.iter().map(|file| {
                    (file.filename.clone(), file.file_hash.clone())
                }));
        }

        println!(
            "\n{}",
            "╔════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║                    ВИРТУАЛЬНОЕ ХРАНИЛИЩЕ                   ║".cyan()
        );
        println!(
            "{}",
            "╠════════════════════════════════════════════════════════════╣".cyan()
        );

        println!("{}", "║ 📁 ФАЙЛЫ ПО ПАПКАМ:".cyan());
        for (path, files) in file_tree.iter() {
            println!("{}", format!("║ 📁 {}", path).cyan());
            for (filename, hash) in files {
                println!("{}", format!("║   └─ {} ({})", filename, hash).white());
            }
        }

        if !group_tree.is_empty() {
            println!("{}", "║ 📁 ГРУППЫ ФАЙЛОВ:".cyan());
            for (group_name, files) in group_tree.iter() {
                println!("{}", format!("║ 📁 Группа: {}", group_name).cyan());
                for (filename, hash) in files {
                    println!("{}", format!("║   └─ {} ({})", filename, hash).white());
                }
            }
        }

        println!(
            "{}",
            "╚════════════════════════════════════════════════════════════╝".cyan()
        );
        println!("\n{}", "Доступные команды:".yellow());
        println!("{}", "  • move <hash> <new_path> - переместить файл".white());
        println!("{}", "  • delete <hash> - удалить файл".white());
        println!("{}", "  • public <hash> - сделать файл публичным".white());
        println!("{}", "  • private <hash> - сделать файл приватным".white());
        println!("{}", "  • group add <group_name> <hash> [tags...] - добавить файл в группу".white());
        println!("{}", "  • group remove <group_name> <hash> - удалить файл из группы".white());
        println!("{}", "  • group list - показать все группы".white());
        println!("{}", "  • exit - выйти из виртуального хранилища".white());

        Ok(())
    }

    fn add_to_group(&self, group_name: &str, file_hash: &str, tags: Vec<String>) -> Result<(), String> {
        let files = self.db.get_my_fragments().map_err(|e| e.to_string())?;
        
        if !files.iter().any(|f| f.file_hash == file_hash) {
            return Err(format!("Файл с хешем {} не найден", file_hash));
        }

        self.db.add_file_to_group(group_name, file_hash, tags).map_err(|e| e.to_string())?;
        
        Ok(())
    }

    fn remove_from_group(&self, group_name: &str, file_hash: &str) -> Result<(), String> {
        self.db.remove_file_from_group(group_name, file_hash).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn list_groups(&self) -> Result<(), String> {
        let groups = self.db.get_all_groups().map_err(|e| e.to_string())?;
        
        if groups.is_empty() {
            println!("{}", "Нет созданных групп".yellow());
            return Ok(());
        }

        println!("\n{}", "Список групп:".cyan());
        for group_name in groups {
            let files = self.db.get_files_by_group(&group_name).map_err(|e| e.to_string())?;
            println!("{}", format!("📁 Группа: {}", group_name).cyan());
            println!("{}", format!("   Количество файлов: {}", files.len()).white());
        }

        Ok(())
    }

    fn get_group_files(&self, group_name: &str) -> Result<Vec<String>, String> {
        let files = self.db.get_files_by_group(group_name).map_err(|e| e.to_string())?;
        Ok(files.into_iter().map(|f| f.file_hash).collect())
    }
    
    pub async fn virtual_storage_interactive(&self) -> Result<(), String> {
        use std::io::{self, Write};

        loop {
            self.display_storage();

            print!("\n{}", "virtual_storage> ".green());
            io::stdout().flush().map_err(|e| e.to_string())?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| e.to_string())?;
            let input = input.trim();

            if input == "exit" {
                break;
            }

            let parts: Vec<&str> = input.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "move" => {
                    if parts.len() != 3 {
                        println!("{}", "Использование: move <hash> <new_path>".red());
                        continue;
                    }
                    if let Err(e) = self
                        .move_file(parts[1].to_string(), parts[2].to_string())
                        .await
                    {
                        println!("{}", format!("Ошибка при перемещении файла: {}", e).red());
                    } else {
                        println!("{}", "Файл успешно перемещен".green());
                    }
                }
                "delete" => {
                    if parts.len() != 2 {
                        println!("{}", "Использование: delete <hash>".red());
                        continue;
                    }
                    if let Err(e) = self.delete_file(parts[1].to_string()).await {
                        println!("{}", format!("Ошибка при удалении файла: {}", e).red());
                    } else {
                        println!("{}", "Файл успешно удален".green());
                    }
                }
                "public" => {
                    if parts.len() != 2 {
                        println!("{}", "Использование: public <hash>".red());
                        continue;
                    }
                    if let Err(e) = self
                        .change_file_public_access(parts[1].to_string(), true)
                        .await
                    {
                        println!("{}", format!("Ошибка при изменении доступа: {}", e).red());
                    } else {
                        println!("{}", "Файл сделан публичным".green());
                    }
                }
                "private" => {
                    if parts.len() != 2 {
                        println!("{}", "Использование: private <hash>".red());
                        continue;
                    }
                    if let Err(e) = self
                        .change_file_public_access(parts[1].to_string(), false)
                        .await
                    {
                        println!("{}", format!("Ошибка при изменении доступа: {}", e).red());
                    } else {
                        println!("{}", "Файл сделан приватным".green());
                    }
                }
                "group" => {
                    if parts.len() < 2 {
                        println!("{}", "Использование: group <add|remove|list> [параметры...]".red());
                        continue;
                    }
                    match parts[1] {
                        "add" => {
                            if parts.len() < 4 {
                                println!("{}", "Использование: group add <group_name> <hash> [tags...]".red());
                                continue;
                            }
                            let group_name = parts[2];
                            let file_hash = parts[3];
                            let tags: Vec<String> = parts[4..].iter().map(|&s| s.to_string()).collect();
                            
                            if let Err(e) = self.add_to_group(group_name, file_hash, tags) {
                                println!("{}", format!("Ошибка при добавлении в группу: {}", e).red());
                            } else {
                                println!("{}", "Файл успешно добавлен в группу".green());
                            }
                        }
                        "remove" => {
                            if parts.len() != 4 {
                                println!("{}", "Использование: group remove <group_name> <hash>".red());
                                continue;
                            }
                            if let Err(e) = self.remove_from_group(parts[2], parts[3]) {
                                println!("{}", format!("Ошибка при удалении из группы: {}", e).red());
                            } else {
                                println!("{}", "Файл успешно удален из группы".green());
                            }
                        }
                        "list" => {
                            if let Err(e) = self.list_groups() {
                                println!("{}", format!("Ошибка при получении списка групп: {}", e).red());
                            }
                        }
                        _ => {
                            println!("{}", "Неизвестная команда группы. Используйте: add, remove или list".red());
                        }
                    }
                }
                _ => {
                    println!(
                        "{}",
                        "Неизвестная команда. Используйте help для списка команд.".red()
                    );
                }
            }
        }

        Ok(())
    }
} 