//! Tiered logging: [`log_error!`](crate::log_error), [`log_warn!`](crate::log_warn),
//! [`log_info!`](crate::log_info) (default level) and [`log_debug!`](crate::log_debug).
//!
//! Godot can only print on the main thread, so the macros queue a [`LogMessage`] that the
//! scheduler prints in batches via [`process_logs`]. Before [`init_log_channel`] runs (and in
//! unit tests) they fall back to `println!`. Set the level with [`set_log_level`] or
//! `GdPAIPlanScheduler.set_log_level`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Mutex, OnceLock};

/// Process-wide log level. Defaults to [`LogLevel::Info`].
static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// Log message with level for planner thread logging.
#[derive(Clone, Debug)]
pub struct LogMessage {
    /// Severity the message was logged at; selects the Godot print function.
    pub level: LogLevel,
    /// The formatted text.
    pub message: String,
    /// When the message was logged: milliseconds since the Unix epoch.
    pub timestamp: u64,
}

/// Global channel for sending log messages from planner threads to main thread.
/// Created lazily on first use.
type LogChannel = (Mutex<Sender<LogMessage>>, Mutex<Receiver<LogMessage>>);
static LOG_CHANNEL: OnceLock<LogChannel> = OnceLock::new();

/// Initialize the log channel. Call this once during initialization.
/// Calling it again is harmless: the channel is created only once.
pub fn init_log_channel() {
    LOG_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (Mutex::new(tx), Mutex::new(rx))
    });
}

/// Get a reference to the log sender. Returns None if not initialized, in which case the
/// `log_*!` macros print to stdout/stderr instead.
pub fn get_log_sender() -> Option<&'static Mutex<Sender<LogMessage>>> {
    LOG_CHANNEL.get().map(|(tx, _)| tx)
}

/// Prints every queued log message. Main thread only; the scheduler calls it each frame.
#[cfg(not(test))]
pub fn process_logs() {
    if let Some((_, rx)) = LOG_CHANNEL.get() {
        loop {
            let log_msg = {
                let Ok(receiver) = rx.lock() else {
                    // Mutex poisoned, stop processing
                    break;
                };
                receiver.try_recv()
            };
            match log_msg {
                Ok(log_msg) => {
                    let timestamp = log_msg.timestamp as f64 / 1000.0;
                    match log_msg.level {
                        LogLevel::Error => {
                            godot::prelude::godot_error!(
                                "[GdPAI {:.3}s] {}",
                                timestamp,
                                log_msg.message
                            );
                        }
                        LogLevel::Warn => {
                            godot::prelude::godot_warn!(
                                "[GdPAI {:.3}s] {}",
                                timestamp,
                                log_msg.message
                            );
                        }
                        LogLevel::Info => {
                            godot::prelude::godot_print!(
                                "[GdPAI {:.3}s] {}",
                                timestamp,
                                log_msg.message
                            );
                        }
                        LogLevel::Debug => {
                            godot::prelude::godot_print!(
                                "[GdPAI {:.3}s | debug] {}",
                                timestamp,
                                log_msg.message
                            );
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }
}

/// Test version for builds without a Godot engine: drains the channel without printing, because
/// the Godot print functions cannot be called outside the engine.
#[cfg(test)]
pub fn process_logs() {
    // Drain logs without calling Godot FFI
    if let Some((_, rx)) = LOG_CHANNEL.get() {
        loop {
            let log_msg = {
                let Ok(receiver) = rx.lock() else {
                    break;
                };
                receiver.try_recv()
            };
            match log_msg {
                Ok(_) => continue,
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
    /// Search transitions and stall notices. No per-node detail.
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

/// Returns the current timestamp in milliseconds since Unix epoch (0 if the clock is before it).
pub fn get_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Logs at error severity via `godot_error!`. Always emitted regardless of log level.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        let timestamp = $crate::logger::get_timestamp_ms();
        if let Some(sender) = $crate::logger::get_log_sender() {
            if let Ok(sender) = sender.lock() {
                let _ = sender.send($crate::logger::LogMessage {
                    level: $crate::logger::LogLevel::Error,
                    message,
                    timestamp,
                });
            }
        } else {
            // Channel not initialized - fall back to Rust stdio (e.g., in tests without Godot)
            eprintln!("[GdPAI ERROR] {}", message);
        }
    };
}

/// Logs at warn severity via `godot_warn!` if the current level is [`LogLevel::Warn`] or above.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Warn {
            let message = format!($($arg)*);
            let timestamp = $crate::logger::get_timestamp_ms();
            if let Some(sender) = $crate::logger::get_log_sender() {
                if let Ok(sender) = sender.lock() {
                    let _ = sender.send($crate::logger::LogMessage {
                        level: $crate::logger::LogLevel::Warn,
                        message,
                        timestamp,
                    });
                }
            } else {
                // Channel not initialized - fall back to Rust stdio (e.g., in tests without Godot)
                eprintln!("[GdPAI WARN] {}", message);
            }
        }
    };
}

/// Logs at info severity via `godot_print!` if the current level is [`LogLevel::Info`] or above.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Info {
            let message = format!($($arg)*);
            let timestamp = $crate::logger::get_timestamp_ms();
            if let Some(sender) = $crate::logger::get_log_sender() {
                if let Ok(sender) = sender.lock() {
                    let _ = sender.send($crate::logger::LogMessage {
                        level: $crate::logger::LogLevel::Info,
                        message,
                        timestamp,
                    });
                }
            } else {
                // Channel not initialized - fall back to Rust stdio (e.g., in tests without Godot)
                println!("[GdPAI] {}", message);
            }
        }
    };
}

/// Logs at debug severity via `godot_print!` only when the level is [`LogLevel::Debug`].
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::logger::get_log_level() >= $crate::logger::LogLevel::Debug {
            let message = format!($($arg)*);
            let timestamp = $crate::logger::get_timestamp_ms();
            if let Some(sender) = $crate::logger::get_log_sender() {
                if let Ok(sender) = sender.lock() {
                    let _ = sender.send($crate::logger::LogMessage {
                        level: $crate::logger::LogLevel::Debug,
                        message,
                        timestamp,
                    });
                }
            } else {
                // Channel not initialized - fall back to Rust stdio (e.g., in tests without Godot)
                println!("[GdPAI DEBUG] {}", message);
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
}
