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
    /// The most threads a lookup is split among.
    threads: usize,
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
            threads: MOST_THREADS,
            indexes: HashMap::new(),
        }
    }

    /// The memory with a lookup split among at most `threads` threads — 1 keeps every lookup on the caller's thread. By
    /// default a lookup over a large memory is split among as many as the machine offers, up to eight; a host that
    /// answers many requests at once, or keeps its own budget of threads, sets it. The answers are the same to the bit
    /// whatever the number.
    pub fn with_threads(mut self, threads: usize) -> Self {
        assert!(threads >= 1, "threads must be at least 1");
        self.threads = threads;
        self
    }

    /// The most threads a lookup is split among.
    pub fn threads(&self) -> usize {
        self.threads
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

        index
            .nearest(
                &text::grams(state, self.min_gram, self.max_gram),
                count,
                self.threads,
            )
            .into_iter()
            .map(|(r, similarity)| Match {
                source: index.rows[r].key.clone(),
                similarity,
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

/// The slots of the rows holding one id and how many times each holds it, side by side, in the order they were posted —
/// the slots ascending. A row's weight for the id is that count × the id's inverse record frequency, which every entry
/// of the list shares until the weights are taken afresh and the lists rebuilt: a lookup multiplies it again, as the
/// row's own weights were, and reads a third less than a list of weights.
#[derive(Default)]
struct Postings {
    slots: Vec<u32>,
    counts: Vec<u32>,
}

/// A slot whose row is gone.
const GONE: u32 = u32::MAX;

/// The products a lookup adds before it is split among threads, and the most threads it is split among by default:
/// below the first, starting threads costs more than it saves.
const SPLIT_AT: usize = 1 << 18;
const MOST_THREADS: usize = 8;

/// Rows in no particular order — forgetting one moves the last into its place — each numbered in the order it was
/// remembered (a key remembered again keeps its number), ties going to the smaller number.
///
/// A lookup touches only the n-grams a row shares with it: every id lists, side by side, the slots of the rows that hold
/// it and their weights, and the query's ids are walked in ascending order, so each row's products are added in the
/// order a walk along both sorted lists would add them — the sums are the same to the bit. A row takes a slot when it is
/// posted and keeps it, so the lists only ever grow at the end, are read front to back, and write into one array of
/// sums. A row forgotten or replaced leaves its entries behind, summed but never read, until they pass an eighth of
/// the live ones and the lists are rebuilt; they are rebuilt too whenever the weights are taken afresh.
///
/// A lookup that would add many products splits the slots into ranges, one a thread: each range's sums take the same
/// additions in the same order, and each thread ranks its own rows, so the leaders are the same to the bit whatever
/// the number of threads — one where threads are not available.
#[derive(Default)]
struct Index {
    rows: Vec<Row>,
    /// Each row's weight norm and the slot its sum is kept in during a lookup, beside the rows: a lookup reads every
    /// one, and reading them out of the rows cost a cache miss a row.
    norms: Vec<f64>,
    slots: Vec<u32>,
    /// The row in each slot, [`GONE`] once it has left it.
    owners: Vec<u32>,
    remembered: u64,
    position: HashMap<String, usize>,
    ids: HashMap<String, usize>,
    /// How many rows hold each id.
    frequency: Vec<i64>,
    postings: Vec<Postings>,
    /// The next slot to give, and the posted entries of the rows still here and of those gone.
    next_slot: u32,
    live: usize,
    gone: usize,
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
        let at = match existing {
            Some(at) => {
                let old = std::mem::replace(&mut self.rows[at], row);
                self.norms[at] = norm;
                self.owners[self.slots[at] as usize] = GONE;
                tally(&mut self.frequency, &old.ids, -1);
                self.retire(&old);
                at
            }
            None => {
                self.position.insert(key.to_owned(), self.rows.len());
                self.rows.push(row);
                self.norms.push(norm);
                self.slots.push(0);
                self.rows.len() - 1
            }
        };

        tally(&mut self.frequency, &self.rows[at].ids, 1);
        if !self.refresh() {
            self.post(at);
            self.compact();
        }
    }

    fn remove(&mut self, key: &str) {
        let Some(at) = self.position.remove(key) else {
            return;
        };

        // The last row moves into the gap with its slot: no list changes.
        self.owners[self.slots[at] as usize] = GONE;
        let row = self.rows.swap_remove(at);
        self.norms.swap_remove(at);
        self.slots.swap_remove(at);
        tally(&mut self.frequency, &row.ids, -1);
        self.retire(&row);
        if at < self.rows.len() {
            self.position.insert(self.rows[at].key.clone(), at);
            self.owners[self.slots[at] as usize] = at as u32;
        }

        if !self.refresh() {
            self.compact();
        }
    }

    /// The query's known ids, ascending, their weights, and its norm. An n-gram no row has ever held matches nothing, but
    /// still weighs in the norm — added in the query's scalar order, then the known ones in the order of their ids.
    fn weigh_query(&self, query: &[(String, u32)]) -> (Vec<usize>, Vec<f64>, f64) {
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
        (ids, weights, (squares + known_norm * known_norm).sqrt())
    }

    /// The `count` rows most similar to the query and their similarities, most similar first; on a tie, the one
    /// remembered first.
    fn nearest(&self, query: &[(String, u32)], count: usize, most: usize) -> Vec<(usize, f64)> {
        let (ids, weights, query_norm) = self.weigh_query(query);
        let products: usize = ids
            .iter()
            .filter_map(|&id| self.postings.get(id))
            .map(|p| p.slots.len())
            .sum();
        let threads = if products < SPLIT_AT || query_norm == 0.0 || most == 1 {
            1
        } else {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(most))
        };
        let slots = self.next_slot as usize;
        let span = slots.div_ceil(threads).max(1);
        let leaders = |from: usize| {
            self.leaders(
                &ids,
                &weights,
                query_norm,
                from..(from + span).min(slots),
                count,
            )
        };

        let mut found: Vec<(usize, f64)> = if threads == 1 {
            leaders(0)
        } else {
            std::thread::scope(|scope| {
                let parts: Vec<_> = (0..slots)
                    .step_by(span)
                    .map(|from| scope.spawn(move || leaders(from)))
                    .collect();
                parts
                    .into_iter()
                    .flat_map(|part| part.join().expect("a lookup thread does not panic"))
                    .collect()
            })
        };
        self.rank(&mut found, count);
        found
    }

    /// The first `count` of the rows in the slots of `range` by similarity to the query, then first remembered. The
    /// products of the shared ids are added to each row in ascending id order (the order of `ids`) — the same additions,
    /// in the same order, as walking each row against the query, but only over the rows that share one.
    fn leaders(
        &self,
        ids: &[usize],
        weights: &[f64],
        query_norm: f64,
        range: std::ops::Range<usize>,
        count: usize,
    ) -> Vec<(usize, f64)> {
        let mut sums = vec![0.0_f64; range.len()];
        if query_norm > 0.0 {
            let (low, high) = (range.start as u32, range.end as u32);
            for (&id, &weight) in ids.iter().zip(weights) {
                let Some(postings) = self.postings.get(id) else {
                    continue;
                };
                let from = postings.slots.partition_point(|&s| s < low);
                let to = postings.slots.partition_point(|&s| s < high);
                let idf = self.idf_of(id);
                for (&slot, &count) in postings.slots[from..to]
                    .iter()
                    .zip(&postings.counts[from..to])
                {
                    sums[(slot - low) as usize] += weight * (f64::from(count) * idf);
                }
            }
        }

        let mut found: Vec<(usize, f64)> = self.owners[range]
            .iter()
            .zip(&sums)
            .filter(|(row, _)| **row != GONE)
            .map(|(&row, &sum)| {
                let norm = self.norms[row as usize];
                let similarity = if query_norm > 0.0 && norm > 0.0 {
                    sum / (query_norm * norm)
                } else {
                    0.0
                };
                (row as usize, similarity)
            })
            .collect();
        self.rank(&mut found, count);
        found
    }

    /// Keeps the first `count` rows by similarity, then first remembered — a total order, so selecting the first
    /// `count` and sorting them gives the same leaders as sorting every row.
    fn rank(&self, found: &mut Vec<(usize, f64)>, count: usize) {
        let by = |a: &(usize, f64), b: &(usize, f64)| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                // Lazily: reading the rows for every comparison, not only on a tie, cost a cache miss each time.
                .then_with(|| self.rows[a.0].order.cmp(&self.rows[b.0].order))
        };
        if count < found.len() {
            found.select_nth_unstable_by(count, by);
            found.truncate(count);
        }
        found.sort_by(by);
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
                    self.postings.push(Postings::default());
                    id
                }
            };
            pairs.push((id, *count));
        }

        pairs.sort_unstable();
        pairs.into_iter().unzip()
    }

    /// Takes the frequencies afresh once the row count has moved by a tenth (at least one) since last time, reweighs
    /// every row and rebuilds the lists; false when it is not yet time.
    fn refresh(&mut self) -> bool {
        if self.rows.len().abs_diff(self.taken_at) < (self.taken_at / 10).max(1) {
            return false;
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
        self.repost();
        true
    }

    /// Lists the row at `at` under each of its ids, in a slot of its own.
    fn post(&mut self, at: usize) {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.slots[at] = slot;
        self.owners.push(at as u32);
        let row = &self.rows[at];
        for (&id, &count) in row.ids.iter().zip(&row.counts) {
            let postings = &mut self.postings[id];
            postings.slots.push(slot);
            postings.counts.push(count);
        }
        self.live += row.ids.len();
    }

    /// Counts a row's entries as gone: they stay in the lists, summed but never read, until the next rebuild.
    fn retire(&mut self, row: &Row) {
        self.live -= row.ids.len();
        self.gone += row.ids.len();
    }

    /// Rebuilds the lists once the entries of rows gone pass an eighth of the live ones: a lookup walks them too, so
    /// they cost it at most an eighth more, and a rebuild comes only after that many entries were retired.
    fn compact(&mut self) {
        if self.gone * 8 > self.live {
            self.repost();
        }
    }

    /// Lists the rows afresh, in slots numbered from zero in the rows' order, with their current weights.
    fn repost(&mut self) {
        for postings in &mut self.postings {
            postings.slots.clear();
            postings.counts.clear();
        }
        self.owners.clear();
        (self.next_slot, self.live, self.gone) = (0, 0, 0);
        for at in 0..self.rows.len() {
            self.post(at);
        }
    }

    /// Fills `weights` with count × idf and returns their norm.
    fn weigh(&self, ids: &[usize], counts: &[u32], weights: &mut [f64]) -> f64 {
        let mut squares = 0.0_f64;
        for i in 0..ids.len() {
            weights[i] = f64::from(counts[i]) * self.idf_of(ids[i]);
            squares += weights[i] * weights[i];
        }

        squares.sqrt()
    }

    /// The inverse record frequency of `id` at the last refresh; an id first seen since then had none.
    fn idf_of(&self, id: usize) -> f64 {
        self.idf
            .get(id)
            .copied()
            .unwrap_or_else(|| self.smoothed(0))
    }

    /// The smoothed inverse record frequency at the last refresh: never zero, highest for an n-gram no row had.
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
    fn rows_remembered_again_in_another_order_answer_to_the_bit_as_before() {
        let text = |i: usize| {
            format!(
                "component: c{}\nsummary: unit {} fails after {} hours",
                i % 7,
                i % 13,
                i % 5
            )
        };
        let mut memory = LexicalMemory::default();
        for i in 0..60 {
            memory.remember("task", &format!("r{i}"), &text(i), &format!("a{}", i % 4));
        }
        let queries: Vec<String> = (100..110).map(text).collect();
        let before: Vec<Vec<(String, u64)>> = queries
            .iter()
            .map(|q| {
                let found = memory.nearest("task", q, 5);
                found
                    .into_iter()
                    .map(|m| (m.source, m.similarity.to_bits()))
                    .collect()
            })
            .collect();

        // Every row remembered again with the same text, from the last to the first: the lists are rebuilt along the
        // way, each time the old entries pass an eighth of the live ones.
        for i in (0..60).rev() {
            memory.remember("task", &format!("r{i}"), &text(i), &format!("a{}", i % 4));
        }
        let after: Vec<Vec<(String, u64)>> = queries
            .iter()
            .map(|q| {
                let found = memory.nearest("task", q, 5);
                found
                    .into_iter()
                    .map(|m| (m.source, m.similarity.to_bits()))
                    .collect()
            })
            .collect();

        assert_eq!(after, before);
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

    /// Every row's similarity by walking it against the query in ascending id order, ranked by similarity and then
    /// first remembered.
    fn ranked_by_every_row(
        memory: &LexicalMemory,
        state: &str,
        count: usize,
    ) -> Vec<(String, u64)> {
        let index = &memory.indexes["task"];
        let (ids, weights, query_norm) =
            index.weigh_query(&text::grams(state, memory.min_gram, memory.max_gram));
        let similarity = |row: &Row, norm: f64| {
            let mut sum = 0.0_f64;
            for (&id, &weight) in ids.iter().zip(&weights) {
                if let Ok(at) = row.ids.binary_search(&id) {
                    sum += weight * row.weights[at];
                }
            }
            if query_norm > 0.0 && norm > 0.0 {
                sum / (query_norm * norm)
            } else {
                0.0
            }
        };
        let mut rows: Vec<(usize, f64)> = index
            .rows
            .iter()
            .zip(&index.norms)
            .enumerate()
            .map(|(r, (row, &norm))| (r, similarity(row, norm)))
            .collect();
        rows.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then(index.rows[a.0].order.cmp(&index.rows[b.0].order))
        });
        rows.into_iter()
            .take(count)
            .map(|(r, s)| (index.rows[r].key.clone(), s.to_bits()))
            .collect()
    }

    /// The leaders of the slots split into `parts` ranges, each ranked on its own, then ranked together — as threads do.
    fn ranked_in_parts(
        memory: &LexicalMemory,
        state: &str,
        count: usize,
        parts: usize,
    ) -> Vec<(String, u64)> {
        let index = &memory.indexes["task"];
        let (ids, weights, query_norm) =
            index.weigh_query(&text::grams(state, memory.min_gram, memory.max_gram));
        let slots = index.next_slot as usize;
        let span = slots.div_ceil(parts).max(1);
        let mut found: Vec<(usize, f64)> = (0..slots)
            .step_by(span)
            .flat_map(|from| {
                index.leaders(
                    &ids,
                    &weights,
                    query_norm,
                    from..(from + span).min(slots),
                    count,
                )
            })
            .collect();
        index.rank(&mut found, count);
        found
            .into_iter()
            .map(|(r, s)| (index.rows[r].key.clone(), s.to_bits()))
            .collect()
    }

    #[test]
    fn a_lookup_kept_on_one_thread_finds_what_a_split_one_finds() {
        let text = |i: usize| {
            format!(
                "license inspection restaurant canvass risk high zip 606 {i} {}",
                i * 7
            )
        };
        let mut split = LexicalMemory::default();
        let mut single = LexicalMemory::default().with_threads(1);
        for i in 0..4_000 {
            split.remember("task", &format!("k{i}"), &text(i), &format!("a{}", i % 13));
            single.remember("task", &format!("k{i}"), &text(i), &format!("a{}", i % 13));
        }
        let index = &split.indexes["task"];
        let (ids, _, _) = index.weigh_query(&text::grams(&text(4_001), 2, 3));
        let products: usize = ids
            .iter()
            .filter_map(|&id| index.postings.get(id))
            .map(|p| p.slots.len())
            .sum();
        assert!(products >= SPLIT_AT, "the lookup is large enough to split");
        assert_eq!((split.threads(), single.threads()), (MOST_THREADS, 1));
        assert_eq!(
            split.nearest("task", &text(4_001), 10),
            single.nearest("task", &text(4_001), 10)
        );
    }

    #[test]
    #[should_panic(expected = "threads must be at least 1")]
    fn no_threads_is_refused() {
        let _ = LexicalMemory::default().with_threads(0);
    }

    #[test]
    fn the_nearest_are_the_same_to_the_bit_however_the_slots_are_split() {
        // Records alike in common words and apart in a few rare ones, remembered, replaced and forgotten in a fixed
        // pseudo-random order, so slots of gone rows sit among the live ones.
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let common = [
            "risk high",
            "license inspection",
            "canvass",
            "zip 606",
            "restaurant",
        ];
        let rare = [
            "walk-in cooler",
            "mouse droppings",
            "hand sink",
            "allergen card",
            "thermometer",
        ];
        let text = |next: &mut dyn FnMut(u64) -> u64| {
            let mut words: Vec<String> = Vec::new();
            for _ in 0..(1 + next(4)) {
                words.push(common[next(common.len() as u64) as usize].to_owned());
            }
            for _ in 0..next(3) {
                words.push(rare[next(rare.len() as u64) as usize].to_owned());
            }
            words.push(format!("no {}", next(40)));
            words.join(" ")
        };

        let mut memory = LexicalMemory::default();
        let mut lookups = 0;
        for step in 0..1_500 {
            let key = format!("r{}", next(600));
            if next(10) == 0 {
                memory.forget("task", &key);
            } else {
                let state = text(&mut next);
                memory.remember("task", &key, &state, &format!("a{}", next(7)));
            }
            if step % 7 == 0 && memory.count("task") > 0 {
                let state = if next(20) == 0 {
                    String::new()
                } else {
                    text(&mut next)
                };
                for count in [1, 3, 10, 11] {
                    let every = ranked_by_every_row(&memory, &state, count);
                    let found: Vec<(String, u64)> = memory
                        .nearest("task", &state, count)
                        .into_iter()
                        .map(|m| (m.source, m.similarity.to_bits()))
                        .collect();
                    assert_eq!(found, every, "step {step}, count {count}");
                    for parts in [2, 3, 7] {
                        assert_eq!(
                            ranked_in_parts(&memory, &state, count, parts),
                            every,
                            "step {step}, {parts} parts"
                        );
                    }
                    lookups += 1;
                }
            }
        }
        assert!(lookups > 800, "{lookups} lookups");
    }
}
