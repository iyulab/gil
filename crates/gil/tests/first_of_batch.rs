//! The first record of a batch answered at thresholds of its own (experimental).

use std::collections::BTreeMap;

use gil::decider::Decider;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::schema::{Attribute, AttributeRole, FirstOfBatch, Record, Schema};

fn decider() -> Decider {
    Decider::new(KeyMemory::default(), Some(LexicalMemory::default()))
}

/// A schema whose key layer trusts nothing for most records and everything for the first of a batch.
fn schema() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.first_of_batch = Some(FirstOfBatch {
        key_threshold: Some(0.0),
        ..FirstOfBatch::default()
    });
    Schema::new(
        "ticket",
        vec![Attribute::new("product", AttributeRole::Observed), team],
    )
    .expect("a valid schema")
}

fn record(id: &str, batch: Option<&str>) -> Record {
    Record {
        id: id.into(),
        values: BTreeMap::from([
            ("product".into(), "router".into()),
            ("team".into(), "network".into()),
        ]),
        settled_at: 1_000,
        batch: batch.map(str::to_owned),
        ..Record::default()
    }
}

fn answered(open: &mut OpenRecord, decider: &mut Decider) -> bool {
    let answers = open.observe(decider, "product", Some("router"));
    answers
        .iter()
        .find(|a| a.attribute == "team")
        .expect("team is answered")
        .answered()
}

#[test]
fn a_batch_is_known_once_a_record_of_it_is_settled() {
    let mut decider = decider();
    assert!(decider.is_first_of_batch(Some("b1"), None));
    decider.rebuild(&schema(), &[record("r1", Some("b1"))]);
    assert!(!decider.is_first_of_batch(Some("b1"), None));
    // The record asked about is not its own batch mate.
    assert!(decider.is_first_of_batch(Some("b1"), Some("r1")));
    // A record that names no batch is always the first of its own.
    assert!(decider.is_first_of_batch(None, None));

    // Settled again in another batch, it leaves the first.
    decider.rebuild(&schema(), &[record("r1", Some("b2"))]);
    assert!(decider.is_first_of_batch(Some("b1"), None));
    assert!(!decider.is_first_of_batch(Some("b2"), None));
    decider.remove(&schema(), "r1");
    assert!(decider.is_first_of_batch(Some("b2"), None));
}

#[test]
fn an_open_record_answers_as_the_first_of_its_batch_until_a_mate_is_settled() {
    let mut decider = decider();
    decider.rebuild(&schema(), &[record("r1", Some("b1"))]);

    let mut first = OpenRecord::new(schema(), "r2", 2_000).with_batch("b2");
    assert!(first.is_first_of_batch(&decider));
    assert!(
        answered(&mut first, &mut decider),
        "the first record's threshold trusts"
    );

    let mut later = OpenRecord::new(schema(), "r3", 2_000).with_batch("b1");
    assert!(!later.is_first_of_batch(&decider));
    assert!(
        !answered(&mut later, &mut decider),
        "a later record's threshold trusts none"
    );

    let mut alone = OpenRecord::new(schema(), "r4", 2_000);
    assert!(alone.is_first_of_batch(&decider));
    assert!(answered(&mut alone, &mut decider));

    // The record carries its batch into what the host saves, and back.
    let saved = first.snapshot();
    assert_eq!(saved.batch.as_deref(), Some("b2"));
    assert_eq!(OpenRecord::reopen(schema(), &saved).batch(), Some("b2"));
}

#[test]
fn a_schema_without_thresholds_for_first_records_answers_every_record_alike() {
    let plain = Schema::new(
        "ticket",
        vec![
            Attribute::new("product", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema");
    let mut decider = decider();
    decider.rebuild(&plain, &[record("r1", Some("b1"))]);
    let open = OpenRecord::new(plain.clone(), "r2", 2_000).with_batch("b2");
    assert!(!open.is_first_of_batch(&decider));
    assert!(!plain.has_first_of_batch());
}

#[test]
fn only_a_decided_attribute_with_one_value_takes_thresholds_for_first_records() {
    let mut product = Attribute::new("product", AttributeRole::Observed);
    product.first_of_batch = Some(FirstOfBatch::default());
    let error = Schema::new(
        "ticket",
        vec![product, Attribute::new("team", AttributeRole::Decided)],
    )
    .expect_err("an observed attribute takes none");
    assert!(error.contains("first record of a batch"), "{error}");

    let view = schema().for_first_of_batch();
    assert_eq!(view.attribute("team").key_threshold, Some(0.0));
}
