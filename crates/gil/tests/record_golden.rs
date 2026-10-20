//! The decision record golden (`tests/fixtures/core/records-out.jsonl`), written by the .NET implementation from the
//! replay its layers threshold choice rests on: for the layers replay with documents settled again (`layers.jsonl`), every
//! document asked about, in settlement order, with what the key layer and the similar document vote made of it whatever
//! the thresholds and what it was settled as — the same here, every number to the bit. The reference replay does not
//! keep the best value itself; where it says the best value was right, the value here must be the settled one.

mod common;

use common::{CHOSEN, COUNTED, doubles, fixture, form, lines};
use gil::decision_record::DecisionRecord;
use gil::field_memory::FieldMemory;
use gil::form::Document;
use gil::lexical_memory::LexicalMemory;
use gil::threshold_selection::{
    Choice, select_coarse, select_coarse_with, select_layers, select_layers_with,
};
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
        "coarse": r.coarse.as_ref().map(|c| json!([
            c.value_answered,
            c.keys.as_ref().map(|k| json!([k.strength, k.correct])),
            c.vote.as_ref().map(|v| json!([v.score, v.correct, v.nearest, v.voted])),
        ])),
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
    let expected: Vec<Value> = lines(&fixture("records-out.jsonl"))
        .filter(|l| l["kind"] == "record")
        .collect();
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
/// document candidate; the key look is the key layer's best value, the first value it offers; the coarse look, for the
/// field with a coarse level, is where the class offered comes from — the keys' class or the vote's — and when no class
/// is offered though no layer answered, neither backed one or a value to catch was settled under the keys.
#[test]
fn suggestions_hand_over_records_that_agree_with_them() {
    let configured = form(&fixture("golden.jsonl"), |_| false);
    let [asked, answered, keyed, voted, looked, offered, withheld] = suggest_and_check(&configured);
    assert!(
        asked > 0 && answered > 0 && keyed > 0 && voted > 0 && looked > 0 && offered > 0,
        "{asked} {answered} {keyed} {voted} {looked} {offered} {withheld}"
    );
    // As configured the code's value layers never answer; let its key layer answer every key it finds, and the class is
    // withheld wherever a value is answered.
    let mut eager = configured.clone();
    let code = eager
        .fields
        .iter_mut()
        .find(|f| f.name == "code")
        .expect("code");
    code.key_threshold = Some(0.0);
    let [.., withheld] = suggest_and_check(&eager);
    assert!(withheld > 0, "{withheld}");
}

/// Asks every judged field of each later record as the answer-side test describes and checks each record against its
/// suggestion — the counts of lookups asked, answered, with a key look, with a vote, with a coarse look, with a class
/// offered, and with a class withheld because a value was answered.
fn suggest_and_check(form: &gil::form::Form) -> [usize; 7] {
    use gil::field_memory::Lookup;
    use gil::form::Role;
    use gil::form_resolver::{FormResolver, Source};
    use std::collections::BTreeMap;

    let documents: Vec<Document> = lines(&fixture("records.jsonl"))
        .map(|r| common::document(&r))
        .collect();
    let mut resolver = FormResolver::new(FieldMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(form, &documents[..CHOSEN]);
    let (mut asked, mut answered, mut keyed, mut voted) = (0, 0, 0, 0);
    let (mut looked, mut offered, mut withheld) = (0, 0, 0);
    for document in &documents[CHOSEN..] {
        let observed: BTreeMap<String, String> = document
            .values
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        let none = BTreeMap::new();
        for expected in resolver.suggest(form, &document.id, &observed, &none) {
            let q = Lookup::new(form, &expected.field, &observed)
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
            match (&definition.coarse, &record.coarse) {
                (None, None) => {}
                (Some(_), Some(look)) => {
                    assert_eq!(look.value_answered, first.is_some());
                    let keys = look.keys.as_ref();
                    let vote = look.vote.as_ref();
                    assert!(keys.is_none_or(|k| k.correct.is_none()));
                    assert!(vote.is_none_or(|v| v.correct.is_none()));
                    match &expected.coarse {
                        Some(c) if c.source == Source::SettledFieldMemory => {
                            let k = keys.expect("the keys' class");
                            assert_eq!(
                                (c.value.as_str(), c.score, c.evidence.as_deref()),
                                (k.value.as_str(), k.strength, Some(k.key.as_str()))
                            );
                            offered += 1;
                        }
                        Some(c) => {
                            assert_eq!(c.source, Source::SimilarDocument);
                            let v = vote.expect("the vote's class");
                            assert_eq!(
                                (c.value.as_str(), c.score, c.evidence.as_deref()),
                                (v.value.as_str(), v.score, Some(v.document.as_str()))
                            );
                            offered += 1;
                        }
                        None if look.value_answered => withheld += 1,
                        None => assert!(
                            keys.is_some_and(|k| k.caught) || keys.is_none() && vote.is_none()
                        ),
                    }
                    looked += 1;
                }
                (level, look) => panic!(
                    "coarse look {:?} for a field with a coarse level {:?}",
                    look.is_some(),
                    level.is_some()
                ),
            }
            asked += 1;
        }

        resolver.rebuild(form, std::slice::from_ref(document));
    }

    [asked, answered, keyed, voted, looked, offered, withheld]
}

/// The coarse replay's records (kind `coarse_record`): the golden's coarse level replay of the ticket's code on the first
/// records, with the form as configured when the level was chosen — each lookup with the class the keys back and the class
/// the similar documents vote for, whatever the level's thresholds, and whether the value layers answered.
#[test]
fn the_coarse_replay_writes_the_golden_decision_records() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| common::document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let is_coarse = |l: &Value| l["kind"] == "coarse" && l["field"] == "code";
    let form = form(&golden, is_coarse);
    let expected: Vec<Value> = lines(&fixture("records-out.jsonl"))
        .filter(|l| l["kind"] == "coarse_record")
        .collect();
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    let mut records = Vec::new();
    let coarse = select_coarse_with(
        &mut FieldMemory::default(),
        &mut LexicalMemory::default(),
        &form,
        "code",
        &early,
        choice,
        &mut |r| records.push(r.clone()),
    );
    assert_eq!(
        coarse,
        select_coarse(
            &mut FieldMemory::default(),
            &mut LexicalMemory::default(),
            &form,
            "code",
            &early,
            choice,
        )
    );
    assert_eq!(records.len(), expected.len());
    let level = form.field("code").coarse.as_ref().expect("a coarse level");
    for (at, (record, expected)) in records.iter().zip(expected).enumerate() {
        let mut actual = line(record);
        actual["kind"] = json!("coarse_record");
        assert_eq!(
            doubles(actual),
            doubles(expected),
            "coarse record line {}",
            at + 1
        );
        // The reference keeps no class names; a class it calls right is the settled value's.
        let look = record.coarse.as_ref().expect("a coarse replay's record");
        let class = level.of(record.settled.as_deref().expect("settled"));
        for (value, correct) in look
            .keys
            .iter()
            .map(|k| (&k.value, k.correct))
            .chain(look.vote.iter().map(|v| (&v.value, v.correct)))
        {
            assert_eq!(correct == Some(true), *value == class);
        }
    }
}
