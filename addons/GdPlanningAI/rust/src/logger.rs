//! Tiered logging for the GdPlanningAI Rust extension.
//!
//! Four log levels map to Godot's print functions:
//! - [`log_error!`] always routes through `godot_error!`.
//! - [`log_warn!`] uses `godot_warn!` when level ≥ [`LogLevel::Warn`].
//! - [`log_info!`] uses `godot_print!` when level ≥ [`LogLevel::Info`] (default).
//! - [`log_debug!`] uses `godot_print!` only at [`LogLevel::Debug`].
//!
//! The level is a single process-wide value. Change it at runtime via
//! [`set_log_level`] (Rust) or `RustPlanningEngine.set_log_level` (GDScript).
//!
//! Debug logs from background threads are sent through a global channel and
//! printed by the scheduler on the main thread.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Mutex, OnceLock};

/// Process-wide log level. Defaults to [`LogLevel::Info`].
static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// Log message with level for background thread logging.
#[derive(Clone, Debug)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
}

/// Global channel for sending log messages from background threads to main thread.
/// Created lazily on first use.
static LOG_CHANNEL: OnceLock<(Mutex<Sender<LogMessage>>, Mutex<Receiver<LogMessage>>)> = OnceLock::new();

/// Initialize the log channel. Call this once during initialization.
pub fn init_log_channel() {
    LOG_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (Mutex::new(tx), Mutex::new(rx))
    });
}

/// Get a reference to the log sender. Returns None if not initialized.
pub fn get_log_sender() -> Option<&'static Mutex<Sender<LogMessage>>> {
    LOG_CHANNEL.get().map(|(tx, _)| tx)
}

/// Process pending log messages on the main thread.
/// Call this from the main thread (e.g., in the scheduler's process_callbacks).
pub fn process_logs() {
    if let Some((_, rx)) = LOG_CHANNEL.get() {
        loop {
            let log_msg = {
                let receiver = rx.lock().unwrap();
                receiver.try_recv()
            };
            match log_msg {
                Ok(log_msg) => {
                    match log_msg.level {
                        LogLevel::Error => {
                            godot::prelude::godot_error!("[GdPAI] {}", log_msg.message);
                        }
                        LogLevel::Warn => {
                            godot::prelude::godot_warn!("[GdPAI] {}", log_msg.message);
                        }
                        LogLevel::Info => {
                            godot::prelude::godot_print!("[GdPAI] {}", log_msg.message);
                        }
                        LogLevel::Debug => {
                            godot::prelude::godot_print!("[GdPAI | debug] {}", log_msg.message);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }
}

/// Verbosity filter for all `log_*!` macros.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum LogLevel {
    /// Errors only. Always routed through Godot's error system.
    Error = 0,
    /// Warnings and errors.
    Warn = 1,
    /// High-level planning lifecycle messages. Default.
    Info = 2,
    /// Per-action trace inside the recursive search. Very verbose.
    Debug = 3,
}

impl LogLevel {
    /// Converts a raw `u8` to a `LogLevel`. Values above 3 map to [`LogLevel::Debug`].
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Error,
            1 => Self::Warn,
            2 => Self::Info,
            _ => Self::Debug,
        }
    }
}

/// Sets the process-wide log level used by all `log_*!` macros.
pub fn set_log_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

/// Returns the current process-wide log level.
pub fn get_log_level() -> LogLevel {
    LogLevel::from_u8(GLOBAL_LOG_LEVEL.load(Ordering::Relaxed))
}

/// Logs at error severity via `godot_error!`. Always emitted regardless of log level.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        {
            let message = format!($($arg)*);
            if let Some(sender) = $crate::logger::get_log_sender() {
                let _ = sender.lock().unwrap().send($crate::logger::LogMessage {
                    level: $crate::logger::LogLevel::Error,
                    message,
                });
            } else {
                // Fallback to direct print if channel not initialized (main thread)
                godot::prelude::godot_error!("[GdPAI] {}", message);
            }
        }
    };
}

/// Logs at warn severity via `godot_warn!` if the current level is [`LogLevel::Warn`] or above.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Warn {
            let message = format!($($arg)*);
            if let Some(sender) = $crate::logger::get_log_sender() {
                let _ = sender.lock().unwrap().send($crate::logger::LogMessage {
                    level: $crate::logger::LogLevel::Warn,
                    message,
                });
            } else {
                // Fallback to direct print if channel not initialized (main thread)
                godot::prelude::godot_warn!("[GdPAI] {}", message);
            }
        }
    };
}

/// Logs at info severity via `godot_print!` if the current level is [`LogLevel::Info`] or above.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Info {
            let message = format!($($arg)*);
            if let Some(sender) = $crate::logger::get_log_sender() {
                let _ = sender.lock().unwrap().send($crate::logger::LogMessage {
                    level: $crate::logger::LogLevel::Info,
                    message,
                });
            } else {
                // Fallback to direct print if channel not initialized (main thread)
                godot::prelude::godot_print!("[GdPAI] {}", message);
            }
        }
    };
}

/// Logs at debug severity via `godot_print!` only when the level is [`LogLevel::Debug`].
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Debug {
            let message = format!($($arg)*);
            if let Some(sender) = $crate::logger::get_log_sender() {
                let _ = sender.lock().unwrap().send($crate::logger::LogMessage {
                    level: $crate::logger::LogLevel::Debug,
                    message,
                });
            } else {
                // Fallback to direct print if channel not initialized (main thread)
                godot::prelude::godot_print!("[GdPAI | debug] {}", message);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u8_maps_all_levels() {
        assert_eq!(LogLevel::from_u8(0), LogLevel::Error);
        assert_eq!(LogLevel::from_u8(1), LogLevel::Warn);
        assert_eq!(LogLevel::from_u8(2), LogLevel::Info);
        assert_eq!(LogLevel::from_u8(3), LogLevel::Debug);
    }

    #[test]
    fn from_u8_out_of_range_gives_debug() {
        assert_eq!(LogLevel::from_u8(4), LogLevel::Debug);
        assert_eq!(LogLevel::from_u8(255), LogLevel::Debug);
    }

    #[test]
    fn level_ordering_matches_verbosity() {
        assert!(LogLevel::Debug > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Warn);
        assert!(LogLevel::Warn > LogLevel::Error);
    }

    #[test]
    fn set_get_log_level_roundtrip() {
        let original = get_log_level();
        set_log_level(LogLevel::Debug);
        assert_eq!(get_log_level(), LogLevel::Debug);
        set_log_level(LogLevel::Error);
        assert_eq!(get_log_level(), LogLevel::Error);
        set_log_level(original);
    }
}
