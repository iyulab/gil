//! What batches do to a promise: records settled together from one source, alike in what they show, are right or wrong
//! together, so choosing on records that batch mates span flatters the precision; counting across halves of the batches
//! does not. A replay reports this only when records name their batch, the same way on every run, and a record that
//! names none is a batch of its own.

use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, LayerOptions, LayerThresholds, select_layers};
use gil::schema::{Attribute, AttributeRole, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;
const TEAMS: [&str; 5] = ["network", "desk", "facilities", "security", "billing"];

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

/// `batches` batches of `size` records each. A batch comes from one of twenty sources and is settled to one team: the
/// source's usual team three times in four, another otherwise. Its records say nothing about the batch in their own words,
/// so the source answers them all alike — right or wrong together.
fn batched(batches: usize, size: usize) -> Vec<Record> {
    let mut random = Lcg(7);
    let mut records = Vec::new();
    for b in 0..batches {
        let source = random.below(20) as usize;
        let usual = TEAMS[source % TEAMS.len()];
        let team = if random.below(4) == 0 {
            TEAMS[random.below(TEAMS.len() as u64) as usize]
        } else {
            usual
        };
        for r in 0..size {
            let n = (b * size + r) as i64;
            records.push(Record {
                id: format!("t{n}"),
                values: [
                    ("source".to_owned(), format!("s{source}")),
                    (
                        "summary".to_owned(),
                        format!("w{} w{}", random.below(5_000), random.below(5_000)),
                    ),
                    ("team".to_owned(), team.to_owned()),
                ]
                .into_iter()
                .collect(),
                settled_at: n * MINUTE,
                batch: Some(format!("b{b}")),
                ..Record::default()
            });
        }
    }
    records
}

fn chosen(records: &[Record]) -> LayerThresholds {
    select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        records,
        Choice {
            target_precision: 0.7,
            minimum_answered: 30,
            counting: None,
        },
        LayerOptions::default(),
    )
}

#[test]
fn batch_mates_right_or_wrong_together_flatter_the_count_across_records() {
    let records = batched(300, 6);
    let check = chosen(&records)
        .batches
        .expect("the records name their batches");

    assert_eq!(check.batches, 300);
    let (across_batches, across_records) = (
        check
            .across_batches
            .expect("halves of the batches answered"),
        check
            .across_records
            .expect("halves of the records answered"),
    );
    assert!(
        across_records > across_batches,
        "across records {across_records} against across batches {across_batches}"
    );
    assert!(check.correlation.expect("batches of more than one answer") > 0.3);
    assert!(check.design_effect.expect("a design effect") > 2.0);
    let size = check.batch_size.expect("a batch size");
    assert!(size > 3.0 && size <= 6.0, "batch size {size}");
    assert!(check.across_batches_answered > 0 && check.across_records_answered > 0);
}

#[test]
fn records_that_name_no_batch_report_nothing_and_choose_the_same() {
    let records = batched(120, 5);
    let unnamed: Vec<Record> = records
        .iter()
        .map(|r| Record {
            batch: None,
            ..r.clone()
        })
        .collect();

    let named = chosen(&records);
    let plain = chosen(&unnamed);
    assert!(named.batches.is_some());
    assert_eq!(plain.batches, None);
    // The check reports; it does not move the thresholds.
    assert_eq!(named.key, plain.key);
    assert_eq!(named.memory, plain.memory);
}

#[test]
fn a_record_that_names_no_batch_is_a_batch_of_its_own_and_the_check_repeats() {
    let mut records = batched(100, 4);
    for record in records.iter_mut().skip(1).step_by(2) {
        record.batch = None;
    }

    let check = chosen(&records)
        .batches
        .expect("some records name their batch");
    // Each batch keeps two of its four records named, and the 200 unnamed ones count once each.
    assert_eq!(check.batches, 100 + 200);
    assert_eq!(chosen(&records).batches, Some(check));
}
