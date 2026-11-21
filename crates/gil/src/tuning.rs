//! The settings the replays chose for a schema, as one record a host keeps between runs — and when they must be chosen
//! again.
//!
//! A host runs the replays ([`crate::replay`]), puts what they chose on the schema ([`Schema::with_layers`]), and captures
//! the result with [`Tuning::capture`] — and, with [`Tuning::promise`], what each layer promised, which
//! [`Tuning::promise_of`] reads back for an answer by its source. It stores the record however it likes (the fields are plain data), puts it back with
//! [`Tuning::apply`] at start-up, and asks [`Tuning::stale`] whether the choice still stands: it does not once this crate's
//! version changed (a new version may score differently), once a setting the choice rests on changed (an attribute's
//! domain, its dependencies, its votes …), once what it was asked with changed (the target, the fewest answers, which
//! settlements count, which batches are left out, the rule — [`choice_fingerprint`]), or once the settled records grew or
//! shrank by a tenth since the choice — the rule the first consuming application chose by. A tuning chosen on records
//! kept elsewhere cannot be chosen again here: [`Tuning::stale_carried`] asks only whether the score scale ([`SCALE`])
//! and the settings the choice rests on still hold.

use std::collections::BTreeMap;

use crate::decider::Source;
use crate::replay::{Choice, LayerThresholds, ThresholdRule, fnv1a};
use crate::schema::{AttributeRole, KeyCombine, Schema};

/// This crate's version — what a tuning records it was chosen with.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The score scale of this crate: what a score means and the range it lives on. A release raises it only when it changes
/// how a score is computed or the range scores live in — never for a release that leaves every score as it was — so a
/// threshold chosen under one scale holds under every release of the same scale. `tests/scale.rs` fails when the golden
/// scores change while this number does not.
pub const SCALE: u32 = 1;

/// How a fingerprint is written in JSON: 16 lowercase hexadecimal digits. A fingerprint is a 64-bit equality token, and a
/// JSON number above 2^53 is rounded by a reader that holds numbers as doubles — a tuning that went through one would
/// no longer match the schema it was chosen for. Reading takes the text and the number it was written as before.
#[cfg(feature = "serde")]
mod fingerprint_text {
    use std::fmt;

    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("{value:016x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        struct Fingerprint;

        impl Visitor<'_> for Fingerprint {
            type Value = u64;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a fingerprint: up to 16 hexadecimal digits, or a non-negative integer")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
                Ok(value)
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<u64, E> {
                u64::try_from(value)
                    .map_err(|_| E::invalid_value(de::Unexpected::Signed(value), &self))
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<u64, E> {
                if text.is_empty()
                    || text.len() > 16
                    || !text.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(E::invalid_value(de::Unexpected::Str(text), &self));
                }
                u64::from_str_radix(text, 16).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(Fingerprint)
    }
}

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
    /// The precision the coarse level's chosen key threshold promised for the next classes it offers.
    pub coarse_key_promise: Option<f64>,
    /// The precision the coarse level's chosen memory threshold promised for the next classes it offers.
    pub coarse_memory_promise: Option<f64>,
    /// Experimental. The value layers' thresholds for the first record of a batch
    /// ([`crate::schema::Attribute::first_of_batch`]); all `None`, with their promises, when none were chosen apart.
    pub first_key_threshold: Option<f64>,
    pub first_memory_threshold: Option<f64>,
    pub first_memory_floor: Option<f64>,
    /// Experimental. The precision the key layer's threshold for first records promised for the next first records.
    pub first_key_promise: Option<f64>,
    /// Experimental. The precision the similar record layer's threshold for first records promised for the next ones.
    pub first_memory_promise: Option<f64>,
    /// Experimental. Whether thresholds for first records were chosen apart at all.
    #[cfg_attr(feature = "serde", serde(default))]
    pub first_of_batch: bool,
}

/// The chosen settings of a schema's decided attributes and what they were chosen on.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Tuning {
    /// The crate version that chose them ([`VERSION`]).
    pub version: String,
    /// The score scale they were chosen on ([`SCALE`]); 0 when a tuning written before the scale was recorded is read.
    #[cfg_attr(feature = "serde", serde(default))]
    pub scale: u32,
    /// The fingerprint of the settings the choice rests on ([`fingerprint`]). In JSON it is 16 lowercase hexadecimal
    /// digits, a text: a number above 2^53 does not survive a JSON reader that holds numbers as doubles, and a
    /// fingerprint is only ever compared for equality. A fingerprint written as a number still reads.
    #[cfg_attr(feature = "serde", serde(with = "fingerprint_text"))]
    pub schema: u64,
    /// The fingerprint of what the choice was asked with ([`choice_fingerprint`]): the target, the fewest answers, which
    /// settlements count, which batches are left out, and the rule. 0 when a tuning written before it was recorded is
    /// read. Written as `schema` is, in JSON, as 16 hexadecimal digits.
    #[cfg_attr(feature = "serde", serde(default, with = "fingerprint_text"))]
    pub choice: u64,
    /// How many records the choice counted when they were chosen — those settled, less the ones of a batch left
    /// uncounted ([`crate::replay::Choice::uncounted_batches`]).
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
    /// A tuning carried from elsewhere ([`Tuning::stale_carried`]) was chosen on another score scale ([`SCALE`]): its
    /// thresholds are on scores that mean something else here. 0 is a tuning that did not record its scale.
    Scale { chosen: u32, now: u32 },
    /// A setting the choice rests on changed.
    Schema,
    /// What the choice was asked with changed — the target, the fewest answers, which settlements count, which batches
    /// are left out, or the rule — or the tuning did not record it (a tuning written before it was).
    Choice,
    /// The records the choice counts grew or shrank by a tenth or more since the choice.
    Settled { chosen: usize, now: usize },
    /// Experimental. Of `attribute`'s last `judged` answers whose rightness came back, only `right` were — under the
    /// target less [`crate::keeper::REALIZED_MARGIN`]. A keeper says it ([`crate::keeper::Keep::watch_realized`]); a
    /// tuning alone never does.
    Realized {
        attribute: String,
        right: usize,
        judged: usize,
    },
}

impl Tuning {
    /// The chosen settings as `schema`'s decided attributes now hold them, chosen on `settled` records as `choice` asked
    /// and `rule` chose. The promises are left unrecorded; [`Tuning::promise`] records a layer replay's.
    pub fn capture(schema: &Schema, settled: usize, choice: &Choice, rule: ThresholdRule) -> Self {
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
                coarse_key_promise: None,
                coarse_memory_promise: None,
                first_key_threshold: a.first_of_batch.as_ref().and_then(|f| f.key_threshold),
                first_memory_threshold: a.first_of_batch.as_ref().and_then(|f| f.memory_threshold),
                first_memory_floor: a.first_of_batch.as_ref().and_then(|f| f.memory_floor),
                first_key_promise: None,
                first_memory_promise: None,
                first_of_batch: a.first_of_batch.is_some(),
            })
            .collect();
        Self {
            version: VERSION.to_owned(),
            scale: SCALE,
            schema: fingerprint(schema),
            choice: choice_fingerprint(choice, rule),
            settled,
            target_precision: choice.target_precision,
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
    /// an attribute the tuning does not name, or a promise it did not record ([`Tuning::promise`]). It reads the value's
    /// layers only: a coarse class offered instead ([`crate::decider::Answer::coarse`]) names the layer that backs the
    /// class, and its promise is [`Tuning::coarse_promise_of`]'s.
    pub fn promise_of(&self, attribute: &str, source: Source) -> Option<f64> {
        let tuned = self.attributes.iter().find(|a| a.attribute == attribute)?;
        match source {
            Source::KeyMemory => tuned.key_promise,
            Source::SimilarRecord => tuned.memory_promise,
            Source::None | Source::External => None,
        }
    }

    /// Experimental. Records the promises of `attribute`'s replay of first records of batches
    /// ([`crate::keeper::Keep::first_of_batch`]): what its chosen key and memory thresholds promised for the next first
    /// records.
    ///
    /// # Panics
    /// When the tuning has no such attribute.
    pub fn promise_first_of_batch(&mut self, attribute: &str, layers: &LayerThresholds) {
        let tuned = self
            .attributes
            .iter_mut()
            .find(|a| a.attribute == attribute)
            .unwrap_or_else(|| panic!("the tuning has no attribute '{attribute}'"));
        tuned.first_key_promise = layers.key.chosen.as_ref().map(|c| c.promise);
        tuned.first_memory_promise = layers.memory.chosen.as_ref().map(|c| c.promise);
    }

    /// Experimental. What the layer an answer came from promised for `attribute` when the record was answered as the
    /// first of its batch ([`crate::open_record::OpenRecord::is_first_of_batch`]); `None` as for [`Tuning::promise_of`],
    /// and when no thresholds for first records were chosen apart — the answer's promise is then
    /// [`Tuning::promise_of`]'s.
    pub fn first_of_batch_promise_of(&self, attribute: &str, source: Source) -> Option<f64> {
        let tuned = self.attributes.iter().find(|a| a.attribute == attribute)?;
        match source {
            Source::KeyMemory => tuned.first_key_promise,
            Source::SimilarRecord => tuned.first_memory_promise,
            Source::None | Source::External => None,
        }
    }

    /// Records the promises of `attribute`'s coarse replay ([`crate::replay::select_coarse`]): what its coarse level's
    /// chosen key and memory thresholds promised.
    ///
    /// # Panics
    /// When the tuning has no such attribute.
    pub fn promise_coarse(&mut self, attribute: &str, layers: &LayerThresholds) {
        let tuned = self
            .attributes
            .iter_mut()
            .find(|a| a.attribute == attribute)
            .unwrap_or_else(|| panic!("the tuning has no attribute '{attribute}'"));
        tuned.coarse_key_promise = layers.key.chosen.as_ref().map(|c| c.promise);
        tuned.coarse_memory_promise = layers.memory.chosen.as_ref().map(|c| c.promise);
    }

    /// What the coarse level promised for a class it offered for `attribute`, by the layer that backs the class (the
    /// coarse candidate's source): the level's key promise for the keys' class, its memory promise for the similar
    /// records'; `None` as for [`Tuning::promise_of`] ([`Tuning::promise_coarse`] records them). A class offered as a
    /// guess — not trusted — was not promised anything.
    pub fn coarse_promise_of(&self, attribute: &str, source: Source) -> Option<f64> {
        let tuned = self.attributes.iter().find(|a| a.attribute == attribute)?;
        match source {
            Source::KeyMemory => tuned.coarse_key_promise,
            Source::SimilarRecord => tuned.coarse_memory_promise,
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
            a.first_of_batch = tuned.first_of_batch.then_some(crate::schema::FirstOfBatch {
                key_threshold: tuned.first_key_threshold,
                memory_threshold: tuned.first_memory_threshold,
                memory_floor: tuned.first_memory_floor,
            });
        }
        Schema::new(schema.name(), attributes)
    }

    /// Why the tuning no longer stands for `schema` with `settled` records, or `None` while it does: another crate
    /// version, a changed setting the choice rests on, what the choice would be asked with now (`choice` and `rule`)
    /// differing from what it was asked with, or a change in the settled records of a tenth or more — at least one
    /// record while fewer than ten were settled at the choice. They are told in that order.
    ///
    /// The settings on the schema are compared (its fingerprint), and so are the settings the choice is asked with
    /// ([`choice_fingerprint`]): the target precision, the fewest answers, which settlements count, which batches are
    /// left out, and the rule. A host that keeps a tuning across runs gives the ones it would choose with now; a
    /// [`crate::keeper::Keeper`] holds them itself. A tuning written before they were recorded does not stand.
    pub fn stale(
        &self,
        schema: &Schema,
        settled: usize,
        choice: &Choice,
        rule: ThresholdRule,
    ) -> Option<Stale> {
        if self.version != VERSION {
            return Some(Stale::Version {
                chosen: self.version.clone(),
            });
        }

        if self.schema != fingerprint(schema) {
            return Some(Stale::Schema);
        }

        if self.choice != choice_fingerprint(choice, rule) {
            return Some(Stale::Choice);
        }

        (settled.abs_diff(self.settled) * 10 >= self.settled.max(10)).then_some(Stale::Settled {
            chosen: self.settled,
            now: settled,
        })
    }

    /// Why a tuning **carried from elsewhere** no longer stands for `schema`, or `None` while it does: another score
    /// scale ([`SCALE`]) or a changed setting the choice rests on (the fingerprint).
    ///
    /// A tuning chosen on records kept in another store — a sample shipped with a form — cannot be chosen again where it
    /// is used: the records are not there. So it is not stale for being chosen by another version of this crate
    /// ([`Tuning::stale`] says so for a host's own tuning, which it can choose again), nor for the count of records
    /// here differing from the count it was chosen on: a threshold stays good while the scores it is a threshold on
    /// mean the same, and the scale says when they stop. Nor is it stale for what it was asked with ([`Tuning::choice`]):
    /// that is the sender's setting for records the receiver does not have, and nothing here could ask again.
    pub fn stale_carried(&self, schema: &Schema) -> Option<Stale> {
        if self.scale != SCALE {
            return Some(Stale::Scale {
                chosen: self.scale,
                now: SCALE,
            });
        }

        (self.schema != fingerprint(schema)).then_some(Stale::Schema)
    }
}

/// A stable fingerprint of what a choice is asked with: the target precision (its bits), the fewest answers, which
/// settlements count, which batches are left out, and the rule. The kinds counted and the batches left out are sets —
/// their order and repeats say nothing — and counting every settlement (`None`) is not naming every kind: the kinds a
/// later version adds would count. A batch is a text of the host's own, so each is written with its length. The same on
/// every platform and in every run.
pub fn choice_fingerprint(choice: &Choice, rule: ThresholdRule) -> u64 {
    use crate::schema::SettlementKind as Kind;

    let mut text = String::new();
    text.push_str(&choice.target_precision.to_bits().to_string());
    text.push('\u{1e}');
    text.push_str(&choice.minimum_answered.to_string());
    text.push('\u{1e}');
    match choice.counting {
        None => text.push('-'),
        Some(kinds) => {
            let mut names = kinds
                .iter()
                .map(|kind| match kind {
                    Kind::Accept => "accept",
                    Kind::Correct => "correct",
                    Kind::Reject => "reject",
                    Kind::Revert => "revert",
                    Kind::Restore => "restore",
                    Kind::Set => "set",
                    Kind::Enter => "enter",
                    Kind::Fill => "fill",
                    Kind::Check => "check",
                    Kind::AcceptOther => "accept_other",
                    Kind::CorrectOther => "correct_other",
                })
                .collect::<Vec<_>>();
            names.sort_unstable();
            names.dedup();
            text.push('+');
            text.push_str(&names.join("\u{1f}"));
        }
    }
    text.push('\u{1e}');
    let mut batches = choice.uncounted_batches.iter().collect::<Vec<_>>();
    batches.sort_unstable();
    batches.dedup();
    for batch in batches {
        text.push_str(&batch.len().to_string());
        text.push(':');
        text.push_str(batch);
    }
    text.push('\u{1e}');
    text.push_str(match rule {
        ThresholdRule::EveryBand => "every_band",
        ThresholdRule::Pooled => "pooled",
        ThresholdRule::LeastCost => "least_cost",
    });

    fnv1a(&text)
}

/// A stable fingerprint of what a schema's choice rests on: its name and, for each attribute in order, everything but
/// the settings the replays choose (key attributes, key concentrations, thresholds, floors) and the policy — role,
/// several values, evidence, dependencies, domain, values to catch, votes, how the keys' strengths combine, coarse
/// level. The same on every platform and in every run.
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
        if a.similar_elements {
            text.push_str("elements\u{1e}");
        }
        // How the keys' strengths combine changes every key score. Written only when it is not the default, so a tuning
        // chosen before the setting existed keeps standing.
        if a.key_combine != KeyCombine::Sum {
            text.push_str("key-max\u{1e}");
        }
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
