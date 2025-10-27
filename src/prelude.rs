//! Prelude module for convenient imports
//!
//! Import everything you need with: `use pay::prelude::*;`

// Domain types
pub use crate::domain::{
    AmountType, ClientAccount, DomainError, FixedPoint, Transaction, TransactionRecord,
};

// Storage types
pub use crate::storage::{
    ClientAccountEntry, ClientAccountManager, ConcurrentAccountManager,
    ConcurrentTransactionStore, StorageError, TransactionStoreManager,
};

// Engine types
pub use crate::engine::{EngineError, TransactionProcessor};

// IO types
pub use crate::io::{CsvTransactionStream, IoError, RawTransactionRecord, write_snapshot};

// Streaming types
pub use crate::streaming::{
    AbortOnError, ErrorPolicy, SilentSkip, SkipErrors,
    StreamProcessor, StreamCombinator, ShardAssignment,
};

// App types
pub use crate::app::{AppError, CliApp, Writers};
