use crate::engine::EngineError;
use crate::io::IoError;

/// Policy for handling errors during stream processing
pub trait ErrorPolicy: Send + Sync {
    /// Handle an IO error (CSV parsing, reading)
    /// Return true to continue processing, false to abort
    fn handle_io_error(&self, error: IoError) -> bool;

    /// Handle an engine error (transaction processing)
    /// Return true to continue processing, false to abort
    fn handle_engine_error(&self, error: EngineError) -> bool;
}

/// Skip errors and continue processing (log to stderr)
pub struct SkipErrors;

impl ErrorPolicy for SkipErrors {
    fn handle_io_error(&self, error: IoError) -> bool {
        eprintln!("IO error (skipping): {}", error);
        true
    }

    fn handle_engine_error(&self, error: EngineError) -> bool {
        eprintln!("Engine error (skipping): {}", error);
        true
    }
}

/// Abort on first error
pub struct AbortOnError;

impl ErrorPolicy for AbortOnError {
    fn handle_io_error(&self, error: IoError) -> bool {
        eprintln!("IO error (aborting): {}", error);
        false
    }

    fn handle_engine_error(&self, error: EngineError) -> bool {
        eprintln!("Engine error (aborting): {}", error);
        false
    }
}

/// Silent error policy - skip errors without logging
pub struct SilentSkip;

impl ErrorPolicy for SilentSkip {
    fn handle_io_error(&self, _error: IoError) -> bool {
        true
    }

    fn handle_engine_error(&self, _error: EngineError) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainError;

    #[test]
    fn skip_errors_continues_on_io_error() {
        let policy = SkipErrors;
        let error = IoError::InvalidTransactionType("test".to_string());
        assert!(policy.handle_io_error(error));
    }

    #[test]
    fn skip_errors_continues_on_engine_error() {
        let policy = SkipErrors;
        let error = EngineError::TransactionNotFound(123);
        assert!(policy.handle_engine_error(error));
    }

    #[test]
    fn abort_on_error_stops_on_io_error() {
        let policy = AbortOnError;
        let error = IoError::InvalidTransactionType("test".to_string());
        assert!(!policy.handle_io_error(error));
    }

    #[test]
    fn abort_on_error_stops_on_engine_error() {
        let policy = AbortOnError;
        let error = EngineError::TransactionNotFound(123);
        assert!(!policy.handle_engine_error(error));
    }

    #[test]
    fn silent_skip_continues_on_io_error() {
        let policy = SilentSkip;
        let error = IoError::InvalidTransactionType("test".to_string());
        assert!(policy.handle_io_error(error));
    }

    #[test]
    fn silent_skip_continues_on_engine_error() {
        let policy = SilentSkip;
        let error = EngineError::Domain(DomainError::InsufficientFunds);
        assert!(policy.handle_engine_error(error));
    }
}
