//! A judge outside Gil that offers a blank value has a defect worth telling of; it is not an answer.

use gil::decider::{Decider, ExternalCandidate};
use gil::key_memory::KeyMemory;
use gil::open_record::OpenRecord;
use gil::schema::{Attribute, AttributeRole, Schema};

fn offered(value: &str) -> ExternalCandidate {
    ExternalCandidate {
        value: value.to_owned(),
        probability: 0.9,
        trusted: true,
    }
}

fn open_record() -> (Decider, OpenRecord) {
    let schema = Schema::new(
        "ticket",
        vec![
            Attribute::new("summary", AttributeRole::Observed),
            Attribute::new("note", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema");
    let decider = Decider::new(KeyMemory::default(), None);
    let open = OpenRecord::new(schema, "t1", 1);
    (decider, open)
}

#[test]
fn a_value_that_is_not_blank_is_offered_as_before() {
    let (decider, open) = open_record();
    let answer = open.offer_external(&decider, "note", &[offered("high")]);
    assert!(answer.answered());
}

#[test]
#[should_panic(expected = "offered a blank value for 'note'")]
fn an_empty_value_is_refused() {
    let (decider, open) = open_record();
    open.offer_external(&decider, "note", &[offered("")]);
}

#[test]
#[should_panic(expected = "offered a blank value for 'note'")]
fn a_white_space_value_is_refused_wherever_it_ranks() {
    let (decider, open) = open_record();
    open.offer_external(&decider, "note", &[offered("high"), offered("  ")]);
}
