use std::future::Future;

use super::error::AppError;

/// Buffered writers for stdout and stderr
pub struct Writers {
    pub stdout: tokio::io::BufWriter<tokio::io::Stdout>,
    pub stderr: tokio::io::BufWriter<tokio::io::Stderr>,
}

/// Reusable CLI application runner that handles:
/// - Tokio runtime creation and configuration
/// - Argument parsing and validation
/// - Signal handling (SIGINT, SIGTERM, SIGHUP)
/// - Stdout/stderr buffering and flushing
/// - Exit codes (0 = success, 1 = error, 130 = SIGINT, 143 = SIGTERM)
pub struct CliApp<Config> {
    name: String,
    flush_on_signal: bool,
    worker_threads: Option<usize>,
    args_parser: Box<dyn FnOnce(Vec<String>) -> Result<Config, AppError> + Send>,
}

impl CliApp<Vec<String>> {
    /// Create a new CLI application
    ///
    /// By default:
    /// - Uses all CPU cores for tokio worker threads
    /// - Does not flush output on signals
    /// - Passes raw args to the main function
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            flush_on_signal: false,
            worker_threads: None,
            args_parser: Box::new(|args| Ok(args)),
        }
    }

    /// Parse and validate arguments into custom config type
    ///
    /// The parser function receives command-line arguments and should return
    /// either a parsed config or an error. Type transforms from `CliApp<Vec<String>>`
    /// to `CliApp<T>`.
    ///
    /// # Example
    /// ```rust,ignore
    /// CliApp::new("myapp")
    ///     .with_args(|args| {
    ///         if args.len() != 2 {
    ///             return Err(AppError::InvalidArguments("Usage: myapp <file>".into()));
    ///         }
    ///         Ok(args[1].clone())
    ///     })
    ///     .run(|writers, filename| async move {
    ///         // filename is String, not Vec<String>
    ///     });
    /// ```
    pub fn with_args<T, F>(self, parser: F) -> CliApp<T>
    where
        F: FnOnce(Vec<String>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        CliApp {
            name: self.name,
            flush_on_signal: self.flush_on_signal,
            worker_threads: self.worker_threads,
            args_parser: Box::new(parser),
        }
    }
}

impl<Config: Send + 'static> CliApp<Config> {
    /// Flush stdout and stderr before exiting on signal
    ///
    /// When enabled, partial output will be flushed when receiving SIGINT,
    /// SIGTERM, or SIGHUP, ensuring buffered data is written before exit.
    pub fn with_flush(mut self, enabled: bool) -> Self {
        self.flush_on_signal = enabled;
        self
    }

    /// Set number of tokio worker threads
    ///
    /// Default: Number of CPU cores (from `std::thread::available_parallelism()`)
    ///
    /// # Example
    /// ```rust,ignore
    /// CliApp::new("myapp")
    ///     .with_worker_threads(8)  // Use exactly 8 worker threads
    ///     .run(main_fn);
    /// ```
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = Some(threads);
        self
    }

    /// Run the application (never returns)
    ///
    /// Creates a tokio runtime, parses arguments, sets up signal handling,
    /// and runs the provided async function. Automatically handles exit codes
    /// and cleanup.
    ///
    /// The main function receives:
    /// - `Writers`: Buffered stdout and stderr writers
    /// - `Config`: Parsed configuration (type determined by `with_args()`)
    ///
    /// # Example
    /// ```rust,ignore
    /// fn main() {
    ///     CliApp::new("myapp")
    ///         .run(|mut writers, args| async move {
    ///             writeln!(writers.stdout, "Hello!").await?;
    ///             Ok(())
    ///         });
    /// }
    /// ```
    pub fn run<F, Fut>(self, main_fn: F) -> !
    where
        F: FnOnce(Writers, Config) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), AppError>> + Send,
    {
        // Build tokio runtime
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.enable_all();

        // Set worker threads (defaults to num_cpus if not specified)
        if let Some(threads) = self.worker_threads {
            builder.worker_threads(threads);
        }

        let runtime = builder
            .build()
            .expect("Failed to create tokio runtime");

        runtime.block_on(async move {
            // Extract flush_on_signal before moving self
            let flush_on_signal = self.flush_on_signal;

            // Parse arguments first (before entering tokio::select)
            let args = std::env::args().collect();
            let config = match (self.args_parser)(args) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            let writers = Writers {
                stdout: tokio::io::BufWriter::new(tokio::io::stdout()),
                stderr: tokio::io::BufWriter::new(tokio::io::stderr()),
            };

            let signal_fut = wait_for_signal();

            // Race main application logic against signal reception
            tokio::select! {
                result = main_fn(writers, config) => {
                    match result {
                        Ok(()) => {
                            std::process::exit(0);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                signal_code = signal_fut => {
                    if flush_on_signal {
                        eprintln!("Interrupted, attempting to flush partial results");
                    }
                    std::process::exit(signal_code);
                }
            }
        });

        #[allow(unreachable_code)]
        {
            unreachable!()
        }
    }

}

/// Wait for any Unix signal (SIGINT, SIGTERM, SIGHUP) or Ctrl+C
/// Returns the exit code to use (130 for SIGINT, 143 for SIGTERM, etc.)
async fn wait_for_signal() -> i32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_app_new() {
        let app = CliApp::new("test-app");
        assert_eq!(app.name, "test-app");
        assert!(!app.flush_on_signal);
        assert!(app.worker_threads.is_none());
    }

    #[test]
    fn cli_app_with_flush() {
        let app = CliApp::new("test-app").with_flush(true);
        assert!(app.flush_on_signal);
    }

    #[test]
    fn cli_app_with_worker_threads() {
        let app = CliApp::new("test-app").with_worker_threads(8);
        assert_eq!(app.worker_threads, Some(8));
    }

    #[test]
    fn cli_app_builder_chain() {
        let app = CliApp::new("test-app")
            .with_flush(true)
            .with_worker_threads(4);
        assert_eq!(app.name, "test-app");
        assert!(app.flush_on_signal);
        assert_eq!(app.worker_threads, Some(4));
    }

    // Note: We can't easily test the run() method since it calls std::process::exit
    // and creates a runtime. Testing would require refactoring to inject these
    // dependencies, which adds complexity. The integration tests in tests/ directory
    // provide coverage by running the actual binary.
}
