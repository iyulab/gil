//! The schema resolver: answers each open decided attribute of a record from the settled key memory and the similar
//! record memory, and keeps both up to date as records are settled.
//!
//! Where neither memory has evidence, the answer says so and carries what an external judgement would read
//! ([`Answer::needs_external`]); the host asks its model, or does not, and hands the ranked result back
//! ([`Decider::offer_external`]) — the core calls nothing outside itself. Not yet carried over from the
//! reference: telemetry.

use std::collections::{BTreeMap, HashMap};

use crate::coarse_groups;
use crate::decision_record::{Answered, CoarseLook, DecisionRecord, KeyLook, SimilarLook};
use crate::key_memory::{self, Key, KeyMemory, Lookup};
use crate::lexical_memory::{LexicalMemory, Match};
use crate::schema::{Attribute, AttributePolicy, AttributeRole, CoarseLevel, Record, Schema};
use crate::{similar_vote, text};

/// Which layer offered a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Source {
    None,
    SettledFieldMemory,
    SimilarDocument,
    /// A judgement the host made outside the core (a model) and offered back — named `Model`, as the reference names it.
    #[cfg_attr(feature = "serde", serde(rename = "Model"))]
    External,
}

impl Source {
    /// The name the reference gives the layer.
    pub fn name(self) -> &'static str {
        match self {
            Source::None => "None",
            Source::SettledFieldMemory => "SettledFieldMemory",
            Source::SimilarDocument => "SimilarDocument",
            Source::External => "Model",
        }
    }
}

/// What a candidate rests on, for a host to show: the key backing it, or the similar settled record it came from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Evidence {
    /// Another attribute's value (as the key memory keeps it) under which the value was settled.
    Key(Key),
    /// The settled record whose value it is — for a vote, its nearest voter — and how similar that record is.
    Record { id: String, similarity: f64 },
}

impl std::fmt::Display for Evidence {
    /// `attribute: value` for a key, the record id for a record — the text a host shows.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Evidence::Key(key) => key.fmt(f),
            Evidence::Record { id, .. } => f.write_str(id),
        }
    }
}

/// A value offered for an attribute: its score on its layer's scale, the layer, what backs it — none for the attribute's
/// overall frequency and an external judgement — and whether the layer trusts it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Candidate {
    pub value: String,
    pub score: f64,
    pub source: Source,
    pub evidence: Option<Evidence>,
    pub trusted: bool,
}

/// One attribute's suggestion: its candidates, best first, the layer of the first, and — when no layer answered the value
/// and the attribute has a coarse level — the class offered instead.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Answer {
    pub attribute: String,
    pub candidates: Vec<Candidate>,
    pub source: Source,
    pub coarse: Option<Candidate>,
    /// How the attribute's answers are offered — a host fills on its own only what is not
    /// [`AttributePolicy::ConfirmRequired`].
    pub policy: AttributePolicy,
    /// Set when neither memory had evidence — no keyed value and no trusted similar record — and nothing is typed: the
    /// (attribute, value) pairs an external judgement would read, in the order they arrived (the schema's order without a
    /// history). The host may ask its model and offer the result with [`Decider::offer_external`]; ignoring it leaves the
    /// answer as it is.
    pub needs_external: Option<Vec<(String, String)>>,
}

/// A value an external judgement ranked, offered back to the core: its probability on the judgement's own scale, and
/// whether the judgement trusts it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternalCandidate {
    pub value: String,
    pub probability: f64,
    pub trusted: bool,
}

impl Answer {
    /// Whether the first candidate is trusted.
    pub fn answered(&self) -> bool {
        self.candidates.first().is_some_and(|c| c.trusted)
    }
}

/// A record's settled value for one attribute, the evidence it was settled on and when.
#[derive(Clone, Debug)]
struct CaseEntry {
    record: String,
    evidence: String,
    value: String,
    settled_at: i64,
}

/// One attribute's records grouped into cases — evidence that reads the same after normalization — each represented in
/// the record memory by its latest settlement.
#[derive(Default)]
struct Cases {
    case_of: HashMap<String, String>,
    members: HashMap<String, HashMap<String, CaseEntry>>,
}

impl Cases {
    /// Replaces the record's entry (`None`: it has no value for the attribute) and returns the record memory writes that
    /// keep every affected case represented by its latest settlement: the records to forget, then those to remember.
    fn put(&mut self, record: &str, entry: Option<CaseEntry>) -> (Vec<String>, Vec<CaseEntry>) {
        let mut affected: Vec<String> = Vec::new();
        let previous = self.case_of.remove(record);
        if let Some(previous) = &previous {
            affected.push(previous.clone());
        }

        let current = entry.as_ref().map(|e| text::normalize(&e.evidence));
        if let Some(current) = &current
            && previous.as_ref() != Some(current)
        {
            affected.push(current.clone());
        }

        let before: Vec<Option<CaseEntry>> = affected.iter().map(|c| self.latest(c)).collect();
        if let Some(previous) = &previous {
            let members = self.members.get_mut(previous).expect("the document's case");
            members.remove(record);
            if members.is_empty() {
                self.members.remove(previous);
            }
        }

        if let (Some(entry), Some(current)) = (entry, current) {
            self.case_of.insert(record.to_owned(), current.clone());
            self.members
                .entry(current)
                .or_default()
                .insert(record.to_owned(), entry);
        }

        let (mut forget, mut remember) = (Vec::new(), Vec::new());
        let mut represents = false;
        for (case, was) in affected.iter().zip(before) {
            let now = self.latest(case);
            let now_id = now.as_ref().map(|n| n.record.as_str());
            represents |= now_id == Some(record);
            if let Some(was) = &was
                && was.record != record
                && Some(was.record.as_str()) != now_id
            {
                forget.push(was.record.clone());
            }

            // The record itself is written again whenever it represents its case: its value or evidence may have
            // changed.
            if let Some(now) = now
                && (Some(now.record.as_str()) != was.as_ref().map(|w| w.record.as_str())
                    || now.record == record)
            {
                remember.push(now);
            }
        }

        if !represents {
            forget.push(record.to_owned());
        }

        (forget, remember)
    }

    /// The case's latest settlement: the later time, then the larger record id.
    fn latest(&self, case: &str) -> Option<CaseEntry> {
        self.members
            .get(case)?
            .values()
            .reduce(|latest, e| {
                let later =
                    (e.settled_at, e.record.as_str()) > (latest.settled_at, latest.record.as_str());
                if later { e } else { latest }
            })
            .cloned()
    }
}

/// The schema resolver over a settled key memory and, optionally, a lexical similar record memory. Not thread-safe.
pub struct Decider {
    key_memory: KeyMemory,
    record_memory: Option<LexicalMemory>,
    cases: HashMap<String, Cases>,
    candidate_count: usize,
    similar_document_count: usize,
}

impl Decider {
    /// A resolver offering up to three candidates per attribute and reporting no similar records of its own.
    pub fn new(key_memory: KeyMemory, record_memory: Option<LexicalMemory>) -> Self {
        Self {
            key_memory,
            record_memory,
            cases: HashMap::new(),
            candidate_count: 3,
            similar_document_count: 0,
        }
    }

    /// The task an attribute's record memory is kept under.
    pub fn task(schema: &Schema, attribute: &str) -> String {
        format!("{}/{attribute}", schema.name())
    }

    /// Puts settled records into memory, each replacing what it contributed before.
    pub fn rebuild(&mut self, schema: &Schema, records: &[Record]) {
        for record in records {
            self.put(schema, record);
        }
    }

    /// Takes the record out of both memories — a record the host deleted. The key memory is then as if the record had
    /// never been put; in the similar record memory what it represented falls to the latest other record of its case,
    /// and it is never offered again. The similarities themselves can differ slightly from memories rebuilt without it,
    /// as they can after any change: the lexical memory takes its n-gram frequencies afresh only once its row count has
    /// moved by a tenth.
    pub fn remove(&mut self, schema: &Schema, record: &str) {
        self.key_memory.remove(schema.name(), record);
        let Some(memory) = self.record_memory.as_mut() else {
            return;
        };

        for attribute in schema
            .attributes()
            .iter()
            .filter(|f| f.role == AttributeRole::Decided)
        {
            let task = Self::task(schema, &attribute.name);
            let Some(cases) = self.cases.get_mut(&task) else {
                continue;
            };
            let (forget, remember) = cases.put(record, None);
            for id in forget {
                memory.forget(&task, &id);
            }

            for latest in remember {
                memory.remember(&task, &latest.record, &latest.evidence, &latest.value);
            }
        }
    }

    /// Replaces the record's contribution to both memories with what its current values imply — leaving out values
    /// the application filled on its own.
    fn put(&mut self, schema: &Schema, record: &Record) {
        let record = record.remembered();
        self.key_memory.put(schema, &record);
        let Some(memory) = self.record_memory.as_mut() else {
            return;
        };

        for attribute in schema
            .attributes()
            .iter()
            .filter(|f| f.role == AttributeRole::Decided)
        {
            let task = Self::task(schema, &attribute.name);
            let entry = record.values.get(&attribute.name).map(|value| CaseEntry {
                record: record.id.clone(),
                evidence: schema.evidence(&attribute.name, &record.values, &record.sets),
                value: value.clone(),
                settled_at: record.settled_at,
            });
            let (forget, remember) = self
                .cases
                .entry(task.clone())
                .or_default()
                .put(&record.id, entry);
            for id in forget {
                memory.forget(&task, &id);
            }

            for latest in remember {
                memory.remember(&task, &latest.record, &latest.evidence, &latest.value);
            }
        }
    }

    /// Answers every open decided attribute — without a value, unless it takes several, and not off — from the values given, writing
    /// nothing to memory. The saved version of `record`, if memory holds one, is not evidence for itself: its
    /// settlements are left out of the key memory, and a similar record lookup that finds it passes it over.
    ///
    /// # Panics
    /// When `values` and `sets` keep an attribute where its kind does not belong ([`Schema::check_placement`]).
    pub fn answer(
        &self,
        schema: &Schema,
        record: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> Vec<Answer> {
        self.answer_with(schema, record, values, sets, &mut |_| {})
    }

    /// As [`Decider::answer`], handing `sink` the decision record of each attribute's lookup, in the answers' order — what
    /// the key layer and the similar record layer found whatever the thresholds, and what was answered — for a host that
    /// keeps them to recount its thresholds later.
    ///
    /// # Panics
    /// As [`Decider::answer`].
    pub fn answer_with(
        &self,
        schema: &Schema,
        record: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Vec<Answer> {
        if let Err(misplaced) = schema.check_placement(values, sets) {
            panic!("{misplaced}");
        }
        schema
            .attributes()
            .iter()
            .filter(|f| {
                f.role == AttributeRole::Decided
                    && f.policy != AttributePolicy::Off
                    && (f.multiple || !values.contains_key(&f.name))
            })
            .map(|f| {
                let q = Lookup::new(schema, &f.name, values)
                    .sets(sets)
                    .excluding(Some(record));
                self.answer_attribute(&q, f, None, sink)
            })
            .collect()
    }

    /// Answers one open decided attribute — without a value, unless it takes several — as [`Decider::answer`] would
    /// among the rest. `typed` is the text typed into the attribute so far: the suggestion then offers only values that
    /// begin with it (ignoring case), only the key layer answers, held to the threshold for that many characters, a
    /// similar record's value that fits is offered as a guess, and no coarse class is offered.
    ///
    /// # Panics
    /// When the attribute is not decided, is off, or has a value and takes one; or when `values` and `sets` keep an
    /// attribute where its kind does not belong ([`Schema::check_placement`]).
    pub fn answer_one(
        &self,
        schema: &Schema,
        record: &str,
        attribute: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
        typed: Option<&str>,
    ) -> Answer {
        let q = Lookup::new(schema, attribute, values)
            .sets(sets)
            .typed(typed)
            .excluding(Some(record));
        self.answer_one_with(&q, &mut |_| {})
    }

    /// As [`Decider::answer_one`] for the lookup `q` — its schema, attribute, known values and sets, typed text and the
    /// record asked about (the one it excludes) — handing `sink` the decision record of the lookup: what the key layer
    /// and the similar record layer found whatever the thresholds, what the attribute's coarse level made of it, and what
    /// was answered. Whether the answer was right is known only when the record is settled; the record leaves it open.
    ///
    /// # Panics
    /// As [`Decider::answer_one`].
    pub fn answer_one_with(&self, q: &Lookup, sink: &mut dyn FnMut(&DecisionRecord)) -> Answer {
        let definition = Self::open_attribute(q);
        self.answer_attribute(q, definition, None, sink)
    }

    /// The answer for the lookup `q` with an external judgement's ranked values in it — what the host's model made of the
    /// evidence an earlier answer asked for ([`Answer::needs_external`]). The values join the candidates in the same order
    /// a model's always took: after values a memory trusts, before the guesses; values outside the attribute's domain, and
    /// values of a set already chosen, are left out. A judgement offered where a memory had evidence changes nothing.
    ///
    /// # Panics
    /// As [`Decider::answer_one`].
    pub fn offer_external(&self, q: &Lookup, ranked: &[ExternalCandidate]) -> Answer {
        self.offer_external_with(q, ranked, &mut |_| {})
    }

    /// As [`Decider::offer_external`], handing `sink` the lookup's decision record.
    ///
    /// # Panics
    /// As [`Decider::answer_one`].
    pub fn offer_external_with(
        &self,
        q: &Lookup,
        ranked: &[ExternalCandidate],
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Answer {
        let definition = Self::open_attribute(q);
        self.answer_attribute(q, definition, Some(ranked), sink)
    }

    /// The looked-up attribute's definition, checked to be open: decided, not off, and without a value unless it takes
    /// several.
    fn open_attribute<'a>(q: &Lookup<'a>) -> &'a Attribute {
        let (attribute, definition) = (q.attribute, q.schema.attribute(q.attribute));
        if let Err(misplaced) = q.schema.check_placement(q.known, q.known_sets) {
            panic!("{misplaced}");
        }
        assert!(
            definition.role == AttributeRole::Decided,
            "'{attribute}' is not a decided attribute"
        );
        assert!(
            definition.policy != AttributePolicy::Off,
            "'{attribute}' is off and is not answered"
        );
        assert!(
            definition.multiple || !q.known.contains_key(attribute),
            "'{attribute}' already has a value"
        );
        definition
    }

    /// One attribute's suggestion. Layers that answer come first — values under a key strong enough, a record similar
    /// enough — then guesses: values under weaker keys, the attribute's most frequent value, the nearest record below the
    /// threshold, the attribute's other values.
    fn answer_attribute(
        &self,
        q: &Lookup,
        attribute: &Attribute,
        external: Option<&[ExternalCandidate]>,
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Answer {
        let q = *q;
        let typed = q.typed;
        let remembered: Vec<Candidate> = self
            .key_memory
            .rank(&q, self.candidate_count)
            .into_iter()
            .map(|c| Candidate {
                value: c.value,
                score: c.score,
                source: Source::SettledFieldMemory,
                evidence: c.evidence.map(Evidence::Key),
                trusted: c.trusted,
            })
            .collect();
        let (keyed, frequent): (Vec<&Candidate>, Vec<&Candidate>) =
            remembered.iter().partition(|c| c.evidence.is_some());
        // A value the attribute must not miss was settled under the record's keys: no layer answers it.
        let caught = self.key_memory.catches(&q);
        let (similar, admitted, vote) = self.similar(&q, attribute, caught);

        // An external judgement only where neither memory had evidence: a value backed by what the record says — even by a
        // key too weak to answer — beats a model's guess. Not while a person is typing: they ask again with every pause.
        // An attribute with a value to catch has a key's values, so it never asks.
        let wants_external =
            typed.is_none() && keyed.is_empty() && !similar.iter().any(|c| c.trusted);
        let chosen: Vec<&String> = if attribute.multiple {
            q.known_sets
                .get(&attribute.name)
                .into_iter()
                .flatten()
                .collect()
        } else {
            Vec::new()
        };
        let offered: Vec<Candidate> = external
            .filter(|_| wants_external)
            .into_iter()
            .flatten()
            .filter(|e| {
                attribute.admits_in(&e.value, q.known, q.known_sets) && !chosen.contains(&&e.value)
            })
            .map(|e| Candidate {
                value: e.value.clone(),
                score: e.probability,
                source: Source::External,
                evidence: None,
                trusted: e.trusted,
            })
            .collect();
        let needs_external = (wants_external && external.is_none()).then(|| {
            let mut pairs = q
                .schema
                .evidence_values(&attribute.name, q.known, q.known_sets);
            if let Some(arrival) = q.arrival {
                pairs.sort_by_key(|(name, _)| {
                    arrival.iter().position(|a| a == name).unwrap_or(usize::MAX)
                });
            }
            pairs
        });

        let mut candidates: Vec<Candidate> = Vec::new();
        let ordered = keyed
            .iter()
            .copied()
            .filter(|c| c.trusted)
            .chain(similar.iter().filter(|c| c.trusted))
            .chain(offered.iter())
            .chain(keyed.iter().copied().filter(|c| !c.trusted))
            .chain(frequent.iter().copied().take(1))
            .chain(similar.iter().filter(|c| !c.trusted))
            .chain(frequent.iter().copied().skip(1));
        for candidate in ordered {
            if candidates.len() == self.candidate_count {
                break;
            }

            if !candidates.iter().any(|c| c.value == candidate.value) {
                candidates.push(candidate.clone());
            }
        }

        let source = candidates.first().map_or(Source::None, |c| c.source);
        let answered = candidates.first().is_some_and(|c| c.trusted);
        // What the attribute's coarse level makes of the lookup, whatever its thresholds — the class the keys back and the class
        // the similar records vote for. Not while a person is typing: a class is no answer to typed text.
        let classes = attribute
            .coarse
            .as_ref()
            .filter(|_| typed.is_none())
            .map(|level| {
                (
                    level,
                    coarse_groups::keys(&self.key_memory.keyed_values(&q), level),
                    coarse_groups::vote(&admitted, attribute.similar_votes, level),
                )
            });
        // No layer answered the value: offer its class, as the same two layers back it. Not where no layer may answer: a
        // class can miss the value to catch as a value can.
        let coarse = match &classes {
            Some((level, keys, vote)) if !answered && !caught => {
                coarse(level, keys.clone(), vote.clone())
            }
            _ => None,
        };

        sink(&DecisionRecord {
            record: q.excluding.unwrap_or_default().to_owned(),
            attribute: attribute.name.clone(),
            order: None,
            weight: 1.0,
            key: keyed.first().map(|best| KeyLook {
                strength: best.score,
                value: best.value.clone(),
                correct: None,
                key: best
                    .evidence
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                caught,
            }),
            similar: vote,
            coarse: classes.map(|(_, keys, vote)| CoarseLook {
                value_answered: answered,
                keys: keys.map(|(class, strength, key)| KeyLook {
                    strength,
                    value: class,
                    correct: None,
                    key: key.to_string(),
                    caught,
                }),
                vote: vote.map(|(class, score, nearest, voted)| SimilarLook {
                    score,
                    value: class,
                    record: nearest.source,
                    correct: None,
                    nearest: nearest.similarity,
                    voted,
                }),
            }),
            typed: None,
            answered: candidates.first().filter(|c| c.trusted).map(|c| Answered {
                layer: c.source,
                value: c.value.clone(),
                threshold: match c.source {
                    Source::SettledFieldMemory if typed.is_none() => attribute.key_threshold,
                    Source::SimilarDocument => attribute.memory_threshold,
                    _ => None,
                },
            }),
            settled: None,
            settlement: None,
            probability: None,
        });

        Answer {
            attribute: attribute.name.clone(),
            candidates,
            source,
            coarse,
            policy: attribute.policy,
            needs_external,
        }
    }

    /// The value the most similar settled records vote for — trusted when its score reaches the attribute's memory
    /// threshold and, with more than one voter, its nearest voter the floor — and the records found other than the
    /// record itself (the one the lookup excludes): one more is asked for, and it is passed over. Nothing for an attribute
    /// that takes several values. With typed text only records whose value begins with it vote, and the vote is never
    /// trusted; nor is it where a value to catch was settled under the record's keys. `attribute` is the looked-up
    /// attribute's definition.
    fn similar(
        &self,
        q: &Lookup,
        attribute: &Attribute,
        caught: bool,
    ) -> (Vec<Candidate>, Vec<Match>, Option<SimilarLook>) {
        let (schema, typed) = (q.schema, q.typed);
        let evidence = schema.evidence(&attribute.name, q.known, q.known_sets);
        let Some(memory) = &self.record_memory else {
            return (Vec::new(), Vec::new(), None);
        };
        if evidence.is_empty() || attribute.multiple {
            return (Vec::new(), Vec::new(), None);
        }

        let asked = self.similar_document_count.max(attribute.similar_votes) + 1;
        let admitted: Vec<Match> = memory
            .nearest(&Self::task(schema, &attribute.name), &evidence, asked)
            .into_iter()
            .filter(|m| {
                q.excluding != Some(m.source.as_str())
                    && attribute.admits_in(&m.answer, q.known, q.known_sets)
                    && key_memory::begins(&m.answer, typed)
            })
            .collect();
        let Some((winner, score)) = similar_vote::decide(&admitted, attribute.similar_votes) else {
            return (Vec::new(), admitted, None);
        };

        let voted = similar_vote::voters(&admitted, attribute.similar_votes) > 1;
        let floored = attribute
            .memory_floor
            .is_some_and(|floor| voted && winner.similarity < floor);
        let hit = typed.is_none()
            && !caught
            && attribute.memory_threshold.is_some_and(|t| score >= t)
            && !floored;
        let candidate = Candidate {
            value: winner.answer.clone(),
            score,
            source: Source::SimilarDocument,
            evidence: Some(Evidence::Record {
                id: winner.source.clone(),
                similarity: winner.similarity,
            }),
            trusted: hit,
        };
        let look = SimilarLook {
            score,
            value: winner.answer.clone(),
            record: winner.source.clone(),
            correct: None,
            nearest: winner.similarity,
            voted,
        };
        (vec![candidate], admitted, Some(look))
    }
}

/// The coarse candidate for a suggestion no layer answered: the keys' class when it reaches the level's key threshold,
/// else the vote's when it reaches the memory threshold and its nearest voter the floor, else — as a guess — the keys'
/// class, or the vote's when the keys back none.
fn coarse(
    level: &CoarseLevel,
    keys: Option<(String, f64, Key)>,
    vote: Option<(String, f64, Match, bool)>,
) -> Option<Candidate> {
    let from_keys = |(class, score, key): (String, f64, Key), trusted| Candidate {
        value: class,
        score,
        source: Source::SettledFieldMemory,
        evidence: Some(Evidence::Key(key)),
        trusted,
    };
    let from_vote = |(class, score, nearest, _): (String, f64, Match, bool), trusted| Candidate {
        value: class,
        score,
        source: Source::SimilarDocument,
        evidence: Some(Evidence::Record {
            id: nearest.source,
            similarity: nearest.similarity,
        }),
        trusted,
    };
    if let Some(k) = &keys
        && level.key_threshold.is_some_and(|t| k.1 >= t)
    {
        return keys.map(|k| from_keys(k, true));
    }

    if let Some(v) = &vote {
        let trusted = level.memory_threshold.is_some_and(|t| v.1 >= t)
            && !(v.3
                && level
                    .memory_floor
                    .is_some_and(|floor| v.2.similarity < floor));
        if trusted {
            return vote.map(|v| from_vote(v, true));
        }
    }

    match keys {
        Some(k) => Some(from_keys(k, false)),
        None => vote.map(|v| from_vote(v, false)),
    }
}
