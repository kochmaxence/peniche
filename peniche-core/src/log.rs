use colored::Colorize as _;

pub const SUCCESS_EMOJI: &str = "🚣";
pub const ERROR_EMOJI: &str = "🦀";
pub const INFO_EMOJI: &str = "🐸";

/// Helper to print error messages and return the original error
pub fn handle_error<T>(result: anyhow::Result<T>, message: &str) -> anyhow::Result<T> {
    match result {
        Ok(val) => Ok(val),
        Err(err) => {
            eprintln!("{} {}: {}", ERROR_EMOJI, "Error".red().bold(), message);
            Err(err)
        }
    }
}

/// Macro for success messages
#[macro_export]
macro_rules! success_msg {
    ($fmt:expr $(, $args:expr)*) => {
        println!("{} {}", "🚣", format!($fmt $(, $args)*).green());
    }
}

/// Macro for error messages
#[macro_export]
macro_rules! error_msg {
    ($fmt:expr $(, $args:expr)*) => {
        eprintln!("{} {}", "🦀", format!($fmt $(, $args)*).red());
    }
}

/// Macro for informational messages
#[macro_export]
macro_rules! info_msg {
    ($fmt:expr $(, $args:expr)*) => {
        println!("{} {}", "🐸", format!($fmt $(, $args)*).blue());
    }
}
