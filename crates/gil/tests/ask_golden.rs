//! The ask planner's golden (`tests/fixtures/core/ask.jsonl`): with the settings the
//! schema path's golden chose and a resolver rebuilt from the first 200 records, a planner for each decided
//! single-valued attribute, asked for advice on every later record with some askable attributes left open — the product
//! every other record, the channel every third, the summary every fifth — before the record is put, and learning from
//! it after, must advise the same here: the answer's coarse class, and each open attribute's gain to the bit, whether to
//! ask and the threshold.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, Golden, fixture, lines, schema, to_record};
use gil::ask_planner::AskPlanner;
use gil::decider::Decider;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::schema::{AttributeRole, Record};
use serde_json::{Value, json};

const ASKABLE: [&str; 3] = ["product", "channel", "summary"];

fn open(attribute: &str, i: usize) -> bool {
    (attribute == "product" && i.is_multiple_of(2))
        || (attribute == "channel" && i % 3 == 1)
        || (attribute == "summary" && i.is_multiple_of(5))
}

#[test]
fn the_ask_planner_reproduces_its_golden() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let schema = schema(&fixture("golden.jsonl"), |_| false);
    let mut golden = Golden::open("ask.jsonl");

    let mut resolver = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&schema, &records[..CHOSEN]);
    let mut planners: Vec<(String, AskPlanner)> = schema
        .attributes()
        .iter()
        .filter(|a| a.role == AttributeRole::Decided && !a.multiple)
        .map(|a| {
            let planner = AskPlanner::new(&schema, &a.name, ASKABLE, 0.3).expect("a valid planner");
            (a.name.clone(), planner)
        })
        .collect();
    for (i, record) in records[CHOSEN..].iter().enumerate() {
        let values: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(attribute, _)| {
                schema.attribute(attribute).role == AttributeRole::Observed && !open(attribute, i)
            })
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect();
        for (attribute, planner) in &planners {
            let answer = resolver.answer_one(
                &schema,
                &record.id,
                attribute,
                &values,
                &BTreeMap::new(),
                None,
            );
            let advice: Vec<Value> = planner
                .advise(&answer, &values)
                .iter()
                .map(|a| json!([a.attribute, a.gain, a.ask, a.threshold]))
                .collect();
            let actual = json!({
                "kind": "ask",
                "doc": record.id,
                "field": attribute,
                "answered": answer.answered(),
                "coarse": answer.coarse.as_ref().map(|c| json!([c.value, c.trusted])),
                "advice": advice,
            });
            golden.check(actual, format!("{} {attribute}", record.id));
        }

        resolver.rebuild(&schema, std::slice::from_ref(record));
        for (_, planner) in &mut planners {
            planner.learn(&resolver, record);
        }
    }

    golden.finish();
}

#[test]
fn a_planner_refuses_what_the_reference_refuses() {
    let schema = schema(&fixture("golden.jsonl"), |_| false);
    assert!(AskPlanner::new(&schema, "team", ASKABLE, 0.0).is_err());
    assert!(AskPlanner::new(&schema, "team", ASKABLE, 1.5).is_err());
    assert!(AskPlanner::new(&schema, "tags", ASKABLE, 0.3).is_err());
    assert!(AskPlanner::new(&schema, "product", ASKABLE, 0.3).is_err());
    assert!(AskPlanner::new(&schema, "team", ["team"], 0.3).is_err());
    assert!(AskPlanner::new(&schema, "team", ["product", "product"], 0.3).is_err());
    assert!(AskPlanner::new(&schema, "team", Vec::<String>::new(), 0.3).is_err());
    assert!(AskPlanner::new(&schema, "team", ASKABLE, 1.0).is_ok());
}
