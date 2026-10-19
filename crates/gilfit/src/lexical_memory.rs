//! The similar document memory that needs no model: cosine nearest neighbour over character n-gram TF-IDF, one index
//! per task. Text is compared by its characters ([`text::grams`]), so it works in any script without a tokenizer.
//!
//! Weights follow the remembered answers: the inverse document frequencies are taken afresh whenever the number of
//! remembered answers has moved by a tenth (or by one, while there are fewer than ten) since they were last taken, and
//! every row and every lookup is weighted with the same ones in between — so a similarity stays a cosine, 1 for the same
//! text. Each distinct n-gram gets an id the first time it is remembered, in the n-grams' scalar order, and every sum is
//! taken in the order of the ids — the same operations, in the same order, as the reference implementation, so the
//! similarities come out the same to the bit.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::text;

/// A remembered document found by a lookup: its key, how similar its text is to the request, and its answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub source: String,
    pub similarity: f64,
    pub answer: String,
}

/// The lexical memory. Not thread-safe.
pub struct LexicalMemory {
    min_gram: usize,
    max_gram: usize,
    indexes: HashMap<String, Index>,
}

impl Default for LexicalMemory {
    fn default() -> Self {
        Self::new(2, 3)
    }
}

impl LexicalMemory {
    /// A memory comparing runs of `min_gram` to `max_gram` scalars.
    pub fn new(min_gram: usize, max_gram: usize) -> Self {
        assert!(min_gram >= 1, "min_gram must be at least 1");
        assert!(max_gram >= min_gram, "max_gram must be at least min_gram");
        Self {
            min_gram,
            max_gram,
            indexes: HashMap::new(),
        }
    }

    /// How many answers the task's memory holds.
    pub fn count(&self, task: &str) -> usize {
        self.indexes.get(task).map_or(0, |index| index.rows.len())
    }

    /// Remembers `answer` for the text `state` under `key`, replacing what the key held; a key remembered again keeps its
    /// place among ties.
    pub fn remember(&mut self, task: &str, key: &str, state: &str, answer: &str) {
        let grams = text::grams(state, self.min_gram, self.max_gram);
        self.indexes
            .entry(task.to_owned())
            .or_default()
            .put(key, &grams, answer);
    }

    /// Forgets what `key` held, if anything.
    pub fn forget(&mut self, task: &str, key: &str) {
        if let Some(index) = self.indexes.get_mut(task) {
            index.remove(key);
        }
    }

    /// The `count` remembered texts most similar to `state`, most similar first; on a tie, the one remembered first.
    /// Empty when the task's memory is empty.
    pub fn nearest(&self, task: &str, state: &str, count: usize) -> Vec<Match> {
        assert!(count >= 1, "count must be at least 1");
        let Some(index) = self.indexes.get(task).filter(|i| !i.rows.is_empty()) else {
            return Vec::new();
        };

        let similarities = index.similarities(&text::grams(state, self.min_gram, self.max_gram));
        let mut ranked: Vec<usize> = (0..similarities.len()).collect();
        ranked.sort_by(|&a, &b| {
            similarities[b]
                .partial_cmp(&similarities[a])
                .unwrap_or(Ordering::Equal)
                .then(index.rows[a].order.cmp(&index.rows[b].order))
        });
        ranked
            .into_iter()
            .take(count)
            .map(|r| Match {
                source: index.rows[r].key.clone(),
                similarity: similarities[r],
                answer: index.rows[r].answer.clone(),
            })
            .collect()
    }
}

/// One remembered text: its n-gram ids, ascending, with their counts and weights.
struct Row {
    key: String,
    answer: String,
    /// When the key was first remembered, among the rows of this index; breaks ties.
    order: u64,
    ids: Vec<usize>,
    counts: Vec<u32>,
    weights: Vec<f64>,
    norm: f64,
}

#[derive(Default)]
struct Index {
    rows: Vec<Row>,
    remembered: u64,
    position: HashMap<String, usize>,
    ids: HashMap<String, usize>,
    /// How many rows hold each id.
    frequency: Vec<i64>,
    /// The inverse document frequency of every id known when there were `taken_at` rows; later ids had none.
    idf: Vec<f64>,
    taken_at: usize,
}

impl Index {
    fn put(&mut self, key: &str, grams: &[(String, u32)], answer: &str) {
        let (ids, counts) = self.intern(grams);
        let existing = self.position.get(key).copied();
        let order = match existing {
            Some(at) => self.rows[at].order,
            None => {
                self.remembered += 1;
                self.remembered - 1
            }
        };
        let mut weights = vec![0.0; ids.len()];
        let norm = self.weigh(&ids, &counts, &mut weights);
        let row = Row {
            key: key.to_owned(),
            answer: answer.to_owned(),
            order,
            ids,
            counts,
            weights,
            norm,
        };
        match existing {
            Some(at) => {
                let old = std::mem::replace(&mut self.rows[at], row);
                tally(&mut self.frequency, &old.ids, -1);
            }
            None => {
                self.position.insert(key.to_owned(), self.rows.len());
                self.rows.push(row);
            }
        }

        let at = existing.unwrap_or(self.rows.len() - 1);
        tally(&mut self.frequency, &self.rows[at].ids, 1);
        self.refresh();
    }

    fn remove(&mut self, key: &str) {
        let Some(at) = self.position.remove(key) else {
            return;
        };

        let row = self.rows.swap_remove(at);
        tally(&mut self.frequency, &row.ids, -1);
        if at < self.rows.len() {
            self.position.insert(self.rows[at].key.clone(), at);
        }

        self.refresh();
    }

    /// The cosine of the query with every row, in the rows' order. An n-gram no row has ever held matches nothing, but
    /// still weighs in the query's norm — added in the query's scalar order, then the known ones in the order of their ids.
    fn similarities(&self, query: &[(String, u32)]) -> Vec<f64> {
        let mut known: Vec<(usize, u32)> = Vec::with_capacity(query.len());
        let mut squares = 0.0_f64;
        for (gram, count) in query {
            match self.ids.get(gram) {
                Some(&id) => known.push((id, *count)),
                // Squares by multiplication: `powf(2.0)` is a library `pow` (not correctly rounded on every platform)
                // unless the optimizer folds it, so it would differ between builds and from other implementations.
                None => {
                    let weight = f64::from(*count) * self.smoothed(0);
                    squares += weight * weight;
                }
            }
        }

        known.sort_unstable();
        let ids: Vec<usize> = known.iter().map(|k| k.0).collect();
        let counts: Vec<u32> = known.iter().map(|k| k.1).collect();
        let mut weights = vec![0.0; ids.len()];
        let known_norm = self.weigh(&ids, &counts, &mut weights);
        let query_norm = (squares + known_norm * known_norm).sqrt();

        let mut similarities = vec![0.0; self.rows.len()];
        if query_norm == 0.0 {
            return similarities;
        }

        for (similarity, row) in similarities.iter_mut().zip(&self.rows) {
            // The products of the shared ids, added in ascending id order: a walk along both sorted lists.
            let (mut sum, mut j) = (0.0_f64, 0);
            for (i, &id) in ids.iter().enumerate() {
                while j < row.ids.len() && row.ids[j] < id {
                    j += 1;
                }
                if j < row.ids.len() && row.ids[j] == id {
                    sum += weights[i] * row.weights[j];
                }
            }

            *similarity = if row.norm > 0.0 {
                sum / (query_norm * row.norm)
            } else {
                0.0
            };
        }

        similarities
    }

    /// The ids of the n-grams, ascending, with their counts. An n-gram seen for the first time takes the next id, in the
    /// n-grams' scalar order.
    fn intern(&mut self, grams: &[(String, u32)]) -> (Vec<usize>, Vec<u32>) {
        let mut pairs: Vec<(usize, u32)> = Vec::with_capacity(grams.len());
        for (gram, count) in grams {
            let id = match self.ids.get(gram) {
                Some(&id) => id,
                None => {
                    let id = self.ids.len();
                    self.ids.insert(gram.clone(), id);
                    self.frequency.push(0);
                    id
                }
            };
            pairs.push((id, *count));
        }

        pairs.sort_unstable();
        pairs.into_iter().unzip()
    }

    /// Takes the frequencies afresh once the row count has moved by a tenth (at least one) since last time, and reweighs
    /// every row.
    fn refresh(&mut self) {
        if self.rows.len().abs_diff(self.taken_at) < (self.taken_at / 10).max(1) {
            return;
        }

        self.taken_at = self.rows.len();
        self.idf = self.frequency.iter().map(|&f| self.smoothed(f)).collect();
        let mut rows = std::mem::take(&mut self.rows);
        for row in &mut rows {
            row.norm = self.weigh(&row.ids, &row.counts, &mut row.weights);
        }
        self.rows = rows;
    }

    /// Fills `weights` with count × idf and returns their norm.
    fn weigh(&self, ids: &[usize], counts: &[u32], weights: &mut [f64]) -> f64 {
        let mut squares = 0.0_f64;
        for i in 0..ids.len() {
            let idf = self
                .idf
                .get(ids[i])
                .copied()
                .unwrap_or_else(|| self.smoothed(0));
            weights[i] = f64::from(counts[i]) * idf;
            squares += weights[i] * weights[i];
        }

        squares.sqrt()
    }

    /// The smoothed inverse document frequency at the last refresh: never zero, highest for an n-gram no row had.
    fn smoothed(&self, frequency: i64) -> f64 {
        ((1.0 + self.taken_at as f64) / (1.0 + frequency as f64)).ln() + 1.0
    }
}

/// Adds `delta` to the frequency of each id.
fn tally(frequency: &mut [i64], ids: &[usize], delta: i64) {
    for &id in ids {
        frequency[id] += delta;
    }
}
