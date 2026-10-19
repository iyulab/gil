//! Decision records: what each memory layer made of one lookup, written out as a replay asks it. A record is the same
//! line a threshold choice rests on, so a host can recount a choice, compare two versions' decisions or track a layer
//! over time without reaching into the replay. The core hands records to a sink and keeps no copy; how they are stored
//! is the host's business.

use crate::form::SettlementKind;
use crate::form_resolver::Source;

/// What the key layer found for one lookup, whatever the threshold: its best value and the key that backs it most strongly.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyLook {
    /// The best value's score.
    pub strength: f64,
    /// The best value.
    pub value: String,
    /// Whether the best value is the settled one — known on a replay only.
    pub correct: Option<bool>,
    /// The key backing the best value most strongly, as `field: value`.
    pub key: String,
    /// Whether a value the field must not miss was settled under the known keys — then the key layer does not answer.
    pub caught: bool,
}

/// What the similar document layer's vote came to for one lookup, whatever the threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarLook {
    /// The winning value's score.
    pub score: f64,
    /// The winning value.
    pub value: String,
    /// The settled document the winning value came from — its nearest voter.
    pub document: String,
    /// Whether the winning value is the settled one — known on a replay only.
    pub correct: Option<bool>,
    /// The similarity of the nearest document voting for the winning value — what a similarity floor gates on.
    pub nearest: f64,
    /// Whether more than one document voted.
    pub voted: bool,
}

/// What a suggestion answered: the first candidate, when a layer trusted it.
#[derive(Clone, Debug, PartialEq)]
pub struct Answered {
    /// The layer that answered.
    pub layer: Source,
    /// The value answered.
    pub value: String,
    /// The threshold the layer held it to — the field's key or memory threshold; `None` while a person types, when the
    /// key layer is held to the threshold for that many characters instead.
    pub threshold: Option<f64>,
}

/// One lookup of one field. On a replay: a settled document asked about before it is remembered, with what it was
/// settled as. On a suggestion: a document being filled in, with what the layers found and what was answered — whether
/// it was right is known only once the document is settled.
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionRecord {
    /// The document asked about.
    pub record: String,
    /// The field asked about.
    pub field: String,
    /// On a replay, the document's place in settlement order, from 1 — counting documents without the field too.
    pub order: Option<u64>,
    /// How many lookups the document counts as: 0 when its settlement is not counted, one ÷ the draw probability for a
    /// checked value, otherwise 1. A suggestion counts as 1.
    pub weight: f64,
    /// The key layer's best value, when the field memory held documents and a known key led to one.
    pub key: Option<KeyLook>,
    /// The similar document layer's vote, when the document memory held documents and one in the field's domain voted.
    pub similar: Option<SimilarLook>,
    /// What the suggestion answered — on a suggestion; `None` when no layer trusted its first candidate, and on a replay,
    /// which answers nothing (its records hold whatever the layers found, whatever the thresholds).
    pub answered: Option<Answered>,
    /// The settled value — on a replay.
    pub settled: Option<String>,
    /// How the field was settled, where known.
    pub settlement: Option<SettlementKind>,
    /// For a value settled as a check, the probability it was drawn with.
    pub probability: Option<f64>,
}
