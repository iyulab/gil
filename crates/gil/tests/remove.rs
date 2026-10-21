//! Taking a record out of a decider: memories rebuilt from the schema path's first 200 logs, with every seventh then
//! removed, answer the later logs from the key memory as memories rebuilt without those records do, and never offer a
//! removed record as a similar one (the lexical memory's similarities follow its own refresh, so they are not held to a
//! rebuild's) — and the decision records an answer hands its sink come one per attribute, in the answers' order.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil::decider::{Decider, Evidence};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::schema::{AttributeRole, Record, Schema};

fn decider(schema: &Schema, records: &[Record], similar: bool) -> Decider {
    let mut d = Decider::new(KeyMemory::default(), similar.then(LexicalMemory::default));
    d.rebuild(schema, records);
    d
}

fn observed(schema: &Schema, record: &Record) -> BTreeMap<String, String> {
    record
        .values
        .iter()
        .filter(|(a, _)| schema.attribute(a).role == AttributeRole::Observed)
        .map(|(a, v)| (a.clone(), v.clone()))
        .collect()
}

#[test]
fn a_removed_record_leaves_the_memories_as_if_it_had_never_been_put() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let schema = schema(&fixture("golden.jsonl"), |_| false);
    let early = &records[..CHOSEN];
    let (removed, kept): (Vec<_>, Vec<_>) = early.iter().enumerate().partition(|(i, _)| i % 7 == 3);
    let kept: Vec<Record> = kept.into_iter().map(|(_, r)| r.clone()).collect();

    // Before they are taken out, the records to remove back later answers as similar records.
    let all = decider(&schema, early, true);
    let backed = records[CHOSEN..]
        .iter()
        .flat_map(|r| all.answer(&schema, &r.id, &observed(&schema, r), &BTreeMap::new()))
        .flat_map(|a| a.candidates)
        .filter(|c| {
            matches!(&c.evidence, Some(Evidence::Record { id, .. }) if removed.iter().any(|(_, r)| &r.id == id))
        })
        .count();
    assert!(backed > 0);

    for similar in [false, true] {
        let mut taken = decider(&schema, early, similar);
        for (_, record) in &removed {
            taken.remove(&schema, &record.id);
        }
        taken.remove(&schema, "never-put"); // nothing to take: no change
        let fresh = decider(&schema, &kept, similar);

        let mut compared = 0;
        for record in &records[CHOSEN..] {
            let values = observed(&schema, record);
            let a = taken.answer(&schema, &record.id, &values, &BTreeMap::new());
            if similar {
                for candidate in a.iter().flat_map(|a| &a.candidates) {
                    if let Some(Evidence::Record { id, .. }) = &candidate.evidence {
                        assert!(removed.iter().all(|(_, r)| &r.id != id), "{id} was removed");
                    }
                }
            } else {
                let b = fresh.answer(&schema, &record.id, &values, &BTreeMap::new());
                assert_eq!(a, b, "record {}", record.id);
            }
            compared += a.len();
        }
        assert!(compared > 100, "{compared}");
    }
}

#[test]
fn an_answer_hands_its_sink_one_decision_record_per_attribute() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let schema = schema(&fixture("golden.jsonl"), |_| false);
    let d = decider(&schema, &records[..CHOSEN], true);
    let record = &records[CHOSEN];
    let mut kept = Vec::new();
    let answers = d.answer_with(
        &schema,
        &record.id,
        &observed(&schema, record),
        &BTreeMap::new(),
        &mut |r| kept.push(r.clone()),
    );
    assert_eq!(kept.len(), answers.len());
    for (r, a) in kept.iter().zip(&answers) {
        assert_eq!(r.attribute, a.attribute);
        assert_eq!(r.record, record.id);
        assert_eq!(r.answered.is_some(), a.answered());
    }
    assert_eq!(
        answers,
        d.answer(
            &schema,
            &record.id,
            &observed(&schema, record),
            &BTreeMap::new()
        )
    );
}
