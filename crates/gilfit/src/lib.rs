//! The decision core of gilfit: suggests and decides the values of a form's judged fields from how settled documents
//! were filled. No I/O — a host feeds it settled documents and asks it about new ones.
//!
//! This crate is being built up to the .NET implementation, step by step, against the language-neutral goldens in
//! `tests/fixtures/core`. It is not published yet.

pub mod coarse_groups;
pub mod field_memory;
pub mod form;
pub mod lexical_memory;
pub mod similar_vote;
pub mod text;
pub mod threshold_selection;
