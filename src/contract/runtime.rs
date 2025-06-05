use crate::db::{P2PDatabase, ContractMetadata};
use bincode;
use colored::*;
use std::collections::HashMap;
use std::fmt;
use wasmtime::*;

#[derive(Debug)]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "{}", "DEBUG".blue().bold()),
            LogLevel::Info => write!(f, "{}", "INFO".green().bold()),
            LogLevel::Warn => write!(f, "{}", "WARN".yellow().bold()),
            LogLevel::Error => write!(f, "{}", "ERROR".red().bold()),
        }
    }
}

fn log(level: LogLevel, message: &str, context: Option<&str>) {
    let context_str = context.map_or("".to_string(), |c| format!(" [{}]", c.cyan()));
    println!(
        "{} {}{} {}",
        level,
        "CONTRACT".magenta().bold(),
        context_str,
        message
    );
}

fn log_debug(message: &str, context: Option<&str>) {
    log(LogLevel::Debug, message, context);
}

fn log_info(message: &str, context: Option<&str>) {
    log(LogLevel::Info, message, context);
}

fn log_warn(message: &str, context: Option<&str>) {
    log(LogLevel::Warn, message, context);
}

fn log_error(message: &str, context: Option<&str>) {
    log(LogLevel::Error, message, context);
}

pub fn execute_contract_with_payload(
    path: &str,
    function_name: &str,
    payload: &[u8],
    db: &P2PDatabase,
) -> Result<Vec<u8>, String> {
    log_info(
        &format!("Loading contract from: {}", path.yellow()),
        Some("CONTRACT_LOADER"),
    );

    let engine = Engine::default();
    let module = match Module::from_file(&engine, path) {
        Ok(m) => m,
        Err(e) => {
            log_error(
                &format!("Failed to load contract: {}", e),
                Some("CONTRACT_LOADER"),
            );
            return Err(format!("Failed to load contract: {}", e));
        }
    };

    let mut store = Store::new(&engine, ());
    let instance = match Instance::new(&mut store, &module, &[]) {
        Ok(i) => i,
        Err(e) => {
            log_error(
                &format!("Failed to create instance: {}", e),
                Some("CONTRACT_LOADER"),
            );
            return Err(format!("Failed to create instance: {}", e));
        }
    };

    log_info("Contract loaded successfully", Some("CONTRACT_LOADER"));

    let memory = match instance.get_memory(&mut store, "memory") {
        Some(m) => m,
        None => {
            log_error("Memory not found", Some("CONTRACT_LOADER"));
            return Err("Memory not found".to_string());
        }
    };

    let offset = 1024;
    if let Err(e) = memory.write(&mut store, offset, payload) {
        log_error(
            &format!("Failed to write payload to memory: {}", e),
            Some("CONTRACT_LOADER"),
        );
        return Err(format!("Failed to write payload to memory: {}", e));
    }

    let execute = match instance.get_typed_func::<(i32, i32), i32>(&mut store, function_name) {
        Ok(f) => f,
        Err(e) => {
            log_error(
                &format!("Failed to get function {}: {}", function_name, e),
                Some("CONTRACT_LOADER"),
            );
            return Err(format!("Failed to get function {}: {}", function_name, e));
        }
    };

    log_info(
        &format!("Executing contract function: {}", function_name),
        Some("CONTRACT_EXECUTION"),
    );
    let result_offset = match execute.call(&mut store, (offset as i32, payload.len() as i32)) {
        Ok(r) => r,
        Err(e) => {
            log_error(
                &format!("Contract execution failed: {}", e),
                Some("CONTRACT_EXECUTION"),
            );
            return Err(format!("Contract execution failed: {}", e));
        }
    };

    let mut result_buffer = vec![0u8; 1024];
    if let Err(e) = memory.read(&mut store, result_offset as usize, &mut result_buffer) {
        log_error(
            &format!("Failed to read result from memory: {}", e),
            Some("CONTRACT_EXECUTION"),
        );
        return Err(format!("Failed to read result from memory: {}", e));
    }

    let result_size = result_buffer
        .iter()
        .position(|&x| x == 0)
        .unwrap_or(result_buffer.len());
    let result = result_buffer[..result_size].to_vec();

    // Сохраняем метаданные контракта
    if let Ok(metadata) = bincode::deserialize::<ContractMetadata>(&result) {
        if let Err(e) = db.save_contract_metadata(&metadata) {
            log_warn(
                &format!("Failed to save contract metadata: {}", e),
                Some("CONTRACT_METADATA"),
            );
        } else {
            log_info(
                "Contract metadata saved successfully",
                Some("CONTRACT_METADATA"),
            );
        }
    }

    log_info("Contract executed successfully", Some("CONTRACT_EXECUTION"));
    Ok(result)
}