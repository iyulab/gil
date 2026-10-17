//! The settled field memory: suggests a judged field's value from how often each value was settled alongside the values
//! of the document's other fields. Each other field's value is a key; a value's score is the sum over the document's
//! keys of its strength there — its weighted count over the key's weighted total plus the key field's concentration.
//! Recent settlements weigh more: under each key, a settlement counts `recency_decay` to the power of the settlements
//! made under the same key after it.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::form::{Document, Field, Form, Role};
use crate::text;

/// Another field's normalized value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    field: String,
    value: String,
}

/// A field's counts under one key, or overall when the key is `None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Slot {
    field: String,
    key: Option<Key>,
}

/// A value a document settled, and when; the time and then the document id order settlements.
#[derive(Clone, Debug, PartialEq)]
struct Settled {
    value: String,
    at: i64,
    document: String,
}

/// One value's place under a slot: its share of the weighted total, its weight, the total and its latest settlement.
#[derive(Clone, Debug)]
struct Share {
    value: String,
    share: f64,
    weight: f64,
    total: f64,
    latest: i64,
}

/// A candidate value: its score, the key that backs it most strongly (`field: value`), and whether it is trusted.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub value: String,
    pub score: f64,
    pub evidence: Option<String>,
    pub trusted: bool,
}

/// The best-ranked value settled under the known keys ([`FieldMemory::first`]): its score, whether it is the value
/// asked about, the key that backs it most strongly (`field: value`), and whether a value the field must not miss was
/// settled under the known keys.
#[derive(Clone, Debug, PartialEq)]
pub struct First {
    pub score: f64,
    pub matches: bool,
    pub key: String,
    /// Always false here: values a field must not miss are not yet supported by this implementation.
    pub caught: bool,
}

/// The settled field memory. Not thread-safe.
pub struct FieldMemory {
    max_key_length: usize,
    recency_decay: f64,
    forms: HashMap<String, FormIndex>,
}

#[derive(Default)]
struct FormIndex {
    documents: HashMap<String, Vec<(Slot, Settled)>>,
    values: HashMap<Slot, Vec<Settled>>,
}

impl Default for FieldMemory {
    fn default() -> Self {
        Self::new(60, 0.95)
    }
}

impl FieldMemory {
    pub fn new(max_key_length: usize, recency_decay: f64) -> Self {
        assert!(max_key_length >= 1, "max_key_length must be at least 1");
        assert!(
            recency_decay > 0.0 && recency_decay <= 1.0,
            "recency_decay must be in (0, 1]"
        );
        Self {
            max_key_length,
            recency_decay,
            forms: HashMap::new(),
        }
    }

    /// How many documents with at least one settled judged field the form's memory holds.
    pub fn count(&self, form: &str) -> usize {
        self.forms
            .get(form)
            .map_or(0, |index| index.documents.len())
    }

    /// Replaces what the document contributed with what its current values imply. Fields that take several values are
    /// not yet remembered by this implementation.
    pub fn put(&mut self, form: &Form, document: &Document) {
        self.remove(&form.name, &document.id);
        let mut tallies = Vec::new();
        for field in form
            .fields
            .iter()
            .filter(|f| f.role == Role::Judged && !f.multiple)
        {
            let Some(value) = document.values.get(&field.name) else {
                continue;
            };

            let settled = Settled {
                value: value.clone(),
                at: document.settled_at,
                document: document.id.clone(),
            };
            tallies.push((
                Slot {
                    field: field.name.clone(),
                    key: None,
                },
                settled.clone(),
            ));
            for key in self.keys(form, field, &document.values, &document.sets) {
                tallies.push((
                    Slot {
                        field: field.name.clone(),
                        key: Some(key),
                    },
                    settled.clone(),
                ));
            }
        }

        if tallies.is_empty() {
            return;
        }

        let index = self.forms.entry(form.name.clone()).or_default();
        for (slot, settled) in &tallies {
            index
                .values
                .entry(slot.clone())
                .or_default()
                .push(settled.clone());
        }
        index.documents.insert(document.id.clone(), tallies);
    }

    /// Removes what the document contributed, if anything.
    pub fn remove(&mut self, form: &str, document: &str) {
        let Some(index) = self.forms.get_mut(form) else {
            return;
        };
        let Some(tallies) = index.documents.remove(document) else {
            return;
        };
        for (slot, settled) in tallies {
            if let Some(list) = index.values.get_mut(&slot) {
                if let Some(at) = list.iter().position(|s| *s == settled) {
                    list.remove(at);
                }
                if list.is_empty() {
                    index.values.remove(&slot);
                }
            }
        }
    }

    /// The `count` values most likely for `field` given the values known so far, best first: the values settled under
    /// the known keys by their score (with the key backing each most strongly), then the field's most frequently settled
    /// values by their share. The keyed values are trusted when the first one's score reaches the field's key threshold.
    pub fn rank(
        &self,
        form: &Form,
        field: &str,
        known: &BTreeMap<String, String>,
        count: usize,
    ) -> Vec<Candidate> {
        let definition = form.field(field);
        let Some(index) = self.forms.get(&form.name) else {
            return Vec::new();
        };

        let ranked = self.keyed(index, form, definition, known, &BTreeMap::new());
        let trusted = definition
            .key_threshold
            .is_some_and(|t| ranked.first().is_some_and(|(_, score, _)| *score >= t));
        let mut candidates: Vec<Candidate> = ranked
            .into_iter()
            .take(count)
            .map(|(value, score, key)| Candidate {
                value,
                score,
                evidence: Some(format!("{}: {}", key.field, key.value)),
                trusted,
            })
            .collect();
        let overall = Slot {
            field: field.to_owned(),
            key: None,
        };
        if let Some(settled) = index.values.get(&overall) {
            for share in self.shares(settled) {
                if !candidates.iter().any(|c| c.value == share.value) {
                    candidates.push(Candidate {
                        value: share.value,
                        score: share.share,
                        evidence: None,
                        trusted: false,
                    });
                }
            }
        }

        candidates.truncate(count);
        candidates
    }

    /// The score of the best-ranked value settled under the known keys, whether it is `value`, and the key that backs it
    /// most strongly — what the field's key threshold is compared with. `None` when no known key was seen. Values a field
    /// must not miss are not yet supported, so [`First::caught`] is always false.
    pub fn first(
        &self,
        form: &Form,
        field: &str,
        known: &BTreeMap<String, String>,
        value: &str,
        known_sets: &BTreeMap<String, Vec<String>>,
    ) -> Option<First> {
        let index = self.forms.get(&form.name)?;
        let ranked = self.keyed(index, form, form.field(field), known, known_sets);
        ranked.into_iter().next().map(|(best, score, key)| First {
            score,
            matches: best == value,
            key: format!("{}: {}", key.field, key.value),
            caught: false,
        })
    }

    /// The values settled under the known keys, best first: by score, then latest settlement, then scalar order. Each
    /// carries the key it is strongest under.
    fn keyed(
        &self,
        index: &FormIndex,
        form: &Form,
        field: &Field,
        known: &BTreeMap<String, String>,
        known_sets: &BTreeMap<String, Vec<String>>,
    ) -> Vec<(String, f64, Key)> {
        // value → (score, strongest, backing key, latest); summed in the order the keys come.
        let mut scores: Vec<(String, f64, f64, Key, i64)> = Vec::new();
        for key in self.keys(form, field, known, known_sets) {
            let slot = Slot {
                field: field.name.clone(),
                key: Some(key.clone()),
            };
            let Some(settled) = index.values.get(&slot) else {
                continue;
            };
            let concentration = field.concentration(&key.field);
            for share in self.shares(settled) {
                let strength = share.weight / (share.total + concentration);
                match scores.iter_mut().find(|s| s.0 == share.value) {
                    Some(entry) => {
                        if strength > entry.2 {
                            entry.2 = strength;
                            entry.3 = key.clone();
                        }
                        entry.1 += strength;
                        entry.4 = entry.4.max(share.latest);
                    }
                    None => {
                        scores.push((share.value, strength, strength, key.clone(), share.latest))
                    }
                }
            }
        }

        scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then(b.4.cmp(&a.4))
                .then(a.0.cmp(&b.0))
        });
        scores
            .into_iter()
            .map(|(value, score, _, key, _)| (value, score, key))
            .collect()
    }

    /// The keys the document's values give `field`: every other field that keys it, with a short enough value — each
    /// value of a field that takes several being a key of its own. Once each, in the form's order.
    fn keys(
        &self,
        form: &Form,
        field: &Field,
        values: &BTreeMap<String, String>,
        sets: &BTreeMap<String, Vec<String>>,
    ) -> Vec<Key> {
        let mut keys: Vec<Key> = Vec::new();
        let mut add = |key: Option<Key>| {
            if let Some(key) = key
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        };
        for evidence in form
            .fields
            .iter()
            .filter(|e| form.keys(&e.name, &field.name))
        {
            if evidence.multiple {
                let mut elements: Vec<&String> =
                    sets.get(&evidence.name).into_iter().flatten().collect();
                elements.sort();
                elements.dedup();
                for element in elements {
                    add(self.key_of(&evidence.name, element));
                }
            } else if let Some(raw) = values.get(&evidence.name) {
                add(self.key_of(&evidence.name, raw));
            }
        }

        keys
    }

    fn key_of(&self, field: &str, raw: &str) -> Option<Key> {
        text::key(raw, self.max_key_length).map(|value| Key {
            field: field.to_owned(),
            value,
        })
    }

    /// Each value's share of the weighted total, largest first; ties to the one settled most recently, then to the
    /// smaller value in scalar order. Settlements are folded from the earliest: each one leaves every earlier weight one
    /// step lighter and adds its own at 1 — the same operations, in the same order, as the reference implementation.
    fn shares(&self, settled: &[Settled]) -> Vec<Share> {
        let mut ordered: Vec<&Settled> = settled.iter().collect();
        ordered.sort_by(|a, b| a.at.cmp(&b.at).then(a.document.cmp(&b.document)));

        let mut raw: BTreeMap<String, (f64, i64)> = BTreeMap::new();
        let mut newest: Vec<&Settled> = Vec::new();
        let (mut raw_total, mut scale) = (0.0_f64, 1.0_f64);
        for s in ordered {
            if let Some(last) = newest.last()
                && last.at == s.at
                && last.document == s.document
            {
                newest.push(s); // one document's values settled together weigh alike
                continue;
            }

            if !newest.is_empty() {
                let amount = 1.0 / scale;
                for previous in &newest {
                    let entry = raw
                        .entry(previous.value.clone())
                        .or_insert((0.0, previous.at));
                    *entry = (entry.0 + amount, previous.at);
                    raw_total += amount;
                }

                newest.clear();
                scale *= self.recency_decay;
                if scale < 1e-150 {
                    for entry in raw.values_mut() {
                        entry.0 *= scale;
                    }
                    raw_total *= scale;
                    scale = 1.0;
                }
            }

            newest.push(s);
        }

        if newest.is_empty() {
            return Vec::new();
        }

        let mut weights: BTreeMap<String, (f64, i64)> = raw
            .into_iter()
            .map(|(v, (r, at))| (v, (r * scale, at)))
            .collect();
        for s in &newest {
            let own = weights.get(&s.value).map_or(0.0, |w| w.0);
            weights.insert(s.value.clone(), (own + 1.0, s.at));
        }

        let total = raw_total * scale + newest.len() as f64;
        let mut shares: Vec<Share> = weights
            .into_iter()
            .map(|(value, (weight, latest))| Share {
                value,
                share: weight / total,
                weight,
                total,
                latest,
            })
            .collect();
        shares.sort_by(|a, b| {
            b.share
                .partial_cmp(&a.share)
                .unwrap_or(Ordering::Equal)
                .then(b.latest.cmp(&a.latest))
                .then(a.value.cmp(&b.value))
        });
        shares
    }
}
