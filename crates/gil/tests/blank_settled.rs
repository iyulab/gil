//! A blank decided value is not a settled value, and a `settled_at` that cannot be read says what it takes.
//!
//! A table exported with empty cells reads as a stream of records whose decided attribute is the empty text. Taken as a
//! value, it is learned as one more team, the thresholds stop being chosen, and nothing says why. A record names the
//! blank where it sits instead, and a person who has settled nothing leaves the attribute out.

use std::collections::BTreeMap;

use gil::key_memory::KeyMemory;
use gil::schema::{Attribute, AttributeRole, Record, Schema, Settlement};

fn schema() -> Schema {
    Schema::new(
        "ticket",
        vec![
            Attribute::new("summary", AttributeRole::Observed),
            {
                let mut topics = Attribute::new("topics", AttributeRole::Decided);
                topics.multiple = true;
                topics
            },
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
}

fn record(values: &[(&str, &str)], sets: &[(&str, &[&str])]) -> Record {
    Record {
        id: "t1".into(),
        values: values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        sets: sets
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.iter().map(|e| (*e).to_owned()).collect()))
            .collect(),
        settled_at: 5,
        ..Record::default()
    }
}

#[test]
fn a_blank_decided_value_is_refused_where_it_sits() {
    let schema = schema();
    for blank in ["", "  ", "\t"] {
        let error = schema
            .check_record(&record(&[("team", blank)], &[]))
            .unwrap_err();
        assert!(error.contains("'team'"), "{error}");
        assert!(error.contains("empty"), "{error}");
        assert!(error.contains("leave it out"), "{error}");
    }
}

#[test]
fn a_blank_element_of_a_decided_set_is_refused() {
    let error = schema()
        .check_record(&record(&[], &[("topics", &["nlp", ""])]))
        .unwrap_err();
    assert!(
        error.contains("'topics'") && error.contains("empty"),
        "{error}"
    );
}

#[test]
fn blank_observed_values_and_filled_decided_values_pass() {
    let schema = schema();
    assert!(
        schema
            .check_record(&record(
                &[("summary", ""), ("team", "network")],
                &[("topics", &["nlp"])]
            ))
            .is_ok()
    );
    // A record that settled nothing leaves the decided attribute out.
    assert!(
        schema
            .check_record(&record(&[("summary", "vpn")], &[]))
            .is_ok()
    );
}

#[test]
#[should_panic(expected = "record 't1': 'team' is settled as an empty text")]
fn a_record_with_a_blank_decided_value_is_not_put_into_memory() {
    let schema = schema();
    KeyMemory::default().put(&schema, &record(&[("team", "")], &[]));
}

#[test]
fn a_blank_settlement_is_refused_before_it_is_taken() {
    let schema = schema();
    let open = gil::open_record::OpenRecord::new(schema, "t2", 5);
    let error = open
        .check_settle("team", &Settlement::enter(""))
        .unwrap_err();
    assert!(error.to_string().contains("empty"), "{error}");
}

#[test]
fn placement_of_a_query_is_unchanged_by_a_blank_decided_value() {
    // `check_placement` serves the questions a host asks mid-form, where an unfilled field is ordinary.
    let schema = schema();
    let values = BTreeMap::from([("team".to_owned(), String::new())]);
    assert!(schema.check_placement(&values, &BTreeMap::new()).is_ok());
}

#[cfg(feature = "serde")]
mod time {
    use gil::schema::Record;
    use serde_json::json;

    fn error_of(settled_at: serde_json::Value) -> String {
        serde_json::from_value::<Record>(json!({ "id": "t3", "settled_at": settled_at }))
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn a_settled_at_that_is_not_a_time_says_what_it_takes() {
        for found in [json!(null), json!(true), json!(1.5), json!([1])] {
            let error = error_of(found);
            assert!(
                error.contains("ticks") && error.contains("yyyy-MM-ddTHH:mm:ss"),
                "{error}"
            );
            assert!(!error.contains("untagged enum"), "{error}");
        }
        let text = error_of(json!("yesterday"));
        assert!(
            text.contains("'yesterday'") && text.contains("yyyy-MM-ddTHH:mm:ss"),
            "{text}"
        );
    }

    #[test]
    fn ticks_and_rfc_3339_times_still_read() {
        let ticks: Record = serde_json::from_value(json!({ "id": "a", "settled_at": 5 })).unwrap();
        let time: Record = serde_json::from_value(
            json!({ "id": "b", "settled_at": "1970-01-01T00:00:00.0000005Z" }),
        )
        .unwrap();
        assert_eq!((ticks.settled_at, time.settled_at), (5, 5));
    }
}
