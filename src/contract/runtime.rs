use colored::*;
use wasmtime::*;
use std::fmt;

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
    println!("{} {}{} {}", 
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

pub fn execute_contract(path: &str, func_name: &str) {
    log_info(&format!("Loading contract from: {}", path.yellow()), Some("CONTRACT_LOADER"));

    let engine = Engine::default();
    let module = match Module::from_file(&engine, path) {
        Ok(m) => m,
        Err(e) => {
            log_error(&format!("Failed to load contract: {}", e), Some("CONTRACT_LOADER"));
            return;
        }
    };

    let mut store = Store::new(&engine, ());
    let instance = match Instance::new(&mut store, &module, &[]) {
        Ok(i) => i,
        Err(e) => {
            log_error(&format!("Failed to create instance: {}", e), Some("CONTRACT_LOADER"));
            return;
        }
    };

    log_info("Contract loaded successfully", Some("CONTRACT_LOADER"));

    log_info(&format!("Looking for function '{}'", func_name.cyan()), Some("FUNCTION_LOOKUP"));
    let func = match instance.get_func(&mut store, func_name) {
        Some(f) => f,
        None => {
            log_error(&format!("Function '{}' not found", func_name), Some("FUNCTION_LOOKUP"));
            return;
        }
    };

    log_info(&format!("Executing '{}'", func_name.cyan()), Some("FUNCTION_EXECUTION"));
    match func.call(&mut store, &[], &mut []) {
        Ok(_) => log_info("Contract executed successfully", Some("FUNCTION_EXECUTION")),
        Err(e) => log_error(&format!("Contract execution failed: {}", e), Some("FUNCTION_EXECUTION")),
    }
}

fn execute_increment_with_arg(contract_path: &str, peer_id: &str) {
    log_debug(&format!("Initializing contract execution for peer: {}", peer_id), Some("INCREMENT_OP"));
    
    let engine = Engine::default();
    let module = match Module::from_file(&engine, contract_path) {
        Ok(m) => m,
        Err(e) => {
            log_error(&format!("Failed to load contract: {}", e), Some("INCREMENT_OP"));
            return;
        }
    };
    
    let mut store = Store::new(&engine, ());
    let instance = match Instance::new(&mut store, &module, &[]) {
        Ok(i) => i,
        Err(e) => {
            log_error(&format!("Failed to create instance: {}", e), Some("INCREMENT_OP"));
            return;
        }
    };

    let memory = match instance.get_memory(&mut store, "memory") {
        Some(m) => m,
        None => {
            log_error("Memory not found", Some("INCREMENT_OP"));
            return;
        }
    };

    let init = match instance.get_func(&mut store, "init") {
        Some(f) => f,
        None => {
            log_error("Init function not found", Some("INCREMENT_OP"));
            return;
        }
    };
    
    log_debug("Initializing contract state", Some("INCREMENT_OP"));
    if let Err(e) = init.call(&mut store, &[], &mut []) {
        log_error(&format!("Failed to initialize contract: {}", e), Some("INCREMENT_OP"));
        return;
    }

    let offset = 1024;
    let peer_id_bytes = peer_id.as_bytes();
    if let Err(e) = memory.write(&mut store, offset, peer_id_bytes) {
        log_error(&format!("Failed to write peer_id to memory: {}", e), Some("INCREMENT_OP"));
        return;
    }

    let increment = match instance.get_typed_func::<(i32, i32), i64>(&mut store, "increment") {
        Ok(f) => f,
        Err(e) => {
            log_error(&format!("Failed to get increment function: {}", e), Some("INCREMENT_OP"));
            return;
        }
    };

    log_debug(&format!("Executing increment for peer_id: {}", peer_id), Some("INCREMENT_OP"));
    let result = match increment.call(&mut store, (offset as i32, peer_id_bytes.len() as i32)) {
        Ok(r) => r,
        Err(e) => {
            log_error(&format!("Failed to execute increment: {}", e), Some("INCREMENT_OP"));
            return;
        }
    };

    log_info(&format!("Counter for peer_id `{}` now equals {}", peer_id, result), Some("INCREMENT_OP"));
}

pub fn hardcoded_test_contract() {
    log_info("Starting WASM smart-contract test suite", Some("TEST_SUITE"));
    log_info("=================================", Some("TEST_SUITE"));

    let path = "contracts/counter/target/wasm32-unknown-unknown/release/counter.wasm";
    execute_increment_with_arg(path, "peer_id");
}
