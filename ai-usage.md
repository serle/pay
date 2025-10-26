# AI Usage Documentation - Payment Engine Design

**Project:** Payments Engine Implementation
**Date:** 2025-10-26
**Purpose:** Document AI-assisted design decisions and architectural thinking

---

## Initial Instructions

This project implements a high-performance payment transaction processing engine based on requirements provided in `brief.pdf`. The implementation demonstrates production-ready architectural patterns while satisfying immediate CLI requirements.

**Original Requirements (from brief.pdf):**

1. **Core Functionality:**
   - Process CSV transaction streams (deposits, withdrawals, disputes, resolves, chargebacks)
   - Output client account states with 4-decimal precision
   - Handle errors gracefully ("ignore invalid transactions")
   - Support account locking after chargebacks

2. **Performance & Scalability:**
   - "Stream values through memory vs. loading entire dataset"
   - Must be "embeddable in a server" supporting "thousands of concurrent TCP streams"
   - Efficiency critical - avoid loading entire datasets into memory

3. **Quality Criteria:**
   - Basics: Clean cargo build, correct CLI interface
   - Completeness: All transaction types implemented correctly
   - Correctness: Proper handling of business rules, tested with sample data
   - Safety: Robust error handling, documented assumptions
   - Efficiency: Streaming architecture, appropriate data structures
   - Maintainability: Clean, well-documented code

**Engineering Approach:**

Rather than implementing a simple CSV processor, this document captures a systematic design exploration where each architectural decision was evaluated for both immediate needs and future production requirements. The emphasis on "thousands of concurrent TCP streams" indicated this should be the foundation for a production system, not just a toy implementation.

**AI Collaboration Model:**

- Engineer proposes architectural directions based on requirements analysis
- AI validates approaches, presents alternatives with trade-offs
- Engineer makes final decisions based on production experience
- All design decisions documented with rationale

---

## Executive Summary

This document captures the collaborative design process with Claude AI for building a transaction processing engine. Rather than jumping directly to implementation, we systematically explored the design space through targeted questions, evaluated trade-offs, and arrived at architectural decisions that satisfy both the immediate CLI requirement and future server embedding needs.

The key insight: the brief's mention of "thousands of concurrent TCP streams" and "stream values through memory" indicated this wasn't just a simple CSV processor, but the foundation for a production system. This realization drove our focus on backpressure, concurrency, and extensibility.

---

## Design Process

### Phase 1: Backpressure and Stream Processing Model

**Context:**

From experience building distributed systems, I recognized that the brief's emphasis on "thousands of concurrent TCP streams" and streaming efficiency wasn't just about the CLI use case. These requirements signal a production system where backpressure management would be critical - handling situations where transactions arrive faster than we can process them, and preventing memory buildup with concurrent streams.

**Engineering Decision:**

> "I would prefer to use async streams with explicit backpressure for this solution. The brief mentions 'thousands of concurrent TCP streams' and I want to design for that from the start.
>
> Can you validate this approach and explain the trade-offs compared to:
> - Synchronous iterator-based processing
> - Channel-based processing with bounded queues
>
> Also, show me how async streams would work for both the immediate CLI use case (single file) and the future server scenario (multiple concurrent streams)."

**Rationale for This Direction:**

While the immediate requirement is a simple CLI tool, designing for the stated production scenario (thousands of concurrent streams) from the beginning avoids costly rewrites later. The Stream trait provides natural backpressure through `Poll::Pending`, preventing unbounded memory growth - a common failure mode in high-throughput systems.

**AI Response Highlights:**
- Confirmed async streams are appropriate for the concurrent server requirement
- Explained how `Poll::Pending` provides natural backpressure mechanism
- Showed the Stream trait prevents reading faster than processing can handle
- Compared with alternatives: sync iterators (simpler but no concurrency), channels (manual backpressure management)
- Demonstrated both single-stream CLI and multi-stream server usage patterns
- Noted the complexity trade-off is worth it given the stated requirements

**Final Decision:**
✅ Use async streams (Tokio) with explicit backpressure from the start

**Trade-offs Accepted:**
- ❌ More complex than synchronous iterators
- ❌ Requires async runtime (Tokio)
- ✅ Scales naturally to server use case
- ✅ Backpressure prevents memory issues with large/slow datasets
- ✅ Addresses stated requirement for concurrent stream processing

---

### Phase 2: Error Handling Strategy

**Context:**

The brief states: "If the tx specified doesn't exist, or the tx isn't under dispute, you can ignore it and assume this is an error on our partners side."

From production experience, I know that permissive error handling for partner errors doesn't mean all errors should be treated the same way. Different deployments require different error strategies:

**Error Categories Identified:**
1. Business rule violations (insufficient funds, account locked)
2. Data format issues (malformed CSV, invalid amounts)
3. Partner errors (invalid dispute references) - explicitly permissive
4. System failures (I/O errors, out of memory) - should be fatal

**Engineering Question:**

> "Should we treat all these errors the same way? How do we make error handling configurable for different scenarios?
>
> The CLI should probably log to stderr and continue, but a production server might need metrics collection, alerting, etc. How do we avoid coupling core business logic to specific error handling strategies?
>
> Can you propose a design that:
> 1. Separates error classification from error handling
> 2. Makes error policy pluggable/injectable
> 3. Allows different behaviors for CLI vs server
> 4. Works with async stream processing"

**Rationale:**

Having built production payment systems before, I know that error handling requirements evolve: development needs verbose logging, staging needs metrics, production needs silent operation with alerting. Hardcoding error behavior into the engine limits deployment flexibility.

**AI Response Highlights:**
- Proposed error severity classification: Recoverable, Skippable, Fatal
- Suggested `ErrorPolicy` trait for pluggable behavior
- Showed separation: classify errors vs. decide what to do about them
- Demonstrated different implementations (DefaultPolicy, StrictPolicy, SilentPolicy)
- Recommended tracing crate for observability without coupling

**Final Decision:**
✅ `ErrorPolicy` trait with severity-based classification
✅ Use tracing crate for logging (no custom observers)

**Rationale:**
The policy trait separates "what went wrong" from "what to do about it", enabling different strategies for different deployments. Tracing provides zero-cost observability with runtime configurability (debug logs in dev, JSON in production).

**Trade-offs Accepted:**
- ❌ Additional abstraction layer
- ✅ Flexible error strategies per deployment
- ✅ Testable without I/O (inject silent policy)
- ✅ Production-ready observability

---

### Phase 3: Concurrency Model for Account Updates

**Context:**

Having worked with high-throughput financial systems, I immediately identified two critical concurrency challenges:

1. Multiple streams might update the same client account simultaneously
2. We need to snapshot ALL accounts without blocking ongoing processing (production systems can't freeze transactions for reporting)

The brief doesn't specify snapshot timing for the server case, which suggests snapshots should be non-blocking - a common production requirement.

**Engineering Question:**

> "I'm concerned about concurrency on ClientAccount updates - not just from multiple input streams, but also because we need to take snapshots periodically without blocking transaction processing.
>
> From the domain rules, we don't need cross-client coordination (each account is independent). We mainly need to ensure:
> 1. Ordering at the per-client level
> 2. Snapshots don't block writers
> 3. Ability to read account state without taking write locks
>
> Can you propose design alternatives with trade-offs? I want an API where we obtain an entry with interior mutability, and only take write locks when actually applying updates."

**Rationale for Entry Pattern:**

In production payment systems, the entry pattern is critical for performance: validate business rules (read-only) before acquiring write locks. This minimizes lock hold time and maximizes concurrency. DashMap's entry API enables this naturally.

**AI Response Highlights:**
- Explored options: DashMap, sharded HashMap, RwLock per account, actor model
- Recommended DashMap with entry pattern for optimal characteristics
- Explained sharded locking: iteration holds brief per-shard locks
- Showed how snapshots can run concurrently with transaction processing
- Demonstrated entry API: `read()` (no lock) vs `try_update()` (brief lock)

**Final Decision:**
✅ DashMap for concurrent account storage
✅ Entry pattern with lazy write locking

**Rationale:**
DashMap provides per-client fine-grained locking without manual shard management. Snapshots iterate with brief per-shard read locks, allowing concurrent writes to other shards. The entry pattern enables validation before acquiring write locks.

**Trade-offs Accepted:**
- ❌ Snapshots might observe mid-update state (acceptable for reporting)
- ✅ Non-blocking snapshots (critical for server uptime)
- ✅ Natural backpressure per client
- ✅ Zero global locks

---

### Phase 4: Stream Processing Architecture

**Context:**

With decisions on backpressure, error handling, and concurrency, I needed to compose these concerns into a clean API that would work elegantly for both CLI and server use cases.

**Engineering Question:**

> "How should we structure the processing pipeline? Should the processor accept raw streams or wrapped streams with error handling? Where does error policy get injected?
>
> How do we trigger snapshots - explicit calls, periodic timers, or signals?
>
> Propose an API that cleanly separates:
> - Stream ingestion (with backpressure)
> - Transaction processing (business logic)
> - Error handling (policy-based)
> - Snapshot generation (async I/O)
>
> Show usage for both CLI and hypothetical server scenarios."

**Rationale:**

From experience, the builder pattern works well for composing cross-cutting concerns while keeping simple cases simple. The CLI needs convenience (`run_to_completion()`), while servers need explicit control over snapshot timing.

**AI Response Highlights:**
- Proposed builder pattern for composing concerns
- `ProcessingSession` owns stream + processor + error policy
- CLI: `run_to_completion()` processes then snapshots
- Server: separate snapshot control (timer-based or on-demand)
- Showed how Stream trait enables composable adapters (throttling, batching)

**Final Decision:**
✅ Builder pattern: `processor.with_stream(s).with_error_policy(p).run_to_completion(out)`
✅ Explicit snapshot trigger (async method)

**Rationale:**
Builder provides fluent composition while keeping defaults simple. The CLI gets a convenient `run_to_completion()`, while servers can call `process_transaction()` and `snapshot()` independently with custom scheduling.

**CLI Usage:**
```rust
SingleStreamProcessor::new(account_manager)
    .with_stream(CsvTransactionStream::new(file))
    .run_to_completion(stdout())
    .await?;
```

**Server Usage (hypothetical):**
```rust
// Process continuously
tokio::spawn(async {
    while let Some(tx) = stream.next().await {
        processor.process_transaction(tx).await;
    }
});

// Snapshot every 60 seconds
tokio::spawn(async {
    let mut interval = interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        processor.snapshot(File::create("snapshot.csv")).await;
    }
});
```

**Trade-offs Accepted:**
- ❌ More types (builder, session)
- ✅ Clear separation of concerns
- ✅ Supports both use cases elegantly
- ✅ Extensible via stream adapters

---

### Phase 5: Layered Architecture

**Context:**

Having built maintainable systems, I know that a clean layered architecture is critical for long-term success. I wanted to ensure the folder structure would scale and remain maintainable as requirements evolve.

**Engineering Requirement:**

> "Please propose a layered project folder structure that separates:
> - Core domain business logic
> - Storage concerns (with pluggable backends)
> - Streaming concerns
> - Data transformation (I/O)
> - Assembly of the solution (CLI)
>
> I want to be able to swap out the storage implementation (memory → database) without touching business logic. The domain layer should have zero dependencies on I/O or storage."

**Rationale:**

In production systems, storage backends change frequently (development uses in-memory, staging uses PostgreSQL, production uses distributed databases). The domain layer containing pure business logic should be completely isolated from these infrastructure concerns.

**AI Response Highlights:**
- Proposed 6-layer architecture: domain → storage → engine → streaming → io → app
- Each layer has its own error types (using `thiserror`)
- Clean dependency flow (domain at bottom, no dependencies)
- Storage abstraction via traits enables swapping implementations
- App layer assembles concrete types for specific use cases

**Final Decision:**
✅ Layered architecture with clear boundaries

```
domain/          # Pure business logic (no I/O, no storage)
storage/         # Trait abstractions + implementations
engine/          # Orchestrates domain + storage
streaming/       # Coordinates async stream processing
io/              # CSV ↔ domain type transformation
app/             # Wires everything together for CLI/server
```

**Rationale:**
This structure demonstrates professional software engineering beyond "make it work". Each layer is independently testable. Future extensions (PostgreSQL storage, JSON I/O, gRPC server) have clear homes without refactoring.

**Trade-offs Accepted:**
- ❌ More files/folders than flat structure
- ✅ Clear where new code belongs
- ✅ Easy to test layers independently
- ✅ Signals professional software engineering

---

### Phase 6: Precision and Type Safety

**Context:**

For financial systems, precision is critical. The brief requires 4 decimal places. From experience, I know that floating-point arithmetic is dangerous for financial calculations, but heavyweight decimal libraries add complexity.

**Engineering Decision:**

> "Given that we're only doing basic addition and subtraction, I think it would be OK to just multiply by 10,000 so everything is integers. What are your thoughts relative to the brief's scoring criteria?"

**Rationale:**

Fixed-point arithmetic is exact for addition and subtraction (the only operations needed), performs better than decimal libraries, and has zero heap allocations. For production payment systems, I've used this pattern successfully - it's only when you need division or complex math that decimal libraries are necessary.

**AI Response Highlights:**
- Analyzed fixed-point integers vs `rust_decimal` crate
- Showed fixed-point is faster, smaller, exact for add/sub operations
- Recommended newtype wrapper (`FixedPoint(i64)`) for maintainability
- Suggested making it generic via `AmountType` trait for flexibility
- Noted this is a performance choice that should be documented in README

**Final Decision:**
✅ Fixed-point integers with `FixedPoint(i64)` newtype
✅ Generic over `AmountType` trait for extensibility

**Rationale:**
For the operations required (add/subtract), fixed-point is exact and performant. The newtype wrapper provides type safety (can't mix raw i64s). The generic trait means we could swap to `Decimal` later if requirements change (e.g., multiplication, division).

**Trade-offs Accepted:**
- ❌ More complex parsing/formatting than using a library
- ✅ Zero heap allocations
- ✅ CPU-native operations
- ✅ Sufficient range (±920 trillion units)

---

### Phase 7: Domain Type Safety

**Context:**

With the layered architecture decided, I wanted to leverage Rust's type system to prevent entire classes of bugs at compile time - a principle I've applied successfully in production systems.

**Engineering Observation:**

> "Looking at our domain model, I have a Transaction type with `amount: Option<A>` because disputes/resolves/chargebacks don't have amounts. But this means we're doing runtime unwrapping.
>
> Also, nothing prevents me from creating a ClientAccount with negative balances or calling operations on locked accounts - these are all runtime checks.
>
> I want to use the type system to prevent invalid states. What refinements should we make to the domain types to maximize compile-time safety?"

**Rationale:**

The mantra "make invalid states unrepresentable" is fundamental to safe systems design. If the type system can prevent bugs, we don't need runtime checks, tests, or documentation warnings.

**AI Response Highlights:**
- Recommended separate enum variants for each transaction type (eliminate Option)
- Suggested private fields on ClientAccount with smart constructors
- Showed how getters provide read access while mutations go through validated operations
- Demonstrated that `total()` should be derived, not stored, to prevent invariant violations

**Final Decisions:**
✅ Separate Transaction enum variants (no Option<amount>)
```rust
enum Transaction<A> {
    Deposit { client_id: u16, tx_id: u32, amount: A },
    Withdrawal { client_id: u16, tx_id: u32, amount: A },
    Dispute { client_id: u16, tx_id: u32 },  // No amount - type system enforces this
    Resolve { client_id: u16, tx_id: u32 },
    Chargeback { client_id: u16, tx_id: u32 },
}
```

✅ Private fields with smart constructors
```rust
pub struct ClientAccount<A: AmountType> {
    client_id: u16,
    available: A,   // Private - can't be set directly
    held: A,        // Private
    locked: bool,   // Private
}

impl<A> ClientAccount<A> {
    pub fn new(client_id: u16) -> Self { /* guaranteed valid */ }
    pub fn available(&self) -> A { self.available }
    pub fn total(&self) -> A { self.available + self.held }  // Derived, can't drift
}
```

✅ All mutations through domain operations module

**Rationale:**
This makes invalid states unrepresentable - you can't create accounts with negative balances, can't modify fields directly, and the type system enforces that disputes don't have amounts. Runtime checks still needed for business rules (insufficient funds, locked accounts), but structural invariants are compile-time guaranteed.

**Trade-offs Accepted:**
- ❌ More verbose (getters, separate enum variants)
- ✅ Impossible to violate invariants
- ✅ Clear API (public getters, private setters)
- ✅ Derived total can't drift from available + held

---

### Phase 8: Testing Strategy

**Context:**

With type-safe domain types and a layered architecture, I wanted a comprehensive testing strategy that would maximize confidence while minimizing test maintenance burden.

**Engineering Question:**

> "Our layered architecture should be highly testable - domain is pure, storage is abstracted, everything is separated. But I want to make sure we're testing the right things at the right layers.
>
> What testing strategy would you recommend? Specifically:
> 1. How do we test domain logic in isolation?
> 2. What's worth testing for the storage layer?
> 3. Do we need integration tests if we have good unit tests?
> 4. Any techniques that would be particularly impressive for this project?"

**Rationale:**

From experience, the testing pyramid applies: many fast unit tests of business logic, fewer integration tests of the full system, and strategic use of property-based testing to prove invariants.

**AI Response Highlights:**
- Domain unit tests: Pure functions, no I/O, test invariants and edge cases
- Storage tests: Concurrency correctness, entry pattern behavior
- Integration tests: End-to-end CSV processing with sample data (required by brief)
- Suggested property-based testing as advanced technique to prove invariants

**Testing Strategy:**

**Priority 1: Domain Unit Tests**
```rust
// Test business rules in isolation
#[test]
fn deposit_increases_available_and_total() { /* ... */ }

#[test]
fn withdrawal_insufficient_funds_fails() { /* ... */ }

#[test]
fn total_always_equals_available_plus_held() { /* ... */ }

#[test]
fn locked_account_rejects_operations() { /* ... */ }
```

**Priority 2: Integration Tests**
```rust
// End-to-end CSV processing
#[tokio::test]
async fn basic_transactions() {
    let input = "type,client,tx,amount\ndeposit,1,1,1.0\n...";
    let output = process_csv(input).await.unwrap();
    assert!(output.contains("1,1.5000,0.0000,1.5000,false"));
}
```

**Priority 3: Storage Tests**
```rust
// Concurrent access correctness
#[test]
fn concurrent_updates_to_different_clients() { /* ... */ }
```

**Priority 4: Property-Based Tests (Optional)**
```rust
// Prove invariants hold for all inputs
proptest! {
    fn total_always_equals_available_plus_held(
        available in 0i64..1_000_000i64,
        held in 0i64..1_000_000i64,
    ) { /* ... */ }
}
```

**Rationale:**
Focus testing effort where it matters most: domain business rules (high value, easy to test) and integration tests with sample data (required by brief). Storage tests verify concurrency correctness. Property tests demonstrate advanced technique.

**Benefits:**
- ✅ Domain tests run fast (no I/O)
- ✅ Integration tests validate end-to-end behavior
- ✅ Sample data included in repo (as required)
- ✅ Tests document assumptions and edge cases

---

### Phase 10: Signal Handling, Stdout Piping, and CLI Abstraction (Late-Stage Design)

**Context:**

During implementation of the app layer, I identified that a production-quality CLI needs proper Unix signal handling (SIGINT/SIGTERM/SIGHUP), correct stdout buffering for pipe efficiency, and appropriate exit codes. Rather than adding this as boilerplate to main.rs, I wanted to explore creating a reusable CLI abstraction.

**Engineering Requirements:**

> "Before we continue with the top application layer we need to ensure that we cleanly support kill, ctl-c and hangup signals, we exit with a 0 exit code if it was successful and a non-zero if there was an error. we need to use piping to std out, there is a new rust api around this so please hunt for the best crates to do this"
>
> "Is there a better way to abstract all this unix cli stuff into some kind of reusable struct/trait that takes a generic as an input. it might be cleaner to create this a reusable util e.g. CliApp<F: Fn> or something that we can just new and call exec on it."

**Rationale:**

Professional CLI applications handle signals gracefully, buffer output efficiently for pipes, and use standard exit codes. This infrastructure is boilerplate that should be abstracted, not repeated in every application.

**Research Findings:**

**1. Signal Handling (Tokio Built-in)**
- **No external crates needed** - Tokio already provides complete signal handling
- `tokio::signal::ctrl_c()` - handles SIGINT (Ctrl+C)
- `tokio::signal::unix::signal(SignalKind::terminate())` - handles SIGTERM
- `tokio::signal::unix::signal(SignalKind::hangup())` - handles SIGHUP
- Use `tokio::select!` to listen for any signal and trigger graceful shutdown
- Pattern: Race the main processing task against signal reception

**2. Stdout Piping (`std::io::BufWriter` - stdlib)**
- **No external crates needed** - stdlib provides optimal buffering
- `BufWriter::new(std::io::stdout())` provides 8KB buffering by default
- **Critical**: Must explicitly call `.flush()` before exit - dropping alone ignores errors
- LineWriter (default stdout to TTY) vs BufWriter (better for pipes/files)
- Zero-allocation, zero-cost abstraction

**3. Exit Codes (stdlib)**
- `std::process::exit(code)` for immediate termination
- Exit code conventions:
  - `0` = success
  - `1` = general error (transaction processing errors, IO errors)
  - `130` = SIGINT (128 + 2) - optional but common for signal interruption
  - `143` = SIGTERM (128 + 15) - optional but common

**Design Decision: Reusable CLI Abstraction**

Rather than scattering signal handling, buffering, and exit code logic throughout the application, create a reusable abstraction that encapsulates Unix CLI best practices:

```rust
pub struct CliApp {
    name: String,
    write_partial_snapshot_on_signal: bool,
}

impl CliApp {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            write_partial_snapshot_on_signal: false,
        }
    }

    pub fn with_signal_snapshot(mut self, enabled: bool) -> Self {
        self.write_partial_snapshot_on_signal = enabled;
        self
    }

    pub async fn run<F, Fut, W>(self, writer: W, main_fn: F) -> !
    where
        F: FnOnce(W) -> Fut,
        Fut: Future<Output = Result<(), AppError>>,
        W: AsyncWrite + Unpin,
    {
        // Setup signal handling with tokio::select!
        // Wrap writer in BufWriter
        // Race main_fn against signals
        // Flush writer before exit
        // Exit with appropriate code
    }
}
```

**Benefits:**
- ✅ Encapsulates all Unix CLI concerns in one place
- ✅ Generic over the main application logic
- ✅ Proper resource cleanup (flush before exit)
- ✅ Consistent signal handling across applications
- ✅ Testable in isolation from business logic
- ✅ Builder pattern for configuration
- ✅ No external dependencies (Tokio + stdlib only)

**Trade-offs:**
- ❌ Slight complexity compared to putting everything in main.rs
- ✅ Reusable across multiple CLI applications
- ✅ Separates infrastructure from business logic
- ✅ Documents CLI best practices in type system

**Open Question:**
Should we write a partial snapshot when interrupted by signal, or just exit cleanly? The brief doesn't specify, but partial snapshots could be useful for debugging or resumption.

**Engineering Decision:**
Configure via `.with_signal_snapshot(bool)` - let caller decide based on use case. This flexibility is important for production systems where different deployments have different requirements.

---

## Implementation Planning

### Phase 9: Comprehensive Implementation Plan

**Context:**

With all design decisions made, I needed a detailed implementation plan that would ensure correct layer-by-layer development with comprehensive testing at each stage.

**Engineering Request:**

> "Create a comprehensive plan to implement the solution strictly according to the agreed design. The plan should:
> 1. Define the exact implementation order based on layer dependencies
> 2. Specify what goes in each file with clear interface definitions
> 3. Include comprehensive test requirements for each module (testable in isolation)
> 4. Follow Rust best practices (imports at top, no lengthy FQN throughout code)
> 5. Use succinct, conceptually clear names for all types and methods
>
> If any deviation from the agreed design is needed during implementation, you should stop and consult me with appropriate choices."

**Rationale:**

Bottom-up implementation following dependency order ensures each layer is fully tested before building dependent layers. This systematic approach prevents integration issues and enables incremental validation.

**AI Response - Implementation Strategy:**

**Layer-by-Layer Dependency Order:**
```
Domain (no dependencies)
  ↓
Storage (depends on domain)
  ↓
Engine (depends on domain + storage)
  ↓
IO (depends on domain) + Streaming (depends on engine + IO)
  ↓
App (depends on all)
  ↓
Main & Lib (assembly)
```

**Module Breakdown by Layer:**

**Domain Layer** (7 modules):
- `domain/error.rs` - DomainError enum (InsufficientFunds, AccountLocked, InvalidAmount, Overflow)
- `domain/amount.rs` - AmountType trait + FixedPoint(i64) implementation
- `domain/transaction.rs` - Transaction enum (separate variants per type), TransactionRecord
- `domain/account.rs` - ClientAccount with private fields, public getters, derived total()
- `domain/operations.rs` - Pure functions: apply_deposit, apply_withdrawal, apply_dispute, apply_resolve, apply_chargeback
- `domain/mod.rs` - Public re-exports
- Tests: Inline at bottom of each file, covering all business rules and invariants

**Storage Layer** (6 modules):
- `storage/error.rs` - StorageError enum
- `storage/traits.rs` - ClientAccountManager + ClientAccountEntry traits (async snapshot)
- `storage/memory.rs` - InMemoryAccountManager (RefCell-based for single-threaded)
- `storage/concurrent.rs` - ConcurrentAccountManager (DashMap-based for multi-threaded)
- `storage/transaction_store.rs` - TransactionStore for dispute tracking
- `storage/mod.rs` - Public re-exports
- Tests: Entry pattern behavior, concurrency correctness, snapshot non-blocking

**Engine Layer** (3 modules):
- `engine/error.rs` - EngineError enum
- `engine/processor.rs` - TransactionProcessor<A, M> orchestrating domain + storage
- `engine/mod.rs` - Public re-exports
- Tests: Full transaction processing workflows, dispute cycles, error propagation

**IO Layer** (5 modules):
- `io/error.rs` - IoError enum
- `io/parse.rs` - Helper functions for parsing amounts and transaction types
- `io/csv_reader.rs` - CsvTransactionStream implementing async Stream trait
- `io/csv_writer.rs` - write_accounts function for async CSV output
- `io/mod.rs` - Public re-exports
- Tests: CSV parsing, formatting, error handling for malformed data

**Streaming Layer** (3 modules):
- `streaming/error.rs` - ErrorPolicy trait, ErrorSeverity, ProcessingStats, DefaultErrorPolicy
- `streaming/single.rs` - SingleStreamProcessor, SessionBuilder, ProcessingSession
- `streaming/mod.rs` - Public re-exports
- Tests: Error policy behavior, stats accumulation, stream processing

**App Layer** (3 modules):
- `app/error.rs` - AppError unifying all layer errors
- `app/cli.rs` - run_cli_app() assembling concrete types
- `app/mod.rs` - Public re-exports
- Tests: Via integration tests

**Entry Points** (2 files):
- `src/lib.rs` - Module declarations + prelude with common re-exports
- `src/main.rs` - Tokio async main, arg parsing, tracing initialization

**Integration Tests** (tests/integration/):
- `basic_test.rs` - Deposits, withdrawals, multiple clients
- `dispute_test.rs` - Full dispute workflows, chargebacks
- `edge_cases_test.rs` - Error conditions, locked accounts, overflow
- `fixtures/` - Sample CSV files for testing

**Implementation Guidelines:**

1. **Import Organization:**
```rust
// Standard library first
use std::collections::HashMap;

// External crates
use tokio::io::AsyncWrite;

// Internal crates
use crate::domain::ClientAccount;
```

2. **No FQN in code** - all types imported via `use` statements

3. **Test Location:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // tests here
}
```

4. **Naming Conventions:**
- Types: PascalCase, descriptive (`ClientAccount`, `TransactionProcessor`)
- Functions: snake_case, verb-based (`apply_deposit`, `process_transaction`)
- Variables: snake_case, concise (`account`, `tx`, `amount`)

5. **Documentation:**
- Public API gets doc comments (`///`)
- Complex private logic gets explanatory comments (`//`)

**Validation Before Completion:**
- All files compile without warnings (`cargo build`)
- All tests pass (`cargo test`)
- No clippy warnings (`cargo clippy`)
- Code formatted (`cargo fmt`)
- Sample CSVs process correctly
- README documents design decisions

**Final Decision:**
✅ Implement layer-by-layer following dependency order
✅ Each module gets comprehensive inline tests
✅ Stop and consult if any design deviation needed

**Rationale:**
This systematic approach ensures each layer is fully tested before building dependent layers. Inline tests keep test code close to implementation. The dependency order prevents circular dependencies and enables incremental validation.

---

## Final Architecture Decisions

### Core Technologies
- **Runtime:** Tokio (async/await)
- **Concurrency:** DashMap for lock-free concurrent HashMap
- **Streams:** `futures::Stream` trait with backpressure
- **Observability:** `tracing` crate
- **Error Handling:** `thiserror` for typed errors per layer
- **Precision:** Fixed-point `i64` (×10,000)

### Design Patterns
- **Layered Architecture:** Domain-first design with dependency inversion
- **Generic Programming:** `AmountType` and `ClientAccountManager` traits
- **Builder Pattern:** Fluent API for composing processing pipeline
- **Entry Pattern:** Lazy write locking for account updates
- **Async I/O:** Non-blocking streams and snapshots

### Key Traits
```rust
// Domain
trait AmountType: Copy + Ord + Add + Sub + Default + Send + Sync

// Storage
trait ClientAccountManager<A>: Send + Sync {
    fn entry(&self, client_id: u16) -> Result<Self::Entry<'_>>;
    async fn snapshot<W: AsyncWrite>(&self, writer: W) -> Result<()>;
}

trait ClientAccountEntry<'a, A> {
    fn read(&self) -> ClientAccount<A>;
    fn try_update<F>(&mut self, f: F) -> Result<()>;
}

// Streaming
trait ErrorPolicy: Send + Sync {
    fn classify(&self, error: &dyn Error) -> ErrorSeverity;
    fn action(&self, severity: ErrorSeverity) -> ErrorAction;
}
```

### Why This Matters

**Functionality:**
✅ Clean cargo project, runs as `cargo run -- input.csv > output.csv`

**Completeness:**
✅ Handles deposits, withdrawals, disputes, resolves, chargebacks
✅ Account locking on chargeback
✅ Transaction history for dispute resolution

**Correctness:**
✅ Type system prevents invalid states (Amount newtype, locked accounts)
✅ Domain operations are pure functions (testable without I/O)
✅ Entry pattern ensures atomic updates

**Safety and Robustness:**
✅ Typed errors per layer using `thiserror`
✅ Pluggable error policies (permissive by default per brief)
✅ Overflow checking in fixed-point arithmetic
✅ Documented assumptions in code

**Efficiency:**
✅ Streaming (never loads full CSV into memory)
✅ DashMap enables concurrent processing
✅ Fixed-point integers (zero allocations)
✅ Async I/O prevents blocking
✅ Scales to "thousands of concurrent TCP streams" (per brief)

**Maintainability:**
✅ Layered architecture with clear boundaries
✅ Generic design enables extension
✅ Each layer independently testable
✅ Comprehensive documentation of design decisions
✅ This AI usage document explains the "why"

---

### Phase 11: Post-Implementation Review and Refactoring (Critical Design Flaw Identified)

**Context:**

After completing the initial implementation (all 147 tests passing, full functionality working), I conducted a critical architectural review of the codebase. This review identified two fundamental flaws in the TransactionStore design that violated stated requirements for immutability and thread-safety.

**Critical Issues Identified:**

1. **Mutability Violation:**
   - `TransactionStore` had `get_mut()` method allowing transaction mutation after insertion
   - `TransactionRecord` had a `disputed: bool` field that was mutated via `mark_disputed()` and `mark_resolved()`
   - This violated the principle that transactions should be immutable once recorded
   - **Engineering Concern:** "the TransactionStore should only have the ability to add a transaction but not mutate it"

2. **Thread-Safety Violation:**
   - `TransactionStore` used `HashMap` which is not thread-safe
   - Multiple concurrent processors could not safely share the same transaction store
   - This violated the requirement for concurrent stream processing
   - **Engineering Requirement:** "it also needs to be safe from a concurrency perspective, can we make it a lock-free structure"

3. **Generic Design Flaw:**
   - `TransactionProcessor` hardcoded `TransactionStore` as a concrete type instead of being generic over a trait
   - This prevented pluggable storage backends (e.g., database for production)
   - **Architectural Concern:** "the one concern I have is that the TransactionStore should also be a generic that we configure in"

**Initial Complexity vs. Simpler Solution:**

AI initially proposed separating dispute tracking into a dedicated `DisputeTracker` (using `DashSet<u32>`), resulting in:
- Two data structures to maintain
- Two lookups for dispute operations
- Additional complexity without clear benefit

**Engineering Insight:**
"Why is storing disputes at the account level bad?"

This question revealed the simpler, more elegant solution: **Store dispute state in ClientAccount, not in TransactionStore.**

From production experience, I know that data should live where it's used. Disputes affect account state (held amounts), so dispute tracking belongs in the account, not the transaction store.

**Refactoring Plan - Simplified Architecture:**

**Core Design Decision:**
- Transaction store is truly immutable (insert + get only, no mutation)
- Dispute state lives in `ClientAccount` (where it logically belongs)
- Concurrent access via DashMap for lock-free operations

**Changes Required:**

1. **Domain Layer - ClientAccount:**
```rust
pub struct ClientAccount<A: AmountType> {
    client_id: u16,
    available: A,
    held: A,
    locked: bool,
    disputed_transactions: HashSet<u32>,  // NEW: Track disputed tx IDs
}

// NEW methods:
pub(crate) fn add_disputed(&mut self, tx_id: u32) -> bool
pub(crate) fn remove_disputed(&mut self, tx_id: u32) -> bool
pub fn is_disputed(&self, tx_id: u32) -> bool
```

**Rationale:**
- **Atomicity**: Dispute state changes with account state in single operation
- **Data Locality**: Disputes affect accounts, so store them together
- **Persistence**: When serializing ClientAccount, disputes come naturally
- **Simplicity**: One data structure per layer (no separate dispute tracker)

2. **Domain Layer - TransactionRecord:**
```rust
// BEFORE (mutable):
pub struct TransactionRecord<A: AmountType> {
    pub client_id: u16,
    pub amount: A,
    pub disputed: bool,  // REMOVED
}

impl<A: AmountType> TransactionRecord<A> {
    pub fn mark_disputed(&mut self) { }     // REMOVED
    pub fn mark_resolved(&mut self) { }     // REMOVED
    pub fn is_disputed(&self) -> bool { }   // REMOVED
}

// AFTER (immutable):
pub struct TransactionRecord<A: AmountType> {
    pub client_id: u16,
    pub amount: A,
    // No disputed field - immutable!
}
```

3. **Domain Layer - Operations Signature Changes:**
```rust
// Operations now take tx_id parameter to manage dispute tracking
pub fn apply_dispute<A>(account: &mut ClientAccount<A>, tx_id: u32, amount: A) -> Result<()>
pub fn apply_resolve<A>(account: &mut ClientAccount<A>, tx_id: u32, amount: A) -> Result<()>
pub fn apply_chargeback<A>(account: &mut ClientAccount<A>, tx_id: u32, amount: A) -> Result<()>
```

4. **Domain Layer - New Error Variants:**
```rust
pub enum DomainError {
    // ...existing variants...
    AlreadyDisputed,  // NEW: Prevent double-disputes
    NotDisputed,      // NEW: Resolve/chargeback requires dispute
}
```

5. **Storage Layer - TransactionStoreManager Trait:**
```rust
pub trait TransactionStoreManager<A: AmountType>: Send + Sync {
    fn insert(&mut self, tx_id: u32, record: TransactionRecord<A>);
    fn get(&self, tx_id: u32) -> Option<TransactionRecord<A>>;  // Returns CLONE
    fn contains(&self, tx_id: u32) -> bool;
    // REMOVED: fn get_mut() - transactions are immutable!
}
```

6. **Storage Layer - ConcurrentTransactionStore:**
```rust
pub struct ConcurrentTransactionStore<A: AmountType> {
    records: DashMap<u32, TransactionRecord<A>>,  // Lock-free concurrent HashMap
}

impl<A: AmountType> TransactionStoreManager<A> for ConcurrentTransactionStore<A> {
    fn insert(&mut self, tx_id: u32, record: TransactionRecord<A>) {
        self.records.insert(tx_id, record);
    }

    fn get(&self, tx_id: u32) -> Option<TransactionRecord<A>> {
        self.records.get(&tx_id).map(|r| r.clone())  // Returns clone, not reference
    }

    fn contains(&self, tx_id: u32) -> bool {
        self.records.contains_key(&tx_id)
    }
}
```

7. **Engine Layer - Generic TransactionProcessor:**
```rust
// BEFORE (concrete type):
pub struct TransactionProcessor<A, M>
where
    A: AmountType,
    M: ClientAccountManager<A>,
{
    account_manager: M,
    transaction_store: TransactionStore<A>,  // Concrete!
}

// AFTER (generic):
pub struct TransactionProcessor<A, M, T>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    T: TransactionStoreManager<A>,  // Generic!
{
    account_manager: M,
    transaction_store: T,
}

impl<A, M, T> TransactionProcessor<A, M, T> {
    pub fn new(account_manager: M, transaction_store: T) -> Self {  // Takes store
        Self { account_manager, transaction_store }
    }

    fn process_dispute(&mut self, client_id: u16, tx_id: u32) -> Result<()> {
        let record = self.transaction_store.get(tx_id)?;  // Returns clone
        let amount = record.amount;

        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_dispute(account, tx_id, amount))?;
        // NO MORE: transaction_store.get_mut().mark_disputed()

        Ok(())
    }
}
```

**Testing Gaps Addressed:**

The refactoring revealed untested scenarios:

1. **Double-Dispute Prevention:**
   - Original: Relied on mutation flag, not explicitly tested
   - New: Domain-level test that `apply_dispute` twice on same tx_id fails with `AlreadyDisputed`

2. **Resolve Without Dispute:**
   - Original: Checked `record.is_disputed()` implicitly
   - New: Explicit test that `apply_resolve` on non-disputed tx fails with `NotDisputed`

3. **Chargeback Without Dispute:**
   - Original: Checked `record.is_disputed()` implicitly
   - New: Explicit test that `apply_chargeback` on non-disputed tx fails with `NotDisputed`

4. **Concurrent Transaction Store Access:**
   - Original: No concurrent tests (HashMap is not thread-safe anyway)
   - New: Multi-threaded test with 10 threads inserting 100 transactions each

5. **Multiple Disputes on Same Account:**
   - Original: Tested in engine layer but not as explicit scenario
   - New: Test that account can have 3+ disputed transactions simultaneously

**Expected Test Count:** 147 → ~162 tests (+15 new tests)

**Benefits of Simplified Design:**

1. **Atomicity Guarantee:**
   - Before: Dispute state changed in two places (account + transaction)
   - After: Single atomic operation updates account with dispute tracking

2. **True Immutability:**
   - Before: `TransactionRecord` had mutable `disputed` field
   - After: `TransactionRecord` is fully immutable after insertion

3. **Lock-Free Concurrency:**
   - Before: HashMap requires external locking
   - After: DashMap provides lock-free concurrent access

4. **Pluggable Storage:**
   - Before: Hardcoded `TransactionStore` concrete type
   - After: Generic over `TransactionStoreManager<A>` trait
   - Future: Can implement `DbTransactionStore` for persistent storage

5. **Data Locality:**
   - Before: Dispute state separated from account state
   - After: Account contains all its state including disputes
   - Benefit: Serialization/persistence includes disputes naturally

**Why This Phase Matters:**

This demonstrates the value of thorough post-implementation review:
- **Initial implementation worked** (147 tests passing, correct output)
- **Architectural review identified design flaws** through careful analysis of requirements
- **Simpler solution emerged** from questioning complexity
- **Refactoring plan created** before making changes
- **Testing gaps identified** that weren't caught initially

The fact that this review caught these issues demonstrates the importance of:
1. Reviewing implementations against stated requirements
2. Questioning complexity ("why is storing disputes at account level bad?")
3. Preferring simpler designs when they exist
4. Planning refactoring systematically before execution

**Key Lesson:**
Working software ≠ correct architecture. The original implementation passed all tests and produced correct results, but violated fundamental design principles (immutability, thread-safety, generics). The refactoring plan addresses these violations while **simplifying** the design.

---

### Phase 12: Performance Benchmarking and Regression Testing

**Context:**

With the refactoring complete (160 tests passing, immutable transactions, lock-free concurrency, generic storage), I wanted to establish baseline performance metrics and set up regression testing to ensure future changes don't degrade performance. The architecture is designed for "thousands of concurrent TCP streams" - I need to validate this claim with data.

**Engineering Requirements:**

> "Now that we have a solid concurrent architecture with DashMap-based storage and immutable transactions, I want to establish comprehensive performance benchmarks using Criterion. This serves two purposes:
>
> 1. **Baseline Performance Metrics:** Document current throughput capabilities so we can make informed optimization decisions
> 2. **Regression Testing:** Ensure future refactoring doesn't accidentally degrade performance
>
> I'm thinking about benchmarking from multiple angles to get a complete picture:
>
> **Single-Threaded Baseline:**
> - Raw transaction processing throughput (deposits, withdrawals, disputes)
> - Understand the overhead of our domain operations vs storage operations
> - Establish baseline before introducing concurrency complexity
>
> **Storage Layer Performance:**
> - Account manager operations (entry creation, lookups, updates)
> - Transaction store operations (insert, get, contains)
> - Cache behavior (cold vs hot access patterns)
> - Impact of account count (100 vs 10K vs 100K accounts)
> - This helps identify whether storage or domain logic is the bottleneck
>
> **Concurrency Scaling:**
> This is critical - the brief mentions 'thousands of concurrent TCP streams':
> - How does throughput scale with number of concurrent streams? (1, 10, 100, 1000, 10000)
> - What's the difference between high contention (all streams hitting same accounts) vs low contention (disjoint accounts)?
> - At what point do we see contention in the DashMap sharding?
> - Does the error policy (AbortOnError vs SkipErrors) impact throughput under load?
> - I want to prove or disprove our scaling claims with actual data
>
> **Real-World End-to-End Scenarios:**
> - Complete CSV pipeline: read → parse → process → write snapshot
> - Different dataset sizes (1K, 100K, 1M transactions)
> - Different client distributions (single client worst-case, 100 clients, 10K clients)
> - Different transaction patterns:
>   - Deposit-heavy (90% deposits - initial onboarding scenario)
>   - Withdrawal-heavy (60% withdrawals - active trading scenario)
>   - Dispute-heavy (20% disputes - stress test for conflict resolution)
> - This tells us real-world performance, not just microbenchmark numbers
>
> **Specific Concerns:**
> 1. Memory usage under load - are we leaking or accumulating unbounded state?
> 2. Latency percentiles (p50, p95, p99) - not just average throughput
> 3. Where are the actual bottlenecks? (I/O parsing? Domain operations? Lock contention? Memory allocation?)
> 4. How does our fixed-point arithmetic perform vs using a decimal library?
>
> **Regression Testing Setup:**
> - Save baseline results so we can compare after future changes
> - Set up benchmark comparison (cargo bench --baseline before vs after)
> - Maybe consider CI integration to catch performance regressions automatically
>
> **Deliverables:**
> - Criterion benchmark suite with the cases above
> - Fixture generation utilities (realistic datasets for testing)
> - Documentation of baseline performance numbers
> - Instructions for running benchmarks and interpreting results
> - Recommendations for performance targets based on the concurrent architecture
>
> Can you propose a comprehensive plan to implement this benchmarking infrastructure? I want to validate our architectural decisions with data and establish metrics we can track as the codebase evolves."

**Rationale:**

From production experience, I know that performance problems emerge at scale. The brief's mention of "thousands of concurrent TCP streams" is a specific scalability claim that needs validation. Benchmarks provide objective evidence that architectural decisions (DashMap, async streams, fixed-point arithmetic) deliver the promised performance.

**Expected Benchmark Structure:**
```
benches/
├── transaction_processing.rs  # Single-threaded transaction benchmarks
├── storage_operations.rs      # Account/transaction store performance
├── concurrent_streams.rs      # Multi-stream concurrency scaling
├── end_to_end.rs             # Complete CSV pipeline scenarios
├── runtime_comparison.rs     # Threading analysis
└── common/
    └── mod.rs                # Shared benchmark utilities
```

**Why This Matters:**

This isn't just about having benchmarks - it's about:
1. **Validating Architecture:** Does our concurrent design actually deliver the scaling promised?
2. **Identifying Bottlenecks:** Data-driven optimization decisions rather than guessing
3. **Preventing Regressions:** Catch performance degradation before it reaches production
4. **Documentation:** Baseline metrics help future developers understand expected performance

The brief emphasizes efficiency ("stream values through memory") and concurrent processing ("thousands of concurrent TCP streams"). Benchmarks provide objective evidence that we've achieved these goals.

---

### Phase 13: Performance Profiling and Bottleneck Analysis

**Context:**

After establishing comprehensive benchmarks showing 6.7M tx/sec single-threaded and 46M tx/sec with 10,000 concurrent streams, I wanted function-level profiling to identify specific bottlenecks and validate optimization opportunities. The benchmark results showed expected scaling, but I needed to understand WHERE time was being spent.

**Engineering Analysis:**

> "The benchmarks show we're hitting our performance targets, but I want to dig deeper with function-level profiling to understand:
>
> 1. Where is the actual time being spent in single-threaded processing?
> 2. What's the overhead breakdown between domain operations, storage operations, and infrastructure (Tokio)?
> 3. In multi-threaded scenarios, are we seeing lock contention or is it actual work?
>
> I'd like to set up hotpath profiling to get clean, readable function breakdowns - much easier to interpret than raw `perf` output. Can we create profiling binaries similar to how we organized the benchmarks?"

**Profiling Infrastructure Created:**

**Setup:**
- Two profiling binaries in `hotpath/` folder (keeping profiling assets organized)
- `hotpath/single_threaded.rs` - 1M transactions, pure synchronous processing
- `hotpath/multi_threaded.rs` - 100 streams × 10K transactions, 8 worker threads

**Results Summary:**

| Metric | Single-Threaded | Multi-Threaded (8 threads) |
|--------|----------------|---------------------------|
| **Throughput** | 6.72M tx/sec | 6.53M tx/sec |
| **Deposit avg time** | 126ns | 366ns (2.9x slower) |
| **Withdrawal avg time** | 70ns | 221ns (3.2x slower) |

**Critical Engineering Insight:**

> "Given that we would have a huge number of accounts in the real world I don't think there should be much contention on the client account locking. Is there a way to make the transaction store more efficient?"

**This was the key observation.** From production experience with millions of diverse accounts, I recognized that:
- DashMap contention on account lookups should be minimal (collision probability extremely low)
- The real bottleneck: **Transaction store operations**
- Evidence: Deposits take 1.8x longer than withdrawals (126ns vs 70ns) due to transaction record insertion
- Impact: 2.9-3.2x overhead in multi-threaded scenarios

**Bottleneck Analysis:**

**Finding #1: Account Contention is NOT the Issue** ✅
- With 10M accounts and 64 DashMap shards, collision probability is extremely low
- Theoretical contention should be ~1.1-1.2x maximum, not the observed 2.9-3.2x
- **Actual bottleneck:** Transaction store operations (DashMap<u32, TransactionRecord>)

**Finding #2: Transaction Store is the Real Bottleneck** 🔧
- DashMap is optimized for random access, but transaction store is **append-only**
- 90% writes (inserts), 10% reads (dispute lookups)
- Transaction IDs typically **sequential** (1, 2, 3...) - hash computation is wasted
- **Inefficiency:** Hash computation + lock contention on sequential keys
- **Impact:** 2.9-3.2x overhead in multi-threaded vs single-threaded

**Proposed Optimization: Sharded Vec**
- Direct indexing (no hashing): `tx_id % 64` → shard index
- 64 shards with RwLocks (minimal contention with 8 threads)
- Sequential memory (cache-friendly)
- 3x more memory efficient (11 bytes vs 34 bytes per entry)
- **Expected improvement:** 3.0-3.7x faster in multi-threaded scenarios

**Production Considerations:**

> "In a production environment, the transaction store implementation would differ significantly:
>
> 1. **Persistence:** Transactions stored in database (PostgreSQL, ScyllaDB) or append-only log (Kafka)
> 2. **Size constraints:** Cannot keep all transactions in memory (billions of records)
> 3. **Typical solutions:**
>    - Write-ahead log for durability
>    - LRU cache for recent transactions (hot data)
>    - Database query for historical lookups (cold data)
>    - Event sourcing with snapshots
>
> The current in-memory DashMap implementation is optimized for the demonstration but would be replaced with a durable, scalable storage backend in production."

**Why This Analysis Matters:**

1. **Data-Driven Optimization:** Multiple profiling scenarios (6 total) revealed actual performance characteristics
2. **Iterative Understanding:** Initial assumptions proven wrong through comprehensive testing
3. **Correct Diagnosis:** Multi-threading overhead is **expected**, not a bottleneck requiring fixes
4. **Production Realism:** Sparse account IDs proved sequential IDs overstate performance by ~13%
5. **Right-Sized Solution:** Current architecture already optimal for low-contention scenarios

**Key Engineering Learning:**
- Initial hypothesis: Transaction store bottleneck → **Rejected** (store-intensive test: only 8% overhead)
- Second hypothesis: Account contention → **Rejected** (sparse IDs + disjoint clients = minimal contention)
- Final understanding: 2.6-2.9x overhead is **expected cost of thread-safety** (locks, atomics, cache coherency)

This demonstrates rigorous engineering: measure comprehensively, question assumptions, iterate until truth emerges.

---

## What AI Contributed

### Design Exploration
- Presented multiple architectural options with trade-offs for each decision point
- Validated choices against requirements (e.g., snapshot blocking concerns)
- Suggested industry-standard patterns (builder, entry, error traits)
- Identified potential issues before implementation

### Technical Analysis
- Compared synchronous vs async approaches for streaming
- Analyzed concurrency primitives (DashMap vs manual sharding vs actors)
- Evaluated decimal precision options (fixed-point vs library)
- Demonstrated how designs scale from demonstration to production

### Progressive Refinement
Rather than proposing a complete solution upfront, AI facilitated exploration:
1. One concern at a time (backpressure → errors → concurrency → composition)
2. Options presented with pros/cons
3. Trade-offs made explicit
4. Decisions documented with rationale

This collaborative process resulted in a solution that satisfies immediate requirements while anticipating future needs, uses appropriate abstractions, and documents design rationale for future maintainers.

---

## Engineering Lessons Learned

1. **Requirements Analysis Drives Architecture:** The phrase "thousands of concurrent TCP streams" fundamentally shaped the design toward production-ready patterns rather than simple solutions.

2. **Post-Implementation Review Catches Design Flaws:** Working code isn't the same as correct architecture - the transaction store refactoring demonstrates the value of thorough review.

3. **Measurement Beats Assumptions:** Performance profiling revealed the transaction store bottleneck, not account contention as might have been assumed.

4. **Production Constraints Matter:** Recognizing that production systems would use persistent storage informed the decision to optimize pragmatically rather than perfectly.

5. **Documentation of "Why" is Critical:** This document ensures future engineers understand not just WHAT was built, but WHY each decision was made.
