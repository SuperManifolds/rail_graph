/// Conditional logging module for development builds
///
/// The `log!` macro provides informational console logging that is compiled out
/// in production (release) builds by default. Errors and warnings should continue
/// using `web_sys::console::error_*` and `web_sys::console::warn_*` directly.
///
/// Logging is enabled when either:
/// - Building in debug mode (`cfg(debug_assertions)`)
/// - The `console_logging` feature is explicitly enabled
///
/// # Examples
///
/// ```rust
/// use crate::logging::log;
///
/// log!("Loading project:", project_name);
/// log!("Performance:", &format!("{:.2}ms", duration));
/// ```
/// Conditionally log to console in development builds
///
/// This macro expands to `web_sys::console::log_1()` in debug builds or when
/// the `console_logging` feature is enabled. In production release builds,
/// it compiles to nothing (zero overhead).
#[macro_export]
macro_rules! log {
    ($($arg:expr),+ $(,)?) => {
        #[cfg(any(debug_assertions, feature = "console_logging"))]
        {
            web_sys::console::log_1(&format!($($arg),+).into());
        }
    };
}

pub use log;
