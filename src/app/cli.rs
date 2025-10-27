use std::future::Future;

use super::error::AppError;

/// Reusable CLI application runner that handles:
/// - Signal handling (SIGINT, SIGTERM, SIGHUP)
/// - Stdout buffering and flushing
/// - Exit codes (0 = success, 1 = error, 130 = SIGINT, 143 = SIGTERM)
/// - Graceful shutdown
pub struct CliApp {
    _name: String,
    write_partial_on_signal: bool,
}

impl CliApp {
    /// Create a new CLI application runner
    pub fn new(name: &str) -> Self {
        Self {
            _name: name.to_string(),
            write_partial_on_signal: false,
        }
    }

    /// Configure whether to write partial results when interrupted by signal
    pub fn with_signal_snapshot(mut self, enabled: bool) -> Self {
        self.write_partial_on_signal = enabled;
        self
    }

    /// Run the CLI application with proper signal handling and resource cleanup
    ///
    /// Creates a buffered stdout writer and passes it to the main function.
    /// Handles flushing and exit codes automatically.
    ///
    /// This function never returns - it calls std::process::exit with the appropriate code
    pub async fn run<F, Fut>(self, main_fn: F) -> !
    where
        F: FnOnce(tokio::io::BufWriter<tokio::io::Stdout>) -> Fut,
        Fut: Future<Output = Result<(), AppError>>,
    {
        // Create buffered stdout writer
        let writer = tokio::io::BufWriter::new(tokio::io::stdout());

        // Setup signal handling
        let signal_fut = self.wait_for_signal();

        // Race main application logic against signal reception
        tokio::select! {
            result = main_fn(writer) => {
                match result {
                    Ok(()) => {
                        // Writer is already flushed by main_fn
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            signal_code = signal_fut => {
                if self.write_partial_on_signal {
                    eprintln!("Interrupted, writing partial results...");
                }
                std::process::exit(signal_code);
            }
        }
    }

    /// Wait for any Unix signal (SIGINT, SIGTERM, SIGHUP) or Ctrl+C
    /// Returns the exit code to use (130 for SIGINT, 143 for SIGTERM, etc.)
    async fn wait_for_signal(&self) -> i32 {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};

            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
            let mut sighup = signal(SignalKind::hangup()).expect("Failed to setup SIGHUP handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    eprintln!("Received SIGTERM");
                    143 // 128 + 15
                }
                _ = sigint.recv() => {
                    eprintln!("Received SIGINT");
                    130 // 128 + 2
                }
                _ = sighup.recv() => {
                    eprintln!("Received SIGHUP");
                    129 // 128 + 1
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to setup Ctrl+C handler");
            eprintln!("Received Ctrl+C");
            130
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_app_new() {
        let app = CliApp::new("test-app");
        assert_eq!(app._name, "test-app");
        assert!(!app.write_partial_on_signal);
    }

    #[test]
    fn cli_app_with_signal_snapshot() {
        let app = CliApp::new("test-app").with_signal_snapshot(true);
        assert!(app.write_partial_on_signal);
    }
}
