// ! Stream processing with configurable topologies
//!
//! This module provides the `StreamProcessor` API for processing transaction streams
//! with flexible topology configuration:
//!
//! - **Stream Combining**: Chain (sequential) vs Merge (concurrent)
//! - **Parallel Sharding**: Distribute streams across multiple processor shards
//! - **Shard Assignment**: RoundRobin, Sequential, or Custom strategies
//! - **Error Policies**: SkipErrors, AbortOnError, or SilentSkip
//!
//! # Examples
//!
//! ## Single Stream
//! ```rust,ignore
//! use pay::prelude::*;
//! use std::sync::Arc;
//!
//! let mgr = Arc::new(ConcurrentAccountManager::new());
//! let store = Arc::new(ConcurrentTransactionStore::new());
//!
//! StreamProcessor::new(mgr, store, SkipErrors)
//!     .add_stream(csv_stream)
//!     .process()
//!     .await;
//! ```
//!
//! ## Multiple Streams with Sharding
//! ```rust,ignore
//! StreamProcessor::new(mgr, store, SilentSkip)
//!     .with_shards(8)  // 8 parallel processors
//!     .with_stream_combinator(StreamCombinator::Merge)
//!     .add_stream(stream1)
//!     .add_stream(stream2)
//!     // ... add more streams
//!     .process()
//!     .await;
//! ```

pub mod error;
mod processor;

// Primary streaming API
pub use processor::{
    StreamProcessor,
    ShardAssignment,
    StreamCombinator,
    ProcessorResults,
    ShardResult,
};

// Error handling policies
pub use error::{AbortOnError, ErrorPolicy, SilentSkip, SkipErrors};
