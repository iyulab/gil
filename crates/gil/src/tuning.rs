//! The settings the replays chose for a schema, as one record a host keeps between runs — and when they must be chosen
//! again.
//!
//! A host runs the replays ([`crate::replay`]), puts what they chose on the schema ([`Schema::with_layers`]), and captures
//! the result with [`Tuning::capture`] — and, with [`Tuning::promise`], what each layer promised, which
//! [`Tuning::promise_of`] reads back for an answer by its source. It stores the record however it likes (the fields are plain data), puts it back with
//! [`Tuning::apply`] at start-up, and asks [`Tuning::stale`] whether the choice still stands: it does not once this crate's
//! version changed (a new version may score differently), once a setting the choice rests on changed (an attribute's
//! domain, its dependencies, its votes …), or once the settled records grew or shrank by a tenth since the choice — the
//! rule the first consuming application chose by.

use std::collections::BTreeMap;

use crate::decider::Source;
use crate::replay::{LayerThresholds, fnv1a};
use crate::schema::{AttributeRole, Schema};

/// This crate's version — what a tuning records it was chosen with.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One decided attribute's chosen settings, with the promises the replays made for them.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AttributeTuning {
    pub attribute: String,
    pub key_fields: Option<Vec<String>>,
    pub key_concentrations: BTreeMap<String, f64>,
    pub key_threshold: Option<f64>,
    pub typed_key_thresholds: Option<Vec<Option<f64>>>,
    pub memory_threshold: Option<f64>,
    pub memory_floor: Option<f64>,
    pub coarse_key_threshold: Option<f64>,
    pub coarse_memory_threshold: Option<f64>,
    pub coarse_memory_floor: Option<f64>,
    /// The precision the key layer's chosen threshold promised for the next answers; `None` when none was recorded.
    pub key_promise: Option<f64>,
    /// The precision the similar record layer's chosen threshold promised for the next answers.
    pub memory_promise: Option<f64>,
}

/// The chosen settings of a schema's decided attributes and what they were chosen on.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Tuning {
    /// The crate version that chose them ([`VERSION`]).
    pub version: String,
    /// The fingerprint of the settings the choice rests on ([`fingerprint`]).
    pub schema: u64,
    /// How many records were settled when they were chosen.
    pub settled: usize,
    /// The precision the replays were asked to meet.
    pub target_precision: f64,
    pub attributes: Vec<AttributeTuning>,
}

/// Why a tuning no longer stands.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Stale {
    /// Chosen by another version of this crate.
    Version { chosen: String },
    /// A setting the choice rests on changed.
    Schema,
    /// The settled records grew or shrank by a tenth or more since the choice.
    Settled { chosen: usize, now: usize },
}

impl Tuning {
    /// The chosen settings as `schema`'s decided attributes now hold them, chosen on `settled` records for
    /// `target_precision`. The promises are left unrecorded; [`Tuning::promise`] records a layer replay's.
    pub fn capture(schema: &Schema, settled: usize, target_precision: f64) -> Self {
        let attributes = schema
            .attributes()
            .iter()
            .filter(|a| a.role == AttributeRole::Decided)
            .map(|a| AttributeTuning {
                attribute: a.name.clone(),
                key_fields: a.key_fields.clone(),
                key_concentrations: a.key_concentrations.clone(),
                key_threshold: a.key_threshold,
                typed_key_thresholds: a.typed_key_thresholds.clone(),
                memory_threshold: a.memory_threshold,
                memory_floor: a.memory_floor,
                coarse_key_threshold: a.coarse.as_ref().and_then(|c| c.key_threshold),
                coarse_memory_threshold: a.coarse.as_ref().and_then(|c| c.memory_threshold),
                coarse_memory_floor: a.coarse.as_ref().and_then(|c| c.memory_floor),
                key_promise: None,
                memory_promise: None,
            })
            .collect();
        Self {
            version: VERSION.to_owned(),
            schema: fingerprint(schema),
            settled,
            target_precision,
            attributes,
        }
    }

    /// Records the promises of `attribute`'s layer replay: what its chosen key and memory thresholds promised.
    ///
    /// # Panics
    /// When the tuning has no such attribute.
    pub fn promise(&mut self, attribute: &str, layers: &LayerThresholds) {
        let tuned = self
            .attributes
            .iter_mut()
            .find(|a| a.attribute == attribute)
            .unwrap_or_else(|| panic!("the tuning has no attribute '{attribute}'"));
        tuned.key_promise = layers.key.chosen.as_ref().map(|c| c.promise);
        tuned.memory_promise = layers.memory.chosen.as_ref().map(|c| c.promise);
    }

    /// What the layer an answer came from promised for `attribute`: the key layer's promise for an answer from the key
    /// memory, the similar record layer's for one from the similar records; `None` for no answer, an outside judgement,
    /// an attribute the tuning does not name, or a promise it did not record ([`Tuning::promise`]).
    pub fn promise_of(&self, attribute: &str, source: Source) -> Option<f64> {
        let tuned = self.attributes.iter().find(|a| a.attribute == attribute)?;
        match source {
            Source::KeyMemory => tuned.key_promise,
            Source::SimilarRecord => tuned.memory_promise,
            Source::None | Source::External => None,
        }
    }

    /// `schema` with the chosen settings put back on its attributes of the same names; attributes the tuning does not
    /// name are left as they are. A coarse threshold goes on an attribute that has a coarse level, and is dropped
    /// otherwise. The result is checked as [`Schema::new`] checks one — a tuning chosen for another schema may name key
    /// attributes this one does not have — and the error names the first rule broken.
    pub fn apply(&self, schema: &Schema) -> Result<Schema, String> {
        let mut attributes = schema.attributes().to_vec();
        for tuned in &self.attributes {
            let Some(a) = attributes.iter_mut().find(|a| a.name == tuned.attribute) else {
                continue;
            };
            a.key_fields.clone_from(&tuned.key_fields);
            a.key_concentrations.clone_from(&tuned.key_concentrations);
            a.key_threshold = tuned.key_threshold;
            a.typed_key_thresholds
                .clone_from(&tuned.typed_key_thresholds);
            a.memory_threshold = tuned.memory_threshold;
            a.memory_floor = tuned.memory_floor;
            if let Some(level) = a.coarse.as_mut() {
                level.key_threshold = tuned.coarse_key_threshold;
                level.memory_threshold = tuned.coarse_memory_threshold;
                level.memory_floor = tuned.coarse_memory_floor;
            }
        }
        Schema::new(schema.name(), attributes)
    }

    /// Why the tuning no longer stands for `schema` with `settled` records, or `None` while it does: another crate
    /// version, a changed setting the choice rests on, or a change in the settled records of a tenth or more — at least
    /// one record while fewer than ten were settled at the choice.
    pub fn stale(&self, schema: &Schema, settled: usize) -> Option<Stale> {
        if self.version != VERSION {
            return Some(Stale::Version {
                chosen: self.version.clone(),
            });
        }

        if self.schema != fingerprint(schema) {
            return Some(Stale::Schema);
        }

        (settled.abs_diff(self.settled) * 10 >= self.settled.max(10)).then_some(Stale::Settled {
            chosen: self.settled,
            now: settled,
        })
    }
}

/// A stable fingerprint of what a schema's choice rests on: its name and, for each attribute in order, everything but
/// the settings the replays choose (key attributes, key concentrations, thresholds, floors) and the policy — role,
/// several values, evidence, dependencies, domain, values to catch, votes, coarse level. The same on every platform and
/// in every run.
pub fn fingerprint(schema: &Schema) -> u64 {
    // Fields are separated by U+001E and list items by U+001F; an absent setting is `-`, a present list starts with `+`.
    fn list(out: &mut String, items: Option<&Vec<String>>) {
        match items {
            None => out.push('-'),
            Some(items) => {
                out.push('+');
                out.push_str(&items.join("\u{1f}"));
            }
        }
        out.push('\u{1e}');
    }

    let mut text = String::new();
    text.push_str(schema.name());
    text.push('\n');
    for a in schema.attributes() {
        text.push_str(&a.name);
        text.push('\u{1e}');
        text.push_str(match a.role {
            AttributeRole::Observed => "observed",
            AttributeRole::Decided => "decided",
        });
        text.push('\u{1e}');
        text.push_str(if a.multiple { "several" } else { "one" });
        text.push('\u{1e}');
        text.push_str(if a.use_as_evidence { "evidence" } else { "-" });
        text.push('\u{1e}');
        list(&mut text, a.depends_on.as_ref());
        list(&mut text, a.candidates.as_ref());
        list(
            &mut text,
            a.candidates_from
                .as_ref()
                .map(std::slice::from_ref)
                .map(<[String]>::to_vec)
                .as_ref(),
        );
        list(&mut text, a.must_catch.as_ref());
        text.push_str(&a.similar_votes.to_string());
        text.push('\u{1e}');
        match &a.coarse {
            None => text.push('-'),
            Some(level) => {
                text.push_str(&level.prefix.to_string());
                let classes = level.classes.as_ref().map(|t| {
                    t.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                });
                text.push('\u{1e}');
                list(&mut text, classes.as_ref());
            }
        }
        text.push('\n');
    }

    fnv1a(&text)
}
