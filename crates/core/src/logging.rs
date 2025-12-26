//! Centralized logging configuration for the emulator.
//!
//! This module provides a unified logging system that replaces the old
//! environment variable-based approach with a more structured command-line
//! configuration system.
//!
//! # Architecture
//!
//! - **LogConfig**: Thread-safe global configuration using atomic operations
//! - **LogLevel**: Hierarchical log levels (Off < Error < Warn < Info < Debug < Trace)
//! - **LogCategory**: Different logging categories (CPU, Bus, PPU, APU, Interrupts, Stubs)
//! - **log()**: Common logging function for all output with async file I/O
//!
//! # Performance
//!
//! Logging is designed to be non-blocking:
//! - Messages are sent to a background thread via a channel
//! - File I/O happens asynchronously, preventing emulation slowdown
//! - Console output is immediate but minimal buffering prevents blocking
//! - Zero overhead when logging is disabled
//!
//! # Usage
//!
//! ```rust
//! use emu_core::logging::{log, LogLevel, LogCategory};
//!
//! // Log with lazy evaluation (zero cost when disabled)
//! log(LogCategory::CPU, LogLevel::Debug, || {
//!     format!("CPU: BRK at PC={:04X}", 0x1234)
//! });
//! ```

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;

/// Log level for controlling verbosity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl LogLevel {
    /// Parse log level from string (case-insensitive)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" | "0" => Some(LogLevel::Off),
            "error" | "err" | "1" => Some(LogLevel::Error),
            "warn" | "warning" | "2" => Some(LogLevel::Warn),
            "info" | "3" => Some(LogLevel::Info),
            "debug" | "4" => Some(LogLevel::Debug),
            "trace" | "5" => Some(LogLevel::Trace),
            _ => None,
        }
    }

    /// Convert to u8 for atomic storage
    fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from u8 for atomic loading
    fn from_u8(val: u8) -> Self {
        match val {
            0 => LogLevel::Off,
            1 => LogLevel::Error,
            2 => LogLevel::Warn,
            3 => LogLevel::Info,
            4 => LogLevel::Debug,
            5 => LogLevel::Trace,
            _ => LogLevel::Off,
        }
    }
}

/// Log category for different emulator components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategory {
    /// CPU execution (instruction execution, PC tracing)
    CPU,
    /// Bus/memory access
    Bus,
    /// PPU/graphics (register writes, rendering)
    PPU,
    /// APU/audio
    APU,
    /// Interrupts (IRQ, NMI)
    Interrupts,
    /// Unimplemented features/stubs
    Stubs,
}

/// Global logging configuration
pub struct LogConfig {
    /// Global log level (applies to all categories unless overridden)
    global_level: AtomicU8,
    /// CPU-specific log level
    cpu_level: AtomicU8,
    /// Bus-specific log level
    bus_level: AtomicU8,
    /// PPU-specific log level
    ppu_level: AtomicU8,
    /// APU-specific log level
    apu_level: AtomicU8,
    /// Interrupt-specific log level
    interrupt_level: AtomicU8,
    /// Stub/unimplemented feature log level
    stub_level: AtomicU8,
    /// Channel for sending log messages to background thread
    log_sender: Mutex<Option<Sender<String>>>,
    /// Flag indicating if logging to file is enabled
    file_logging_enabled: AtomicBool,
}

impl LogConfig {
    /// Create a new LogConfig with all logging disabled
    const fn new() -> Self {
        Self {
            global_level: AtomicU8::new(LogLevel::Off as u8),
            cpu_level: AtomicU8::new(LogLevel::Off as u8),
            bus_level: AtomicU8::new(LogLevel::Off as u8),
            ppu_level: AtomicU8::new(LogLevel::Off as u8),
            apu_level: AtomicU8::new(LogLevel::Off as u8),
            interrupt_level: AtomicU8::new(LogLevel::Off as u8),
            stub_level: AtomicU8::new(LogLevel::Off as u8),
            log_sender: Mutex::new(None),
            file_logging_enabled: AtomicBool::new(false),
        }
    }

    /// Get the global singleton instance
    pub fn global() -> &'static Self {
        static INSTANCE: LogConfig = LogConfig::new();
        &INSTANCE
    }

    /// Set the global log level (applies to all categories unless overridden)
    pub fn set_global_level(&self, level: LogLevel) {
        self.global_level.store(level.to_u8(), Ordering::Relaxed);
    }

    /// Get the global log level
    pub fn get_global_level(&self) -> LogLevel {
        LogLevel::from_u8(self.global_level.load(Ordering::Relaxed))
    }

    /// Set log level for a specific category
    pub fn set_level(&self, category: LogCategory, level: LogLevel) {
        let atomic = match category {
            LogCategory::CPU => &self.cpu_level,
            LogCategory::Bus => &self.bus_level,
            LogCategory::PPU => &self.ppu_level,
            LogCategory::APU => &self.apu_level,
            LogCategory::Interrupts => &self.interrupt_level,
            LogCategory::Stubs => &self.stub_level,
        };
        atomic.store(level.to_u8(), Ordering::Relaxed);
    }

    /// Get log level for a specific category
    pub fn get_level(&self, category: LogCategory) -> LogLevel {
        let atomic = match category {
            LogCategory::CPU => &self.cpu_level,
            LogCategory::Bus => &self.bus_level,
            LogCategory::PPU => &self.ppu_level,
            LogCategory::APU => &self.apu_level,
            LogCategory::Interrupts => &self.interrupt_level,
            LogCategory::Stubs => &self.stub_level,
        };
        LogLevel::from_u8(atomic.load(Ordering::Relaxed))
    }

    /// Check if a message should be logged for the given category and level
    ///
    /// Returns true if:
    /// 1. The category-specific level is set and >= the message level, OR
    /// 2. The category-specific level is Off AND the global level >= the message level
    pub fn should_log(&self, category: LogCategory, level: LogLevel) -> bool {
        let category_level = self.get_level(category);
        if category_level != LogLevel::Off {
            // Category has a specific level set
            level <= category_level
        } else {
            // Fall back to global level
            level <= self.get_global_level()
        }
    }

    /// Reset all logging to Off
    pub fn reset(&self) {
        self.set_global_level(LogLevel::Off);
        self.set_level(LogCategory::CPU, LogLevel::Off);
        self.set_level(LogCategory::Bus, LogLevel::Off);
        self.set_level(LogCategory::PPU, LogLevel::Off);
        self.set_level(LogCategory::APU, LogLevel::Off);
        self.set_level(LogCategory::Interrupts, LogLevel::Off);
        self.set_level(LogCategory::Stubs, LogLevel::Off);
    }

    /// Set the log file path
    ///
    /// Starts a background thread for async file I/O to prevent blocking the emulation.
    /// If a logging thread is already running, it will be stopped and a new one started.
    ///
    /// Returns Ok(()) if successful, or an error if the file cannot be opened.
    pub fn set_log_file(&self, path: PathBuf) -> std::io::Result<()> {
        // Open the file first to validate it works
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        
        // Create a channel for log messages
        let (sender, receiver) = channel::<String>();
        
        // Spawn background thread for async file writing
        thread::Builder::new()
            .name("log-writer".to_string())
            .spawn(move || {
                let mut file = file;
                // Process messages until channel is closed
                while let Ok(message) = receiver.recv() {
                    // Write to file, ignore errors (logging shouldn't crash the app)
                    let _ = writeln!(file, "{}", message);
                    // Flush periodically to ensure logs aren't lost
                    let _ = file.flush();
                }
                // Final flush when shutting down
                let _ = file.flush();
            })?;
        
        // Store the sender
        let mut log_sender = self.log_sender.lock().unwrap();
        *log_sender = Some(sender);
        self.file_logging_enabled.store(true, Ordering::Relaxed);
        
        Ok(())
    }

    /// Clear the log file (close it and stop logging to file)
    pub fn clear_log_file(&self) {
        let mut log_sender = self.log_sender.lock().unwrap();
        *log_sender = None;
        self.file_logging_enabled.store(false, Ordering::Relaxed);
        // Thread will automatically stop when sender is dropped
    }

    /// Write a message to the configured output (file or stderr)
    ///
    /// This is an internal method used by the public log() function.
    /// Uses async I/O for file logging to prevent blocking.
    fn write_message(&self, message: &str) {
        if self.file_logging_enabled.load(Ordering::Relaxed) {
            // Try to send to background thread (non-blocking)
            let log_sender = self.log_sender.lock().unwrap();
            if let Some(ref sender) = *log_sender {
                // Send is non-blocking unless channel is full
                // If send fails, fall back to stderr
                if sender.send(message.to_string()).is_err() {
                    eprintln!("{}", message);
                }
            } else {
                // File logging was enabled but sender is gone, fall back to stderr
                eprintln!("{}", message);
            }
        } else {
            // Write to stderr (immediate, unbuffered)
            eprintln!("{}", message);
        }
    }
}

/// Log a message with the specified category and level
///
/// This is the primary logging function that should be used throughout the codebase.
/// The message is lazily evaluated via a closure, so complex formatting only occurs
/// when logging is actually enabled for the given category and level.
///
/// # Arguments
///
/// * `category` - The logging category (CPU, Bus, PPU, etc.)
/// * `level` - The log level (Error, Warn, Info, Debug, Trace)
/// * `message_fn` - A closure that produces the message string
///
/// # Performance
///
/// - Zero overhead when logging is disabled (closure is never called)
/// - Thread-safe file writing with automatic fallback to stderr
/// - Single point of control for all logging output
///
/// # Examples
///
/// ```rust
/// use emu_core::logging::{log, LogCategory, LogLevel};
///
/// log(LogCategory::CPU, LogLevel::Debug, || {
///     format!("CPU: BRK at PC={:04X}", 0x1234)
/// });
/// ```
pub fn log<F>(category: LogCategory, level: LogLevel, message_fn: F)
where
    F: FnOnce() -> String,
{
    let config = LogConfig::global();
    if config.should_log(category, level) {
        let message = message_fn();
        config.write_message(&message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_str("off"), Some(LogLevel::Off));
        assert_eq!(LogLevel::from_str("OFF"), Some(LogLevel::Off));
        assert_eq!(LogLevel::from_str("0"), Some(LogLevel::Off));

        assert_eq!(LogLevel::from_str("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("ERR"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("1"), Some(LogLevel::Error));

        assert_eq!(LogLevel::from_str("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("WARNING"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("2"), Some(LogLevel::Warn));

        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("3"), Some(LogLevel::Info));

        assert_eq!(LogLevel::from_str("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("4"), Some(LogLevel::Debug));

        assert_eq!(LogLevel::from_str("trace"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_str("TRACE"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_str("5"), Some(LogLevel::Trace));

        assert_eq!(LogLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Off < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
    }

    #[test]
    fn test_log_config_global_level() {
        let config = LogConfig::new();
        assert_eq!(config.get_global_level(), LogLevel::Off);

        config.set_global_level(LogLevel::Info);
        assert_eq!(config.get_global_level(), LogLevel::Info);
    }

    #[test]
    fn test_log_config_category_levels() {
        let config = LogConfig::new();

        // Initially all categories are Off
        assert_eq!(config.get_level(LogCategory::CPU), LogLevel::Off);
        assert_eq!(config.get_level(LogCategory::Bus), LogLevel::Off);

        // Set CPU to Debug
        config.set_level(LogCategory::CPU, LogLevel::Debug);
        assert_eq!(config.get_level(LogCategory::CPU), LogLevel::Debug);
        assert_eq!(config.get_level(LogCategory::Bus), LogLevel::Off);
    }

    #[test]
    fn test_should_log_with_category_level() {
        let config = LogConfig::new();
        config.set_level(LogCategory::CPU, LogLevel::Info);

        // Should log Info and below
        assert!(config.should_log(LogCategory::CPU, LogLevel::Error));
        assert!(config.should_log(LogCategory::CPU, LogLevel::Warn));
        assert!(config.should_log(LogCategory::CPU, LogLevel::Info));

        // Should not log Debug and above
        assert!(!config.should_log(LogCategory::CPU, LogLevel::Debug));
        assert!(!config.should_log(LogCategory::CPU, LogLevel::Trace));
    }

    #[test]
    fn test_should_log_with_global_level() {
        let config = LogConfig::new();
        config.set_global_level(LogLevel::Warn);

        // CPU has no specific level, should use global
        assert!(config.should_log(LogCategory::CPU, LogLevel::Error));
        assert!(config.should_log(LogCategory::CPU, LogLevel::Warn));
        assert!(!config.should_log(LogCategory::CPU, LogLevel::Info));
    }

    #[test]
    fn test_category_level_overrides_global() {
        let config = LogConfig::new();
        config.set_global_level(LogLevel::Error);
        config.set_level(LogCategory::CPU, LogLevel::Debug);

        // CPU should use its specific level (Debug)
        assert!(config.should_log(LogCategory::CPU, LogLevel::Debug));

        // Bus should use global level (Error)
        assert!(!config.should_log(LogCategory::Bus, LogLevel::Warn));
        assert!(config.should_log(LogCategory::Bus, LogLevel::Error));
    }

    #[test]
    fn test_reset() {
        let config = LogConfig::new();
        config.set_global_level(LogLevel::Trace);
        config.set_level(LogCategory::CPU, LogLevel::Debug);
        config.set_level(LogCategory::Bus, LogLevel::Info);

        config.reset();

        assert_eq!(config.get_global_level(), LogLevel::Off);
        assert_eq!(config.get_level(LogCategory::CPU), LogLevel::Off);
        assert_eq!(config.get_level(LogCategory::Bus), LogLevel::Off);
    }
}
