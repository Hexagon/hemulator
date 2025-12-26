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
//!
//! # Usage
//!
//! ```rust
//! use emu_core::logging::{LogConfig, LogLevel, LogCategory};
//!
//! // Initialize logging from command-line args
//! LogConfig::global().set_level(LogCategory::CPU, LogLevel::Debug);
//!
//! // Check if logging is enabled for a category
//! if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Info) {
//!     eprintln!("CPU: Something happened");
//! }
//! ```

use std::sync::atomic::{AtomicU8, Ordering};

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

    /// Initialize logging from environment variables (for backward compatibility)
    ///
    /// This provides a migration path from the old ENV-based system.
    /// Emits deprecation warnings when ENV variables are detected.
    #[allow(deprecated)]
    pub fn init_from_env(&self) {
        use std::env;

        // Map old ENV variables to new logging system
        let env_mappings = [
            ("EMU_LOG_UNKNOWN_OPS", LogCategory::Stubs, LogLevel::Info),
            ("EMU_LOG_BRK", LogCategory::CPU, LogLevel::Debug),
            ("EMU_LOG_PPU_WRITES", LogCategory::PPU, LogLevel::Debug),
            ("EMU_LOG_IRQ", LogCategory::Interrupts, LogLevel::Info),
            ("EMU_TRACE_PC", LogCategory::CPU, LogLevel::Trace),
            ("EMU_TRACE_NES", LogCategory::CPU, LogLevel::Trace),
            (
                "EMU_TRACE_INTERRUPTS",
                LogCategory::Interrupts,
                LogLevel::Debug,
            ),
            ("EMU_TRACE_INT13", LogCategory::Bus, LogLevel::Debug),
            ("EMU_LOG_TIA_ALL_WRITES", LogCategory::PPU, LogLevel::Debug),
            ("EMU_LOG_TIA_GRP", LogCategory::PPU, LogLevel::Debug),
            ("EMU_LOG_SCANLINE", LogCategory::PPU, LogLevel::Trace),
            ("EMU_LOG_VISIBLE_START", LogCategory::PPU, LogLevel::Debug),
            ("EMU_LOG_ATARI_FRAME", LogCategory::PPU, LogLevel::Info),
            (
                "EMU_LOG_ATARI_FRAME_PIXELS",
                LogCategory::PPU,
                LogLevel::Trace,
            ),
        ];

        let mut any_env_found = false;
        for (env_var, category, level) in env_mappings.iter() {
            if let Ok(val) = env::var(env_var) {
                // Check if it's enabled (1, true, TRUE)
                let enabled = matches!(val.as_str(), "1" | "true" | "TRUE");
                if enabled {
                    any_env_found = true;
                    eprintln!(
                        "DEPRECATION WARNING: Environment variable {} is deprecated. Use command-line flags instead.",
                        env_var
                    );
                    eprintln!(
                        "  Recommended: --log-{} {}",
                        match category {
                            LogCategory::CPU => "cpu",
                            LogCategory::Bus => "bus",
                            LogCategory::PPU => "ppu",
                            LogCategory::APU => "apu",
                            LogCategory::Interrupts => "interrupts",
                            LogCategory::Stubs => "stubs",
                        },
                        match level {
                            LogLevel::Off => "off",
                            LogLevel::Error => "error",
                            LogLevel::Warn => "warn",
                            LogLevel::Info => "info",
                            LogLevel::Debug => "debug",
                            LogLevel::Trace => "trace",
                        }
                    );

                    // Set the logging level
                    let current = self.get_level(*category);
                    if *level > current {
                        self.set_level(*category, *level);
                    }
                }
            }
        }

        if any_env_found {
            eprintln!("\nEnvironment-based logging will be removed in a future version.");
            eprintln!("Please update your workflow to use command-line flags.\n");
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
}

/// Convenience macro for logging
#[macro_export]
macro_rules! log {
    ($category:expr, $level:expr, $($arg:tt)*) => {
        if $crate::logging::LogConfig::global().should_log($category, $level) {
            eprintln!($($arg)*);
        }
    };
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
