pub mod error;
pub mod processor;

// Re-export commonly used types
pub use error::EngineError;
pub use processor::TransactionProcessor;
