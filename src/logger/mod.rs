use colored::*;

static mut SHOW_DEBUG: bool = false;
static mut SHOW_INFO: bool = true;
static mut SHOW_WARNING: bool = true;
static mut SHOW_ERROR: bool = true;

pub fn set_debug(enabled: bool) {
    unsafe { SHOW_DEBUG = enabled; }
}

pub fn set_info(enabled: bool) {
    unsafe { SHOW_INFO = enabled; }
}

pub fn set_warning(enabled: bool) {
    unsafe { SHOW_WARNING = enabled; }
}

pub fn set_error(enabled: bool) {
    unsafe { SHOW_ERROR = enabled; }
}

pub fn debug(message: &str) {
    unsafe {
        if SHOW_DEBUG {
            println!("{}", format!("[DEBUG] {}", message).blue().bold());
        }
    }
}

pub fn info(message: &str) {
    unsafe {
        if SHOW_INFO {
            println!("{}", message);
        }
    }
}

pub fn warning(message: &str) {
    unsafe {
        if SHOW_WARNING {
            println!("{}", format!("[WARNING] {}", message).yellow().bold());
        }
    }
}

pub fn error(message: &str) {
    unsafe {
        if SHOW_ERROR {
            println!("{}", format!("[ERROR] {}", message).red().bold());
        }
    }
}

pub fn peer(message: &str) {
    unsafe {
        if SHOW_INFO {
            println!("{}", format!("[PEER] {}", message).cyan());
        }
    }
}

pub fn turn(message: &str) {
    unsafe {
        if SHOW_INFO {
            println!("{}", format!("[TURN] {}", message).green());
        }
    }
}

pub fn storage(message: &str) {
    unsafe {
        if SHOW_INFO {
            println!("{}", format!("[STORAGE] {}", message).magenta());
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::logger::debug(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logger::info(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logger::warning(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logger::error(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! peer {
    ($($arg:tt)*) => {
        $crate::logger::peer(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! turn {
    ($($arg:tt)*) => {
        $crate::logger::turn(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! storage {
    ($($arg:tt)*) => {
        $crate::logger::storage(&format!($($arg)*))
    };
} 