//! The settled field memory: suggests a judged field's value from how often each value was settled alongside the values
//! of the document's other fields. Each other field's value is a key; a value's score is the sum over the document's
//! keys of its strength there — its weighted count over the key's weighted total plus the key field's concentration.
//! Recent settlements weigh more: under each key, a settlement counts `recency_decay` to the power of the settlements
//! made under the same key after it.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::form::{Document, Field, Form, Role};
use crate::{hangul, text};

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
    /// The best value.
    pub value: String,
    pub matches: bool,
    pub key: String,
    /// Whether a value the field must not miss was settled under the known keys — then the key layer does not answer.
    pub caught: bool,
}

/// The sets a lookup without any carries.
static NO_SETS: BTreeMap<String, Vec<String>> = BTreeMap::new();

/// What one field is looked up with — the key layer, the similar document layer and the replays take the same shape.
#[derive(Clone, Copy, Debug)]
pub struct Lookup<'a> {
    pub form: &'a Form,
    /// The field looked up.
    pub field: &'a str,
    /// The values known so far.
    pub known: &'a BTreeMap<String, String>,
    /// The values known so far of the fields that take several.
    pub known_sets: &'a BTreeMap<String, Vec<String>>,
    /// The text typed into the field so far; empty text reads as none.
    pub typed: Option<&'a str>,
    /// The saved document asked about again, whose settlements are no evidence for it.
    pub excluding: Option<&'a str>,
}

impl<'a> Lookup<'a> {
    /// A lookup of `field` from the values known so far: no sets known, nothing typed, no document left out.
    pub fn new(form: &'a Form, field: &'a str, known: &'a BTreeMap<String, String>) -> Self {
        Self {
            form,
            field,
            known,
            known_sets: &NO_SETS,
            typed: None,
            excluding: None,
        }
    }

    /// The lookup with the values known so far of the fields that take several.
    pub fn sets(self, known_sets: &'a BTreeMap<String, Vec<String>>) -> Self {
        Self { known_sets, ..self }
    }

    /// The lookup with the text typed into the field so far; empty text reads as none.
    pub fn typed(self, typed: Option<&'a str>) -> Self {
        Self {
            typed: typed.filter(|t| !t.is_empty()),
            ..self
        }
    }

    /// The lookup leaving out the settlements of `document`, a saved document asked about again.
    pub fn excluding(self, document: Option<&'a str>) -> Self {
        Self {
            excluding: document,
            ..self
        }
    }

    /// The field looked up.
    fn definition(&self) -> &'a Field {
        self.form.field(self.field)
    }
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

    /// Replaces what the document contributed with what its current values imply. A field that takes several values
    /// settles each of them: overall, under the document's keys, and under each of its other values — the values of one
    /// document weigh alike, as one settlement, whatever order they are listed in.
    pub fn put(&mut self, form: &Form, document: &Document) {
        self.remove(&form.name, &document.id);
        let mut tallies = Vec::new();
        for field in form.fields.iter().filter(|f| f.role == Role::Judged) {
            if field.multiple {
                let elements = elements(&document.sets, &field.name);
                let keys = self.keys(form, field, &document.values, &document.sets);
                for element in &elements {
                    let settled = Settled {
                        value: (*element).clone(),
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
                    let mut under = keys.clone();
                    for other in elements.iter().filter(|e| *e != element) {
                        if let Some(key) = self.key_of(&field.name, other)
                            && !under.contains(&key)
                        {
                            under.push(key);
                        }
                    }
                    for key in under {
                        tallies.push((
                            Slot {
                                field: field.name.clone(),
                                key: Some(key),
                            },
                            settled.clone(),
                        ));
                    }
                }

                continue;
            }

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

    /// The `count` values most likely for the looked-up field given the values known so far, best first: the values
    /// settled under the known keys by their score (with the key backing each most strongly), then the field's most
    /// frequently settled values by their share — only values that begin with the text typed so far, if any. The keyed
    /// values are trusted when the first one's score reaches the field's key threshold for that many characters typed,
    /// or the same value was already trusted with fewer of them. The settlements of the document the lookup excludes,
    /// one asked about again once saved, are no evidence for it; for a field that takes several values, each value on
    /// its own score, and the values already chosen in the known sets are evidence and are not offered again.
    pub fn rank(&self, q: &Lookup, count: usize) -> Vec<Candidate> {
        let q = q.typed(q.typed);
        let (typed, excluding) = (q.typed, q.excluding);
        let definition = q.definition();
        let Some(index) = self.forms.get(&q.form.name) else {
            return Vec::new();
        };

        let chosen: Vec<&String> = if definition.multiple {
            elements(q.known_sets, q.field)
        } else {
            Vec::new()
        };
        let ranked: Vec<(String, f64, Key)> = self
            .keyed(index, &q)
            .into_iter()
            .filter(|(value, _, _)| !chosen.contains(&value))
            .collect();
        let threshold = definition.key_threshold_for(typed.map_or(0, text::scalars));
        let reaches = |score: f64| threshold.is_some_and(|t| score >= t);
        let layer_trusted = ranked.first().is_some_and(|(best, score, _)| {
            reaches(*score)
                || typed.is_some_and(|typed| {
                    !definition.multiple
                        && claimed(&self.keyed(index, &q.typed(None)), definition, typed, best)
                })
        }) && !self.caught(index, &q);
        let mut candidates: Vec<Candidate> = ranked
            .into_iter()
            .take(count)
            .enumerate()
            .map(|(i, (value, score, key))| Candidate {
                value,
                score,
                evidence: Some(format!("{}: {}", key.field, key.value)),
                // One value answers a field that takes one value; each value whose own score reaches the threshold, a set.
                trusted: if definition.multiple {
                    reaches(score)
                } else {
                    i == 0 && layer_trusted
                },
            })
            .collect();
        let overall = Slot {
            field: q.field.to_owned(),
            key: None,
        };
        if let Some(settled) = index.values.get(&overall) {
            for share in self.shares(settled, excluding) {
                if definition.admits(&share.value)
                    && begins(&share.value, typed)
                    && !chosen.contains(&&share.value)
                    && !candidates.iter().any(|c| c.value == share.value)
                {
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
    /// most strongly — what the field's key threshold is compared with unless a value the field must not miss was settled
    /// under the known keys ([`First::caught`]). `None` when no known key was seen. Reads the lookup's form, field, known
    /// values and sets; its typed text and excluded document are not taken into account.
    pub fn first(&self, q: &Lookup, value: &str) -> Option<First> {
        let index = self.forms.get(&q.form.name)?;
        let q = q.typed(None).excluding(None);
        let caught = self.caught(index, &q);
        let ranked = self.keyed(index, &q);
        ranked.into_iter().next().map(|(best, score, key)| First {
            score,
            matches: best == value,
            value: best,
            key: format!("{}: {}", key.field, key.value),
            caught,
        })
    }

    /// As [`FieldMemory::first`], with none, one, … up to `longest` characters of `value` typed into the field: for each
    /// length, the best-ranked value that begins with that much of it and its score — or `None` when no value under the
    /// known keys does, or when a value the field must not miss holds the key layer back. Reads the lookup's form, field,
    /// known values and sets; its own typed text and excluded document are not taken into account.
    pub fn typed(&self, q: &Lookup, value: &str, longest: usize) -> Vec<Option<(f64, String)>> {
        let mut best = vec![None; longest.min(text::scalars(value)) + 1];
        let Some(index) = self.forms.get(&q.form.name) else {
            return best;
        };
        let q = q.typed(None).excluding(None);
        if self.caught(index, &q) {
            return best; // the key layer does not answer here at any length
        }

        let ranked = self.keyed(index, &q);
        for (length, slot) in best.iter_mut().enumerate() {
            let typed = (length > 0).then(|| text::prefix(value, length));
            *slot = ranked
                .iter()
                .find(|(v, _, _)| begins(v, typed))
                .map(|(v, score, _)| (*score, v.clone()));
        }

        best
    }

    /// Every value settled under the known keys, best first: its score and the key that backs it most strongly
    /// (`field: value`) — what a field's coarse level adds up by class. Empty when no known key was seen. The
    /// settlements of the document the lookup excludes, one asked about again, are left out; its typed text is not
    /// taken into account.
    pub fn keyed_values(&self, q: &Lookup) -> Vec<(String, f64, String)> {
        let Some(index) = self.forms.get(&q.form.name) else {
            return Vec::new();
        };
        let q = q.typed(None);
        if self.caught(index, &q) {
            return Vec::new();
        }

        self.keyed(index, &q)
            .into_iter()
            .map(|(value, score, key)| (value, score, format!("{}: {}", key.field, key.value)))
            .collect()
    }

    /// For each key the known values give the looked-up field and that was seen before: the key's field, its weighted
    /// total and the weight `value` has under it (0 when the key has not had it) — what the next settlement under each
    /// key is predicted from. Reads the lookup's form, field and known values only: its sets, typed text and excluded
    /// document are not taken into account.
    pub fn key_totals(&self, q: &Lookup, value: &str) -> Vec<(String, f64, f64)> {
        let Some(index) = self.forms.get(&q.form.name) else {
            return Vec::new();
        };

        let mut found = Vec::new();
        for key in self.keys(q.form, q.definition(), q.known, &NO_SETS) {
            let slot = Slot {
                field: q.field.to_owned(),
                key: Some(key.clone()),
            };
            let Some(settled) = index.values.get(&slot) else {
                continue;
            };
            let shares = self.shares(settled, None);
            if let Some(first) = shares.first() {
                let own = shares
                    .iter()
                    .find(|s| s.value == value)
                    .map_or(0.0, |s| s.weight);
                found.push((key.field, first.total, own));
            }
        }

        found
    }

    /// Every value of a field that takes several settled under the known keys, best first, with the score each is compared
    /// with the field's key threshold on — the values already chosen (the field's own entry in the known sets) left out.
    /// Reads the lookup's form, field, known values and sets; its typed text and excluded document are not taken into
    /// account.
    pub fn scored(&self, q: &Lookup) -> Vec<(String, f64)> {
        let Some(index) = self.forms.get(&q.form.name) else {
            return Vec::new();
        };
        let chosen = elements(q.known_sets, q.field);
        self.keyed(index, &q.typed(None).excluding(None))
            .into_iter()
            .filter(|(value, _, _)| !chosen.contains(&value))
            .map(|(value, score, _)| (value, score))
            .collect()
    }

    /// Whether a value the field must not miss was ever settled under a key the document has — then no layer answers the
    /// field for it. The settlements of the document the lookup excludes are left out; its typed text is not taken into
    /// account.
    pub fn catches(&self, q: &Lookup) -> bool {
        self.forms
            .get(&q.form.name)
            .is_some_and(|index| self.caught(index, q))
    }

    /// Whether a value the field must not miss was ever settled under a key the document has — then no layer answers the
    /// field for it. The lookup's typed text is not read.
    fn caught(&self, index: &FormIndex, q: &Lookup) -> bool {
        let field = q.definition();
        let Some(must_catch) = field.must_catch.as_ref().filter(|m| !m.is_empty()) else {
            return false;
        };
        if field.multiple {
            return false;
        }

        self.keys(q.form, field, q.known, q.known_sets)
            .into_iter()
            .any(|key| {
                let slot = Slot {
                    field: field.name.clone(),
                    key: Some(key),
                };
                index.values.get(&slot).is_some_and(|settled| {
                    self.shares(settled, q.excluding)
                        .iter()
                        .any(|s| must_catch.contains(&s.value))
                })
            })
    }

    /// The values settled under the known keys, best first: by score, then latest settlement, then scalar order. Each
    /// carries the key it is strongest under. Values outside the field's domain, or not beginning with the typed text,
    /// are left out, and so are the settlements of the document the lookup excludes.
    fn keyed(&self, index: &FormIndex, q: &Lookup) -> Vec<(String, f64, Key)> {
        let field = q.definition();
        let (typed, excluding) = (q.typed, q.excluding);
        // value → (score, strongest, backing key, latest); summed in the order the keys come.
        let mut scores: Vec<(String, f64, f64, Key, i64)> = Vec::new();
        let mut keys = self.keys(q.form, field, q.known, q.known_sets);
        if field.multiple {
            // A field that takes several values: its chosen values are keys for the rest.
            for element in elements(q.known_sets, &field.name) {
                if let Some(key) = self.key_of(&field.name, element)
                    && !keys.contains(&key)
                {
                    keys.push(key);
                }
            }
        }

        for key in keys {
            let slot = Slot {
                field: field.name.clone(),
                key: Some(key.clone()),
            };
            let Some(settled) = index.values.get(&slot) else {
                continue;
            };
            let concentration = field.concentration(&key.field);
            // A value outside the domain, or not beginning with the typed text, is not offered, but it still counts
            // towards its key's total.
            for share in self
                .shares(settled, excluding)
                .into_iter()
                .filter(|s| field.admits(&s.value) && begins(&s.value, typed))
            {
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
                for element in elements(sets, &evidence.name) {
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
    fn shares(&self, settled: &[Settled], excluding: Option<&str>) -> Vec<Share> {
        let mut ordered: Vec<&Settled> = settled
            .iter()
            .filter(|s| excluding != Some(s.document.as_str()))
            .collect();
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

/// A set field's values, once each and in scalar order — the order a set is listed in carries no meaning.
/// Whether `value` begins with the text typed so far, ignoring case; always when none is. Korean is typed a letter at a
/// time and the last syllable may still be composing, so text with Hangul begins a value when its keystrokes begin the
/// value's.
pub fn begins(value: &str, typed: Option<&str>) -> bool {
    typed.is_none_or(|typed| {
        text::starts_with_ignoring_case(value, typed)
            || (hangul::has(typed)
                && text::starts_with_ignoring_case(
                    &hangul::keystrokes(value),
                    &hangul::keystrokes(typed),
                ))
    })
}

/// Whether `value`, the best value that begins with `typed`, was already trusted with fewer of its characters typed —
/// the best value there too, with a score that reached the threshold for that many. It is the same claim, not a new one.
fn claimed(ranked: &[(String, f64, Key)], field: &Field, typed: &str, value: &str) -> bool {
    (0..text::scalars(typed)).any(|length| {
        let prefix = (length > 0).then(|| text::prefix(typed, length));
        ranked
            .iter()
            .find(|(v, _, _)| begins(v, prefix))
            .is_some_and(|(v, score, _)| {
                v == value && field.key_threshold_for(length).is_some_and(|t| *score >= t)
            })
    })
}

fn elements<'a>(sets: &'a BTreeMap<String, Vec<String>>, field: &str) -> Vec<&'a String> {
    let mut elements: Vec<&String> = sets.get(field).into_iter().flatten().collect();
    elements.sort();
    elements.dedup();
    elements
}
