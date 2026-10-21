//! The similar record memory that needs no model: cosine nearest neighbour over character n-gram TF-IDF, one index
//! per task. Text is compared by its characters ([`text::grams`]), so it works in any script without a tokenizer.
//!
//! Weights follow the remembered answers: the inverse record frequencies are taken afresh whenever the number of
//! remembered answers has moved by a tenth (or by one, while there are fewer than ten) since they were last taken, and
//! every row and every lookup is weighted with the same ones in between — so a similarity stays a cosine, 1 for the same
//! text. Each distinct n-gram gets an id the first time it is remembered, in the n-grams' scalar order, and every sum is
//! taken in the order of the ids — always the same operations in the same order, so the similarities come out the same
//! to the bit.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::text;

/// A remembered record found by a lookup: its key, how similar its text is to the request, and its answer.
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
        // A total order (similarity, then first remembered), so selecting the first `count` and sorting them gives the
        // same leaders as sorting every row.
        let by = |a: &usize, b: &usize| {
            similarities[*b]
                .partial_cmp(&similarities[*a])
                .unwrap_or(Ordering::Equal)
                // Lazily: reading the rows for every comparison, not only on a tie, cost a cache miss each time.
                .then_with(|| index.rows[*a].order.cmp(&index.rows[*b].order))
        };
        if count < ranked.len() {
            ranked.select_nth_unstable_by(count, by);
            ranked.truncate(count);
        }
        ranked.sort_by(by);
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
}

#[derive(Default)]
struct Index {
    rows: Vec<Row>,
    /// Each row's weight norm, beside the rows: a lookup reads every one, and reading them out of the rows cost a cache
    /// miss a row.
    norms: Vec<f64>,
    remembered: u64,
    position: HashMap<String, usize>,
    ids: HashMap<String, usize>,
    /// How many rows hold each id.
    frequency: Vec<i64>,
    /// The rows holding each id with the id's weight in that row, in no particular order — a lookup visits only the
    /// rows that share an n-gram with it, and reads each weight where it walks. Rebuilt whenever the weights are retaken.
    postings: Vec<Vec<(usize, f64)>>,
    /// The inverse record frequency of every id known when there were `taken_at` rows; later ids had none.
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
        };
        match existing {
            Some(at) => {
                let old = std::mem::replace(&mut self.rows[at], row);
                self.norms[at] = norm;
                tally(&mut self.frequency, &old.ids, -1);
                for &id in &old.ids {
                    unpost(&mut self.postings[id], at);
                }
            }
            None => {
                self.position.insert(key.to_owned(), self.rows.len());
                self.rows.push(row);
                self.norms.push(norm);
            }
        }

        let at = existing.unwrap_or(self.rows.len() - 1);
        tally(&mut self.frequency, &self.rows[at].ids, 1);
        let row = &self.rows[at];
        for (&id, &weight) in row.ids.iter().zip(&row.weights) {
            self.postings[id].push((at, weight));
        }
        self.refresh();
    }

    fn remove(&mut self, key: &str) {
        let Some(at) = self.position.remove(key) else {
            return;
        };

        let row = self.rows.swap_remove(at);
        self.norms.swap_remove(at);
        tally(&mut self.frequency, &row.ids, -1);
        for &id in &row.ids {
            unpost(&mut self.postings[id], at);
        }
        if at < self.rows.len() {
            // The last row moved into the gap: its postings point at its old place.
            let moved_from = self.rows.len();
            self.position.insert(self.rows[at].key.clone(), at);
            for &id in &self.rows[at].ids {
                let entry = self.postings[id]
                    .iter_mut()
                    .find(|p| p.0 == moved_from)
                    .expect("a moved row is posted under each of its ids");
                entry.0 = at;
            }
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

        // The products of the shared ids, added to each row in ascending id order (the order of `ids`) — the same
        // additions, in the same order, as walking each row against the query, but only over the rows that share one.
        let mut sums = vec![0.0_f64; self.rows.len()];
        for (i, &id) in ids.iter().enumerate() {
            let Some(rows) = self.postings.get(id) else {
                continue;
            };
            for &(r, weight) in rows {
                sums[r] += weights[i] * weight;
            }
        }

        for ((similarity, &norm), sum) in similarities.iter_mut().zip(&self.norms).zip(sums) {
            *similarity = if norm > 0.0 {
                sum / (query_norm * norm)
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
                    self.postings.push(Vec::new());
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
        let (mut rows, mut norms) = (
            std::mem::take(&mut self.rows),
            std::mem::take(&mut self.norms),
        );
        for (row, norm) in rows.iter_mut().zip(&mut norms) {
            *norm = self.weigh(&row.ids, &row.counts, &mut row.weights);
        }
        (self.rows, self.norms) = (rows, norms);
        for postings in &mut self.postings {
            postings.clear();
        }
        for (r, row) in self.rows.iter().enumerate() {
            for (&id, &weight) in row.ids.iter().zip(&row.weights) {
                self.postings[id].push((r, weight));
            }
        }
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

    /// The smoothed inverse record frequency at the last refresh: never zero, highest for an n-gram no row had.
    fn smoothed(&self, frequency: i64) -> f64 {
        ((1.0 + self.taken_at as f64) / (1.0 + frequency as f64)).ln() + 1.0
    }
}

/// Takes `row` out of one id's postings.
fn unpost(postings: &mut Vec<(usize, f64)>, row: usize) {
    let at = postings
        .iter()
        .position(|p| p.0 == row)
        .expect("a row is posted under each of its ids");
    postings.swap_remove(at);
}

/// Adds `delta` to the frequency of each id.
fn tally(frequency: &mut [i64], ids: &[usize], delta: i64) {
    for &id in ids {
        frequency[id] += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(memory: &LexicalMemory, task: &str, state: &str) -> Option<Match> {
        memory.nearest(task, state, 1).into_iter().next()
    }

    #[test]
    fn nearest_on_an_empty_memory_is_empty() {
        let memory = LexicalMemory::default();
        assert!(memory.nearest("task", "anything", 3).is_empty());
        assert_eq!(memory.count("task"), 0);
    }

    #[test]
    fn weights_are_taken_afresh_once_the_memory_has_grown_by_a_tenth() {
        let mut memory = LexicalMemory::default();
        for i in 0..20 {
            memory.remember(
                "task",
                &format!("r{i}"),
                &format!("row{i:02} common"),
                &format!("a{i}"),
            );
        }

        let taken = lookup(&memory, "task", "common").unwrap();
        memory.remember("task", "r20", "row20 common", "a20");
        let within = lookup(&memory, "task", "common").unwrap();
        memory.remember("task", "r21", "row21 common", "a21");
        let refreshed = lookup(&memory, "task", "common").unwrap();

        // 21 rows: not yet a tenth more than 20.
        assert_eq!(within.similarity.to_bits(), taken.similarity.to_bits());
        // 22 rows: an n-gram every row shares counts for less.
        assert!(refreshed.similarity < taken.similarity);
        assert_eq!(
            (
                taken.source.as_str(),
                within.source.as_str(),
                refreshed.source.as_str()
            ),
            ("r0", "r0", "r0")
        );
    }

    #[test]
    fn forgetting_a_row_keeps_the_remembered_order_of_the_rest_for_ties() {
        let mut memory = LexicalMemory::default();
        for key in ["a", "b", "c", "d"] {
            memory.remember("task", key, "same words", key);
        }

        memory.forget("task", "b"); // the last row takes its place
        memory.forget("task", "a");
        let tie = lookup(&memory, "task", "same words").unwrap();
        memory.remember("task", "c", "same words", "c again"); // keeps its place in the order
        memory.remember("task", "a", "same words", "a again"); // remembered anew: last
        let ranked: Vec<(String, String)> = memory
            .nearest("task", "same words", 3)
            .into_iter()
            .map(|m| (m.source, m.answer))
            .collect();

        assert_eq!(tie.source, "c");
        assert_eq!(
            ranked,
            [("c", "c again"), ("d", "d"), ("a", "a again")]
                .map(|(s, a)| (s.to_owned(), a.to_owned()))
        );
    }

    #[test]
    fn empty_or_one_character_text_is_handled() {
        for text in ["", "   ", "a"] {
            let mut memory = LexicalMemory::default();
            memory.remember("task", "t1", text, "answer");
            memory.remember("task", "t2", "a longer request", "other");

            let found =
                lookup(&memory, "task", text).expect("a lookup in a non-empty memory finds a row");
            assert!(
                (0.0..=1.0 + 1e-9).contains(&found.similarity),
                "{text:?}: {}",
                found.similarity
            );
        }
    }

    #[test]
    fn rows_replaced_and_forgotten_many_times_leave_no_trace_in_lookups() {
        let mut memory = LexicalMemory::default();
        memory.remember(
            "task",
            "kept",
            "my card was blocked at the shop",
            "cards_block",
        );
        for i in 0..40 {
            memory.remember(
                "task",
                "moving",
                &format!("refund order number {i} please"),
                &format!("answer_{i}"),
            );
            memory.remember(
                "task",
                &format!("brief_{i}"),
                &format!("where is parcel {i}"),
                "tracking",
            );
            memory.forget("task", &format!("brief_{i}"));
        }

        let all = memory.nearest("task", "refund order number 39 please", 10);
        let stale = memory.nearest("task", "refund order number 0 please", 10);

        let sources = |m: &[Match]| m.iter().map(|m| m.source.clone()).collect::<Vec<_>>();
        assert_eq!(sources(&all), ["moving", "kept"]);
        assert_eq!(all[0].answer, "answer_39");
        assert!((all[0].similarity - 1.0).abs() < 1e-12);
        // The old text is gone with the row it was in.
        assert_eq!(sources(&stale), ["moving", "kept"]);
        assert_eq!(stale[0].answer, "answer_39");
        assert_eq!(memory.count("task"), 2);
    }

    #[test]
    fn korean_text_matches_without_a_tokenizer_and_decomposed_hangul_is_composed_first() {
        use unicode_normalization::UnicodeNormalization;

        let mut memory = LexicalMemory::default();
        memory.remember(
            "task",
            "t1",
            "로그인 화면에서 비밀번호 오류가 납니다",
            "auth",
        );
        memory.remember("task", "t2", "결제 금액이 두 번 청구되었어요", "billing");

        let found = lookup(&memory, "task", "비밀번호 오류로 로그인이 안 돼요").unwrap();
        // The same sentence typed as conjoining jamo (NFD) reads as the composed syllables.
        let nfd: String = "결제 금액이 두 번 청구되었어요".nfd().collect();
        assert_ne!(nfd, "결제 금액이 두 번 청구되었어요");
        let decomposed = lookup(&memory, "task", &nfd).unwrap();

        assert_eq!(found.answer, "auth");
        assert_eq!(decomposed.answer, "billing");
        assert!((decomposed.similarity - 1.0).abs() < 1e-9);
    }
}
