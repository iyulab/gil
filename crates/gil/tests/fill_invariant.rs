//! A value the application filled ([`SettlementKind::Fill`]) is kept in the record but is neither learned by the memories
//! nor asked about by a selection: no person settled it, so a memory that learned it would answer by its own earlier
//! answers and promise more than it keeps. These tests hold that, over a stream with fills interleaved, to the selection
//! giving the same thresholds, the same promises and the same lookups as the stream without them. The promise is read
//! from the latest [`RECENT_RECORDS`](gil::replay::RECENT_RECORDS) records, and that window counts every record, the fills
//! among them: the tests keep the stream inside it.

use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, LayerOptions, LayerThresholds, select_layers};
use gil::schema::{Attribute, AttributeRole, Record, Schema, SettlementKind};

const MINUTE: i64 = 60 * 10_000_000;
const TEAMS: [&str; 4] = ["network", "desk", "facilities", "billing"];

fn schema() -> Schema {
    Schema::new(
        "ticket",
        vec![
            Attribute::new("source", AttributeRole::Observed),
            Attribute::new("summary", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
}

/// A small linear congruential generator, so the records come out the same on every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// `n` records settled by a person, each to its source's usual team now and then another, saying a word of its team in
/// its summary some of the time; every record is settled `step` minutes after the one before it.
fn settled(n: usize, seed: u64) -> Vec<Record> {
    let mut random = Lcg(seed);
    (0..n)
        .map(|i| {
            let source = random.below(12) as usize;
            let usual = TEAMS[source % TEAMS.len()];
            let team = if random.below(20) < 15 {
                usual
            } else {
                TEAMS[random.below(TEAMS.len() as u64) as usize]
            };
            let word = if random.below(3) == 0 {
                format!("{team}-word")
            } else {
                format!("w{}", random.below(400))
            };
            Record {
                id: format!("t{i}"),
                values: [
                    ("source".to_owned(), format!("s{source}")),
                    (
                        "summary".to_owned(),
                        format!("{word} w{}", random.below(2_000)),
                    ),
                    ("team".to_owned(), team.to_owned()),
                ]
                .into_iter()
                .collect(),
                settled_at: i as i64 * 2 * MINUTE,
                ..Record::default()
            }
        })
        .collect()
}

/// `n` records the application filled, settled a minute after the person-settled record of the same place: the team the
/// key of their source most often settles to, which is what a filled value is — the memory's own answer given back.
fn filled(n: usize) -> Vec<Record> {
    let mut records = settled(n, 71);
    for (i, record) in records.iter_mut().enumerate() {
        record.id = format!("f{i}");
        record.settled_at = i as i64 * 2 * MINUTE + MINUTE;
        record
            .settlements
            .insert("team".to_owned(), SettlementKind::Fill);
    }
    records
}

fn chosen(records: &[Record]) -> (LayerThresholds, Vec<DecisionRecord>) {
    let mut decisions = Vec::new();
    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        records,
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: None,
            uncounted_batches: &[],
        },
        LayerOptions {
            sink: Some(&mut |d| decisions.push(d.clone())),
            ..LayerOptions::default()
        },
    );
    (layers, decisions)
}

#[test]
fn a_filled_value_is_neither_learned_nor_asked_by_a_selection() {
    let people = settled(600, 23);
    let both: Vec<Record> = people.iter().cloned().chain(filled(600)).collect();

    let (alone, alone_decisions) = chosen(&people);
    let (mixed, mixed_decisions) = chosen(&both);

    // The same thresholds, promises and counts, and the same lookups with the same scores and outcomes — the fills left
    // no trace in what was learned or asked. A decision record's `order` is its record's place in settlement order, which
    // counts every record, the fills among them, so it is the one thing left out of the comparison.
    let unplaced = |decisions: Vec<DecisionRecord>| -> Vec<DecisionRecord> {
        decisions
            .into_iter()
            .map(|d| DecisionRecord { order: None, ..d })
            .collect()
    };
    assert_eq!(mixed, alone);
    assert_eq!(unplaced(mixed_decisions), unplaced(alone_decisions));
    assert!(
        alone.key.chosen.is_some() || alone.memory.chosen.is_some(),
        "a stream that chooses something, or the comparison says nothing"
    );
}

#[test]
fn a_stream_of_nothing_but_fills_gives_a_selection_nothing_to_replay() {
    let (layers, decisions) = chosen(&filled(300));
    assert_eq!((layers.key.lookups, layers.memory.lookups), (0, 0));
    assert!(decisions.is_empty());
    assert_eq!(layers.key.chosen, None);
    assert_eq!(layers.memory.chosen, None);
}
