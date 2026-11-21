//! Pure logic for SQL Explorer. Everything that can be tested without a device
//! lives here; the app crate only draws, persists and routes messages.
//!
//! This is **not SQLite** and not a general SQL engine: it is a small,
//! in-memory subset for learning and poking at sample tables. [`sql`] documents
//! exactly what is supported; anything else is an error that says so, never a
//! silent partial answer.
//!
//! * [`sql`]: tokenizer, parser, executor (no panics on any input).
//! * [`model`]: the screen state machine, driven by `Key`s.

pub mod model;
pub mod sql;

pub use model::{LoadError, Mode, Model, Out, Saved, Viewport, MAX_HISTORY, MAX_QUERY_CHARS, STATE_VERSION};
pub use sql::{format_table, Database, Outcome, SAMPLE_QUERIES, SUPPORTED};
