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

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Rate limiter for controlling log output frequency per category
///
/// Uses a sliding window algorithm to track log timestamps and enforce
/// a maximum rate of logs per second.
struct RateLimiter {
    /// Maximum logs allowed per second (atomic for dynamic updates)
    max_logs_per_second: AtomicUsize,
    /// Sliding window duration (1 second)
    window_duration: Duration,
    /// Timestamps of recent logs (one queue per category)
    timestamps: Mutex<[VecDeque<Instant>; 6]>,
    /// Counter for dropped messages per category
    dropped_counts: Mutex<[usize; 6]>,
    /// Last time we reported dropped messages per category
    last_drop_report: Mutex<[Option<Instant>; 6]>,
}

impl RateLimiter {
    /// Create a new rate limiter with the specified maximum logs per second
    fn new(max_logs_per_second: usize) -> Self {
        Self {
            max_logs_per_second: AtomicUsize::new(max_logs_per_second),
            window_duration: Duration::from_secs(1),
            timestamps: Mutex::new([
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
            ]),
            dropped_counts: Mutex::new([0; 6]),
            last_drop_report: Mutex::new([None; 6]),
        }
    }

    /// Update the maximum logs per second
    fn set_max_logs_per_second(&self, max: usize) {
        self.max_logs_per_second.store(max, Ordering::Relaxed);
    }

    /// Get the current maximum logs per second
    fn get_max_logs_per_second(&self) -> usize {
        self.max_logs_per_second.load(Ordering::Relaxed)
    }

    /// Get the category index for array access
    fn category_index(category: LogCategory) -> usize {
        match category {
            LogCategory::CPU => 0,
            LogCategory::Bus => 1,
            LogCategory::PPU => 2,
            LogCategory::APU => 3,
            LogCategory::Interrupts => 4,
            LogCategory::Stubs => 5,
        }
    }

    /// Check if a log should be allowed based on rate limits
    /// Returns (allowed, dropped_count) where dropped_count is Some(n) if we should report drops
    fn should_allow(&self, category: LogCategory) -> (bool, Option<usize>) {
        let now = Instant::now();
        let idx = Self::category_index(category);

        let mut timestamps = self.timestamps.lock().unwrap();
        let mut dropped_counts = self.dropped_counts.lock().unwrap();
        let mut last_drop_report = self.last_drop_report.lock().unwrap();

        // Remove timestamps outside the sliding window
        let window = &mut timestamps[idx];
        while let Some(&front) = window.front() {
            if now.duration_since(front) > self.window_duration {
                window.pop_front();
            } else {
                break;
            }
        }

        // Check if we're under the rate limit (load atomically)
        let max_logs = self.max_logs_per_second.load(Ordering::Relaxed);
        if window.len() < max_logs {
            window.push_back(now);

            // Check if we need to report dropped messages
            let dropped = dropped_counts[idx];
            if dropped > 0 {
                dropped_counts[idx] = 0;
                last_drop_report[idx] = Some(now);
                return (true, Some(dropped));
            }

            (true, None)
        } else {
            // Rate limit exceeded, drop this log
            dropped_counts[idx] += 1;

            // Report dropped messages once per second
            let should_report = match last_drop_report[idx] {
                None => true,
                Some(last) => now.duration_since(last) >= Duration::from_secs(1),
            };

            if should_report {
                let dropped = dropped_counts[idx];
                dropped_counts[idx] = 0;
                last_drop_report[idx] = Some(now);
                (false, Some(dropped))
            } else {
                (false, None)
            }
        }
    }
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
    /// Rate limiter for controlling log output frequency
    rate_limiter: RateLimiter,
}

impl LogConfig {
    /// Create a new LogConfig with all logging disabled and default rate limit (60 logs/second)
    fn new() -> Self {
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
            rate_limiter: RateLimiter::new(60), // Default: 60 logs per second
        }
    }

    /// Get the global singleton instance
    pub fn global() -> &'static Self {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<LogConfig> = OnceLock::new();
        INSTANCE.get_or_init(LogConfig::new)
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

    /// Set the maximum logs per second per category (rate limit)
    pub fn set_rate_limit(&self, max_logs_per_second: usize) {
        self.rate_limiter
            .set_max_logs_per_second(max_logs_per_second);
    }

    /// Get the current rate limit (maximum logs per second per category)
    pub fn get_rate_limit(&self) -> usize {
        self.rate_limiter.get_max_logs_per_second()
    }

    /// Set the log file path
    ///
    /// Starts a background thread for async file I/O to prevent blocking the emulation.
    /// If a logging thread is already running, it will be stopped and a new one started.
    ///
    /// Returns Ok(()) if successful, or an error if the file cannot be opened.
    pub fn set_log_file(&self, path: PathBuf) -> std::io::Result<()> {
        // Open the file first to validate it works
        let file = OpenOptions::new().create(true).append(true).open(path)?;

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
/// # Rate Limiting
///
/// To prevent log flooding, this function enforces a rate limit of 60 logs per second
/// per category. When the rate limit is exceeded, logs are dropped and a summary
/// message is periodically emitted.
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
/// - Rate limiting prevents log flooding and performance degradation
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
        // Check rate limit before evaluating the message
        let (allowed, dropped_count) = config.rate_limiter.should_allow(category);

        // If we have dropped messages to report, emit a warning
        if let Some(count) = dropped_count {
            if count > 0 {
                let warning = format!(
                    "[{:?}] WARNING: Rate limit exceeded, {} log message(s) dropped in the last second",
                    category, count
                );
                config.write_message(&warning);
            }
        }

        // Only evaluate and log the message if allowed by rate limiter
        if allowed {
            let message = message_fn();
            config.write_message(&message);
        }
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

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(60);

        // Should allow up to 60 logs
        for _ in 0..60 {
            let (allowed, _) = limiter.should_allow(LogCategory::CPU);
            assert!(allowed, "Should allow logs within the rate limit");
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(60);

        // Fill up the rate limit
        for _ in 0..60 {
            limiter.should_allow(LogCategory::CPU);
        }

        // The 61st log should be blocked
        let (allowed, _) = limiter.should_allow(LogCategory::CPU);
        assert!(!allowed, "Should block logs exceeding the rate limit");
    }

    #[test]
    fn test_rate_limiter_per_category() {
        let limiter = RateLimiter::new(60);

        // Fill up CPU category
        for _ in 0..60 {
            limiter.should_allow(LogCategory::CPU);
        }

        // CPU should be blocked
        let (allowed, _) = limiter.should_allow(LogCategory::CPU);
        assert!(!allowed, "CPU category should be blocked");

        // But Bus should still be allowed
        let (allowed, _) = limiter.should_allow(LogCategory::Bus);
        assert!(allowed, "Bus category should still be allowed");
    }

    #[test]
    fn test_rate_limiter_sliding_window() {
        use std::thread::sleep;

        let limiter = RateLimiter::new(5); // Small limit for faster testing

        // Fill up the limit
        for _ in 0..5 {
            limiter.should_allow(LogCategory::CPU);
        }

        // Should be blocked
        let (allowed, _) = limiter.should_allow(LogCategory::CPU);
        assert!(!allowed);

        // Wait for the window to slide (1.1 seconds to ensure we're past the window)
        sleep(Duration::from_millis(1100));

        // Should be allowed again after window slides
        let (allowed, _) = limiter.should_allow(LogCategory::CPU);
        assert!(allowed, "Should allow logs after sliding window expires");
    }

    #[test]
    fn test_rate_limiter_reports_dropped_count() {
        let limiter = RateLimiter::new(5);

        // Fill up the limit
        for _ in 0..5 {
            limiter.should_allow(LogCategory::CPU);
        }

        // Drop some logs
        for _ in 0..10 {
            limiter.should_allow(LogCategory::CPU);
        }

        // Wait for window to slide
        std::thread::sleep(Duration::from_millis(1100));

        // Next log should report dropped count
        let (allowed, dropped) = limiter.should_allow(LogCategory::CPU);
        assert!(allowed, "Should be allowed after window slides");
        assert!(dropped.is_some(), "Should report dropped count");
        // The count might be 9 or 10 depending on timing of the drop report
        assert!(
            dropped.unwrap() >= 9 && dropped.unwrap() <= 10,
            "Should report approximately 10 dropped messages, got {}",
            dropped.unwrap()
        );
    }
}
