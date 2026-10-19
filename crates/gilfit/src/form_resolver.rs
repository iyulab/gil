//! The form resolver: suggests each open judged field of a document from the settled field memory and the similar
//! document memory, and keeps both up to date as documents are settled.
//!
//! Not yet carried over from the reference: a model where neither memory had evidence, a domain taken from another
//! field's values, and telemetry.

use std::collections::{BTreeMap, HashMap};

use crate::coarse_groups;
use crate::decision_record::{Answered, DecisionRecord, KeyLook, SimilarLook};
use crate::field_memory::{self, FieldMemory, Lookup};
use crate::form::{CoarseLevel, Document, Field, Form, Role};
use crate::lexical_memory::{LexicalMemory, Match};
use crate::{similar_vote, text};

/// Which layer offered a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    None,
    SettledFieldMemory,
    SimilarDocument,
}

impl Source {
    /// The name the reference gives the layer.
    pub fn name(self) -> &'static str {
        match self {
            Source::None => "None",
            Source::SettledFieldMemory => "SettledFieldMemory",
            Source::SimilarDocument => "SimilarDocument",
        }
    }
}

/// A value offered for a field: its score on its layer's scale, the layer, what backs it (the key `field: value`, or
/// the similar document's id), and whether the layer trusts it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub value: String,
    pub score: f64,
    pub source: Source,
    pub evidence: Option<String>,
    pub trusted: bool,
}

/// One field's suggestion: its candidates, best first, the layer of the first, and — when no layer answered the value
/// and the field has a coarse level — the class offered instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub field: String,
    pub candidates: Vec<Candidate>,
    pub source: Source,
    pub coarse: Option<Candidate>,
}

impl Suggestion {
    /// Whether the first candidate is trusted.
    pub fn answered(&self) -> bool {
        self.candidates.first().is_some_and(|c| c.trusted)
    }
}

/// A document's settled value for one field, the evidence it was settled on and when.
#[derive(Clone, Debug)]
struct CaseEntry {
    document: String,
    evidence: String,
    value: String,
    settled_at: i64,
}

/// One field's documents grouped into cases — evidence that reads the same after normalization — each represented in
/// the document memory by its latest settlement.
#[derive(Default)]
struct Cases {
    case_of: HashMap<String, String>,
    members: HashMap<String, HashMap<String, CaseEntry>>,
}

impl Cases {
    /// Replaces the document's entry (`None`: it has no value for the field) and returns the document memory writes that
    /// keep every affected case represented by its latest settlement: the documents to forget, then those to remember.
    fn put(&mut self, document: &str, entry: Option<CaseEntry>) -> (Vec<String>, Vec<CaseEntry>) {
        let mut affected: Vec<String> = Vec::new();
        let previous = self.case_of.remove(document);
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
            members.remove(document);
            if members.is_empty() {
                self.members.remove(previous);
            }
        }

        if let (Some(entry), Some(current)) = (entry, current) {
            self.case_of.insert(document.to_owned(), current.clone());
            self.members
                .entry(current)
                .or_default()
                .insert(document.to_owned(), entry);
        }

        let (mut forget, mut remember) = (Vec::new(), Vec::new());
        let mut represents = false;
        for (case, was) in affected.iter().zip(before) {
            let now = self.latest(case);
            let now_id = now.as_ref().map(|n| n.document.as_str());
            represents |= now_id == Some(document);
            if let Some(was) = &was
                && was.document != document
                && Some(was.document.as_str()) != now_id
            {
                forget.push(was.document.clone());
            }

            // The document itself is written again whenever it represents its case: its value or evidence may have
            // changed.
            if let Some(now) = now
                && (Some(now.document.as_str()) != was.as_ref().map(|w| w.document.as_str())
                    || now.document == document)
            {
                remember.push(now);
            }
        }

        if !represents {
            forget.push(document.to_owned());
        }

        (forget, remember)
    }

    /// The case's latest settlement: the later time, then the larger document id.
    fn latest(&self, case: &str) -> Option<CaseEntry> {
        self.members
            .get(case)?
            .values()
            .reduce(|latest, e| {
                let later = (e.settled_at, e.document.as_str())
                    > (latest.settled_at, latest.document.as_str());
                if later { e } else { latest }
            })
            .cloned()
    }
}

/// The form resolver over a settled field memory and, optionally, a lexical similar document memory. Not thread-safe.
pub struct FormResolver {
    field_memory: FieldMemory,
    document_memory: Option<LexicalMemory>,
    cases: HashMap<String, Cases>,
    candidate_count: usize,
    similar_document_count: usize,
}

impl FormResolver {
    /// A resolver offering up to three candidates per field and reporting no similar documents of its own.
    pub fn new(field_memory: FieldMemory, document_memory: Option<LexicalMemory>) -> Self {
        Self {
            field_memory,
            document_memory,
            cases: HashMap::new(),
            candidate_count: 3,
            similar_document_count: 0,
        }
    }

    /// The task a field's document memory is kept under.
    pub fn task(form: &Form, field: &str) -> String {
        format!("{}/{field}", form.name)
    }

    /// Puts settled documents into memory, each replacing what it contributed before.
    pub fn rebuild(&mut self, form: &Form, documents: &[Document]) {
        for document in documents {
            self.put(form, document);
        }
    }

    /// Replaces the document's contribution to both memories with what its current values imply — leaving out values
    /// the application filled on its own.
    fn put(&mut self, form: &Form, document: &Document) {
        let document = document.remembered();
        self.field_memory.put(form, &document);
        let Some(memory) = self.document_memory.as_mut() else {
            return;
        };

        for field in form.fields.iter().filter(|f| f.role == Role::Judged) {
            let task = Self::task(form, &field.name);
            let entry = document.values.get(&field.name).map(|value| CaseEntry {
                document: document.id.clone(),
                evidence: form.evidence(&field.name, &document.values, &document.sets),
                value: value.clone(),
                settled_at: document.settled_at,
            });
            let (forget, remember) = self
                .cases
                .entry(task.clone())
                .or_default()
                .put(&document.id, entry);
            for id in forget {
                memory.forget(&task, &id);
            }

            for latest in remember {
                memory.remember(&task, &latest.document, &latest.evidence, &latest.value);
            }
        }
    }

    /// Suggests every open judged field — without a value, unless it takes several — from the values given, writing
    /// nothing to memory. The saved version of `document`, if memory holds one, is not evidence for itself: its
    /// settlements are left out of the field memory, and a similar document lookup that finds it passes it over.
    pub fn suggest(
        &self,
        form: &Form,
        document: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> Vec<Suggestion> {
        form.fields
            .iter()
            .filter(|f| f.role == Role::Judged && (f.multiple || !values.contains_key(&f.name)))
            .map(|f| {
                let q = Lookup::new(form, &f.name, values)
                    .sets(sets)
                    .excluding(Some(document));
                self.suggest_field(&q, f, &mut |_| {})
            })
            .collect()
    }

    /// Suggests one open judged field — without a value, unless it takes several — as [`FormResolver::suggest`] would
    /// among the rest. `typed` is the text typed into the field so far: the suggestion then offers only values that
    /// begin with it (ignoring case), only the key layer answers, held to the threshold for that many characters, a
    /// similar document's value that fits is offered as a guess, and no coarse class is offered.
    ///
    /// # Panics
    /// When the field is not judged, or has a value and takes one.
    pub fn suggest_one(
        &self,
        form: &Form,
        document: &str,
        field: &str,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
        typed: Option<&str>,
    ) -> Suggestion {
        let q = Lookup::new(form, field, values)
            .sets(sets)
            .typed(typed)
            .excluding(Some(document));
        self.suggest_one_with(&q, &mut |_| {})
    }

    /// As [`FormResolver::suggest_one`] for the lookup `q` — its form, field, known values and sets, typed text and the
    /// document asked about (the one it excludes) — handing `sink` the decision record of the lookup: what the key layer
    /// and the similar document layer found whatever the thresholds, and what was answered. Whether the answer was right
    /// is known only when the document is settled; the record leaves it open.
    ///
    /// # Panics
    /// As [`FormResolver::suggest_one`].
    pub fn suggest_one_with(
        &self,
        q: &Lookup,
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Suggestion {
        let (field, definition) = (q.field, q.form.field(q.field));
        assert!(
            definition.role == Role::Judged,
            "'{field}' is not a judged field"
        );
        assert!(
            definition.multiple || !q.known.contains_key(field),
            "'{field}' already has a value"
        );
        self.suggest_field(q, definition, sink)
    }

    /// One field's suggestion. Layers that answer come first — values under a key strong enough, a document similar
    /// enough — then guesses: values under weaker keys, the field's most frequent value, the nearest document below the
    /// threshold, the field's other values.
    fn suggest_field(
        &self,
        q: &Lookup,
        field: &Field,
        sink: &mut dyn FnMut(&DecisionRecord),
    ) -> Suggestion {
        let q = *q;
        let typed = q.typed;
        let remembered: Vec<Candidate> = self
            .field_memory
            .rank(&q, self.candidate_count)
            .into_iter()
            .map(|c| Candidate {
                value: c.value,
                score: c.score,
                source: Source::SettledFieldMemory,
                evidence: c.evidence,
                trusted: c.trusted,
            })
            .collect();
        let (keyed, frequent): (Vec<&Candidate>, Vec<&Candidate>) =
            remembered.iter().partition(|c| c.evidence.is_some());
        // A value the field must not miss was settled under the document's keys: no layer answers it.
        let caught = self.field_memory.catches(&q);
        let (similar, admitted, vote) = self.similar(&q, field, caught);

        let mut candidates: Vec<Candidate> = Vec::new();
        let ordered = keyed
            .iter()
            .copied()
            .filter(|c| c.trusted)
            .chain(similar.iter().filter(|c| c.trusted))
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
        // No layer answered the value: offer its class, as the same two layers back it. Not while a person is typing, and
        // not where no layer may answer: a class can miss the value to catch as a value can.
        let coarse = match &field.coarse {
            Some(level) if !answered && typed.is_none() && !caught => coarse(
                level,
                coarse_groups::keys(&self.field_memory.keyed_values(&q), level),
                coarse_groups::vote(&admitted, field.similar_document_votes, level),
            ),
            _ => None,
        };

        sink(&DecisionRecord {
            record: q.excluding.unwrap_or_default().to_owned(),
            field: field.name.clone(),
            order: None,
            weight: 1.0,
            key: keyed.first().map(|best| KeyLook {
                strength: best.score,
                value: best.value.clone(),
                correct: None,
                key: best.evidence.clone().unwrap_or_default(),
                caught,
            }),
            similar: vote,
            coarse: None,
            answered: candidates.first().filter(|c| c.trusted).map(|c| Answered {
                layer: c.source,
                value: c.value.clone(),
                threshold: match c.source {
                    Source::SettledFieldMemory if typed.is_none() => field.key_threshold,
                    Source::SimilarDocument => field.memory_threshold,
                    _ => None,
                },
            }),
            settled: None,
            settlement: None,
            probability: None,
        });

        Suggestion {
            field: field.name.clone(),
            candidates,
            source,
            coarse,
        }
    }

    /// The value the most similar settled documents vote for — trusted when its score reaches the field's memory
    /// threshold and, with more than one voter, its nearest voter the floor — and the documents found other than the
    /// document itself (the one the lookup excludes): one more is asked for, and it is passed over. Nothing for a field
    /// that takes several values. With typed text only documents whose value begins with it vote, and the vote is never
    /// trusted; nor is it where a value to catch was settled under the document's keys. `field` is the looked-up
    /// field's definition.
    fn similar(
        &self,
        q: &Lookup,
        field: &Field,
        caught: bool,
    ) -> (Vec<Candidate>, Vec<Match>, Option<SimilarLook>) {
        let (form, typed) = (q.form, q.typed);
        let evidence = form.evidence(&field.name, q.known, q.known_sets);
        let Some(memory) = &self.document_memory else {
            return (Vec::new(), Vec::new(), None);
        };
        if evidence.is_empty() || field.multiple {
            return (Vec::new(), Vec::new(), None);
        }

        let asked = self
            .similar_document_count
            .max(field.similar_document_votes)
            + 1;
        let admitted: Vec<Match> = memory
            .nearest(&Self::task(form, &field.name), &evidence, asked)
            .into_iter()
            .filter(|m| {
                q.excluding != Some(m.source.as_str())
                    && field.admits(&m.answer)
                    && field_memory::begins(&m.answer, typed)
            })
            .collect();
        let Some((winner, score)) = similar_vote::decide(&admitted, field.similar_document_votes)
        else {
            return (Vec::new(), admitted, None);
        };

        let voted = similar_vote::voters(&admitted, field.similar_document_votes) > 1;
        let floored = field
            .memory_similarity_floor
            .is_some_and(|floor| voted && winner.similarity < floor);
        let hit = typed.is_none()
            && !caught
            && field.memory_threshold.is_some_and(|t| score >= t)
            && !floored;
        let candidate = Candidate {
            value: winner.answer.clone(),
            score,
            source: Source::SimilarDocument,
            evidence: Some(winner.source.clone()),
            trusted: hit,
        };
        let look = SimilarLook {
            score,
            value: winner.answer.clone(),
            document: winner.source.clone(),
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
    keys: Option<(String, f64, String)>,
    vote: Option<(String, f64, Match, bool)>,
) -> Option<Candidate> {
    let from_keys = |(class, score, evidence): (String, f64, String), trusted| Candidate {
        value: class,
        score,
        source: Source::SettledFieldMemory,
        evidence: Some(evidence),
        trusted,
    };
    let from_vote = |(class, score, nearest, _): (String, f64, Match, bool), trusted| Candidate {
        value: class,
        score,
        source: Source::SimilarDocument,
        evidence: Some(nearest.source),
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
                    .memory_similarity_floor
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
