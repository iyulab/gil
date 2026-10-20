//! The decision core of Gil: answers the decided attributes of a record from how settled records were filled, and
//! replays those records to choose how far each answer can be trusted. No I/O — a host feeds it settled records and
//! asks it about new ones.
//!
//! The vocabulary: a [`schema::Schema`] lists the [`schema::Attribute`]s of a record, each observed or decided; a
//! [`schema::Record`] holds settled values; the [`decider::Decider`] answers a record's decided attributes layer by
//! layer ([`decider::Answer`]); [`replay`] chooses each layer's threshold. The crate reproduces the .NET
//! implementation's language-neutral goldens (`tests/fixtures/core`) to the bit.

pub mod ask_planner;
pub mod coarse_groups;
pub mod decider;
pub mod decision_record;
pub mod hangul;
pub mod key_memory;
pub mod lexical_memory;
pub mod open_record;
pub mod replay;
pub mod schema;
pub mod similar_vote;
pub mod text;
pub mod tuning;
pub mod usage;
