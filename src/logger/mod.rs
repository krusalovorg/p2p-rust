use colored::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

pub struct Logger {
    show_debug: bool,
    show_info: bool,
    show_warning: bool,
    show_error: bool,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            show_debug: false,
            show_info: true,
            show_warning: true,
            show_error: true,
        }
    }

    pub fn set_level(&mut self, level: LogLevel, enabled: bool) {
        match level {
            LogLevel::Debug => self.show_debug = enabled,
            LogLevel::Info => self.show_info = enabled,
            LogLevel::Warning => self.show_warning = enabled,
            LogLevel::Error => self.show_error = enabled,
        }
    }

    pub fn debug(&self, message: &str) {
        if self.show_debug {
            println!("{}", format!("[DEBUG] {}", message).blue().bold());
        }
    }

    pub fn info(&self, message: &str) {
        if self.show_info {
            println!("{}", message);
        }
    }

    pub fn warning(&self, message: &str) {
        if self.show_warning {
            println!("{}", format!("[WARNING] {}", message).yellow().bold());
        }
    }

    pub fn error(&self, message: &str) {
        if self.show_error {
            println!("{}", format!("[ERROR] {}", message).red().bold());
        }
    }

    pub fn peer(&self, message: &str) {
        if self.show_info {
            println!("{}", format!("[PEER] {}", message).cyan());
        }
    }

    pub fn turn(&self, message: &str) {
        if self.show_info {
            println!("{}", format!("[TURN] {}", message).green());
        }
    }

    pub fn storage(&self, message: &str) {
        if self.show_info {
            println!("{}", format!("[STORAGE] {}", message).magenta());
        }
    }
}

lazy_static::lazy_static! {
    pub static ref LOGGER: Logger = Logger::new();
} 