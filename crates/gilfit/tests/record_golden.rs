//! The decision record golden (`tests/fixtures/core/records-out.jsonl`), written by the .NET implementation from the
//! replay its layers threshold choice rests on: for the layers replay with documents settled again (`layers.jsonl`), every
//! document asked about, in settlement order, with what the key layer and the similar document vote made of it whatever
//! the thresholds and what it was settled as — the same here, every number to the bit. The reference replay does not
//! keep the best value itself; where it says the best value was right, the value here must be the settled one.

mod common;

use common::{CHOSEN, COUNTED, doubles, fixture, form, lines};
use gilfit::decision_record::DecisionRecord;
use gilfit::field_memory::FieldMemory;
use gilfit::form::Document;
use gilfit::lexical_memory::LexicalMemory;
use gilfit::threshold_selection::{Choice, select_layers, select_layers_with};
use serde_json::{Value, json};

fn line(r: &DecisionRecord) -> Value {
    json!({
        "kind": "record",
        "field": r.field,
        "record": r.record,
        "order": r.order.expect("a replay's order"),
        "weight": r.weight,
        "key": r.key.as_ref().map(|k| json!([k.strength, k.correct, k.key, k.caught])),
        "similar": r.similar.as_ref().map(|s| json!([s.score, s.correct, s.nearest, s.voted])),
        "settled": r.settled,
        "settlement": r.settlement.map(|k| format!("{k:?}")),
        "probability": r.probability,
    })
}

#[test]
fn the_layers_replay_writes_the_golden_decision_records() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| common::document(&r))
        .collect();
    let last = early[CHOSEN - 1].settled_at;
    let mut again = early.clone();
    for (i, document) in early.iter().take(40).enumerate() {
        let mut copy = document.clone();
        if i % 2 == 1 {
            copy.id = format!("{}-copy", document.id);
        }
        copy.settled_at = last + (i as i64 + 1) * 60 * 10_000_000;
        again.push(copy);
    }

    let golden = fixture("golden.jsonl");
    let expected: Vec<Value> = lines(&fixture("records-out.jsonl")).collect();
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    let mut at = 0;
    for field in ["team", "code"] {
        let form = form(&golden, |l| l["kind"] == "layers" && l["field"] == field);
        let mut records = Vec::new();
        let layers = select_layers_with(
            &mut FieldMemory::default(),
            &mut LexicalMemory::default(),
            &form,
            field,
            &again,
            choice,
            &mut |r| records.push(r.clone()),
        );
        // Writing records changes nothing the choice comes to.
        assert_eq!(
            layers,
            select_layers(
                &mut FieldMemory::default(),
                &mut LexicalMemory::default(),
                &form,
                field,
                &again,
                choice,
            ),
            "field {field}"
        );

        for record in &records {
            assert_eq!(
                doubles(line(record)),
                doubles(expected[at].clone()),
                "record line {}",
                at + 1
            );
            assert!(record.answered.is_none(), "a replay answers nothing");
            let settled = record.settled.as_deref();
            if let Some(key) = &record.key {
                assert_eq!(
                    key.correct == Some(true),
                    Some(key.value.as_str()) == settled
                );
            }
            if let Some(similar) = &record.similar {
                assert_eq!(
                    similar.correct == Some(true),
                    Some(similar.value.as_str()) == settled
                );
            }
            at += 1;
        }
    }

    assert_eq!(at, expected.len());
}

/// The answer side: asked about every judged field of each later record of the form path's golden, as the decision golden
/// asks, `suggest_one_with` makes the very suggestion `suggest` makes and hands over one record that agrees with it — what
/// it answered is its first candidate when trusted, at the field's threshold; the similar document look is the similar
/// document candidate; the key look is the key layer's best value, the first value it offers.
#[test]
fn suggestions_hand_over_records_that_agree_with_them() {
    use gilfit::field_memory::Lookup;
    use gilfit::form::Role;
    use gilfit::form_resolver::{FormResolver, Source};
    use std::collections::BTreeMap;

    let documents: Vec<Document> = lines(&fixture("records.jsonl"))
        .map(|r| common::document(&r))
        .collect();
    let form = form(&fixture("golden.jsonl"), |_| false);
    let mut resolver = FormResolver::new(FieldMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&form, &documents[..CHOSEN]);
    let (mut asked, mut answered, mut keyed, mut voted) = (0, 0, 0, 0);
    for document in &documents[CHOSEN..] {
        let observed: BTreeMap<String, String> = document
            .values
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        let none = BTreeMap::new();
        for expected in resolver.suggest(&form, &document.id, &observed, &none) {
            let q = Lookup::new(&form, &expected.field, &observed)
                .sets(&none)
                .excluding(Some(&document.id));
            let mut records = Vec::new();
            let actual = resolver.suggest_one_with(&q, &mut |r| records.push(r.clone()));
            assert_eq!(actual, expected);
            let [record] = records.as_slice() else {
                panic!("one record per lookup, got {}", records.len())
            };
            let definition = form.field(&expected.field);
            assert_eq!(
                (record.record.as_str(), record.field.as_str()),
                (document.id.as_str(), expected.field.as_str())
            );
            assert_eq!((record.order, record.settled.as_ref()), (None, None));

            let first = expected.candidates.first().filter(|c| c.trusted);
            assert_eq!(
                record
                    .answered
                    .as_ref()
                    .map(|a| (a.layer, a.value.as_str())),
                first.map(|c| (c.source, c.value.as_str()))
            );
            if let Some(a) = &record.answered {
                let threshold = match a.layer {
                    Source::SettledFieldMemory => definition.key_threshold,
                    _ => definition.memory_threshold,
                };
                assert_eq!(a.threshold, threshold);
                answered += 1;
            }
            if let Some(similar) = &record.similar {
                assert_eq!(similar.correct, None);
                if let Some(c) = expected
                    .candidates
                    .iter()
                    .find(|c| c.source == Source::SimilarDocument)
                {
                    assert_eq!(
                        (c.value.as_str(), c.score, c.evidence.as_deref()),
                        (
                            similar.value.as_str(),
                            similar.score,
                            Some(similar.document.as_str())
                        )
                    );
                }
                voted += 1;
            }
            if let Some(key) = &record.key {
                assert_eq!(key.correct, None);
                let best = expected
                    .candidates
                    .iter()
                    .find(|c| c.source == Source::SettledFieldMemory && c.evidence.is_some());
                if let Some(c) = best.filter(|c| c.value == key.value) {
                    assert_eq!(
                        (c.score, c.evidence.as_deref()),
                        (key.strength, Some(key.key.as_str()))
                    );
                }
                keyed += 1;
            }
            asked += 1;
        }

        resolver.rebuild(&form, std::slice::from_ref(document));
    }

    assert!(
        asked > 0 && answered > 0 && keyed > 0 && voted > 0,
        "{asked} {answered} {keyed} {voted}"
    );
}
