//! The decision log golden (`tests/fixtures/core/records-out.jsonl`), from the replay
//! its layers threshold choice rests on: for the layers replay with records settled again (`layers.jsonl`), every
//! record asked about, in settlement order, with what the key layer and the similar record vote made of it whatever
//! the thresholds and what it was settled as — the same here, every number to the bit. The reference replay does not
//! keep the best value itself; where it says the best value was right, the value here must be the settled one.

mod common;

use common::{CHOSEN, COUNTED, Golden, fixture, lines, schema};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, select_coarse, select_coarse_with, select_layers, select_layers_with};
use gil::schema::Record;
use serde_json::{Value, json};

fn line(r: &DecisionRecord) -> Value {
    json!({
        "kind": "record",
        "field": r.attribute,
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
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| common::to_record(&r))
        .collect();
    let last = early[CHOSEN - 1].settled_at;
    let mut again = early.clone();
    for (i, record) in early.iter().take(40).enumerate() {
        let mut copy = record.clone();
        if i % 2 == 1 {
            copy.id = format!("{}-copy", record.id);
        }
        copy.settled_at = last + (i as i64 + 1) * 60 * 10_000_000;
        again.push(copy);
    }

    let golden = fixture("golden.jsonl");
    let mut golden_out = Golden::part("records-out.jsonl", &["record"]);
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    for attribute in ["team", "code"] {
        let schema = schema(&golden, |l| {
            l["kind"] == "layers" && l["field"] == attribute
        });
        let mut logs = Vec::new();
        let layers = select_layers_with(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &again,
            choice,
            &mut |r| logs.push(r.clone()),
        );
        // Writing logs changes nothing the choice comes to.
        assert_eq!(
            layers,
            select_layers(
                &mut KeyMemory::default(),
                &mut LexicalMemory::default(),
                &schema,
                attribute,
                &again,
                choice,
            ),
            "attribute {attribute}"
        );

        for log in &logs {
            golden_out.check(line(log), format!("{attribute} record {}", log.record));
            assert!(log.answered.is_none(), "a replay answers nothing");
            let settled = log.settled.as_deref();
            if let Some(key) = &log.key {
                assert_eq!(
                    key.correct == Some(true),
                    Some(key.value.as_str()) == settled
                );
            }
            if let Some(similar) = &log.similar {
                assert_eq!(
                    similar.correct == Some(true),
                    Some(similar.value.as_str()) == settled
                );
            }
        }
    }

    golden_out.finish();
}

/// The answer side: asked about every decided attribute of each later log of the schema path's golden, as the decision golden
/// asks, `answer_one_with` makes the very suggestion `answer` makes and hands over one log that agrees with it — what
/// it answered is its first candidate when trusted, at the attribute's threshold; the similar record look is the similar
/// record candidate; the key look is the key layer's best value, the first value it offers; the coarse look, for the
/// attribute with a coarse level, is where the class offered comes from — the keys' class or the vote's — and when no class
/// is offered though no layer answered, neither backed one or a value to catch was settled under the keys.
#[test]
fn suggestions_hand_over_records_that_agree_with_them() {
    let configured = schema(&fixture("golden.jsonl"), |_| false);
    let [asked, answered, keyed, voted, looked, offered, withheld] = suggest_and_check(&configured);
    assert!(
        asked > 0 && answered > 0 && keyed > 0 && voted > 0 && looked > 0 && offered > 0,
        "{asked} {answered} {keyed} {voted} {looked} {offered} {withheld}"
    );
    // As configured the code's value layers never answer; let its key layer answer every key it finds, and the class is
    // withheld wherever a value is answered.
    let mut attributes = configured.attributes().to_vec();
    let code = attributes
        .iter_mut()
        .find(|f| f.name == "code")
        .expect("code");
    code.key_threshold = Some(0.0);
    let eager = gil::schema::Schema::new(configured.name(), attributes).expect("a valid schema");
    let [.., withheld] = suggest_and_check(&eager);
    assert!(withheld > 0, "{withheld}");
}

/// Asks every decided attribute of each later log as the answer-side test describes and checks each log against its
/// suggestion — the counts of lookups asked, answered, with a key look, with a vote, with a coarse look, with a class
/// offered, and with a class withheld because a value was answered.
fn suggest_and_check(schema: &gil::schema::Schema) -> [usize; 7] {
    use gil::decider::{Decider, Source};
    use gil::key_memory::Lookup;
    use gil::schema::AttributeRole;
    use std::collections::BTreeMap;

    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| common::to_record(&r))
        .collect();
    let mut resolver = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(schema, &records[..CHOSEN]);
    let (mut asked, mut answered, mut keyed, mut voted) = (0, 0, 0, 0);
    let (mut looked, mut offered, mut withheld) = (0, 0, 0);
    for record in &records[CHOSEN..] {
        let observed: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(attribute, _)| schema.attribute(attribute).role == AttributeRole::Observed)
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect();
        let none = BTreeMap::new();
        for expected in resolver.answer(schema, &record.id, &observed, &none) {
            let q = Lookup::new(schema, &expected.attribute, &observed)
                .sets(&none)
                .excluding(Some(&record.id));
            let mut logs = Vec::new();
            let actual = resolver.answer_one_with(&q, &mut |r| logs.push(r.clone()));
            assert_eq!(actual, expected);
            let [log] = logs.as_slice() else {
                panic!("one record per lookup, got {}", logs.len())
            };
            let definition = schema.attribute(&expected.attribute);
            assert_eq!(
                (log.record.as_str(), log.attribute.as_str()),
                (record.id.as_str(), expected.attribute.as_str())
            );
            assert_eq!((log.order, log.settled.as_ref()), (None, None));

            let first = expected.candidates.first().filter(|c| c.trusted);
            assert_eq!(
                log.answered.as_ref().map(|a| (a.layer, a.value.as_str())),
                first.map(|c| (c.source, c.value.as_str()))
            );
            if let Some(a) = &log.answered {
                let threshold = match a.layer {
                    Source::KeyMemory => definition.key_threshold,
                    _ => definition.memory_threshold,
                };
                assert_eq!(a.threshold, threshold);
                answered += 1;
            }
            if let Some(similar) = &log.similar {
                assert_eq!(similar.correct, None);
                if let Some(c) = expected
                    .candidates
                    .iter()
                    .find(|c| c.source == Source::SimilarRecord)
                {
                    assert_eq!(
                        (
                            c.value.as_str(),
                            c.score,
                            c.evidence.as_ref().map(ToString::to_string).as_deref()
                        ),
                        (
                            similar.value.as_str(),
                            similar.score,
                            Some(similar.record.as_str())
                        )
                    );
                }
                voted += 1;
            }
            if let Some(key) = &log.key {
                assert_eq!(key.correct, None);
                let best = expected
                    .candidates
                    .iter()
                    .find(|c| c.source == Source::KeyMemory && c.evidence.is_some());
                if let Some(c) = best.filter(|c| c.value == key.value) {
                    assert_eq!(
                        (
                            c.score,
                            c.evidence.as_ref().map(ToString::to_string).as_deref()
                        ),
                        (key.strength, Some(key.key.as_str()))
                    );
                }
                keyed += 1;
            }
            match (&definition.coarse, &log.coarse) {
                (None, None) => {}
                (Some(_), Some(look)) => {
                    assert_eq!(look.value_answered, first.is_some());
                    let keys = look.keys.as_ref();
                    let vote = look.vote.as_ref();
                    assert!(keys.is_none_or(|k| k.correct.is_none()));
                    assert!(vote.is_none_or(|v| v.correct.is_none()));
                    match &expected.coarse {
                        Some(c) if c.source == Source::KeyMemory => {
                            let k = keys.expect("the keys' class");
                            assert_eq!(
                                (
                                    c.value.as_str(),
                                    c.score,
                                    c.evidence.as_ref().map(ToString::to_string).as_deref()
                                ),
                                (k.value.as_str(), k.strength, Some(k.key.as_str()))
                            );
                            offered += 1;
                        }
                        Some(c) => {
                            assert_eq!(c.source, Source::SimilarRecord);
                            let v = vote.expect("the vote's class");
                            assert_eq!(
                                (
                                    c.value.as_str(),
                                    c.score,
                                    c.evidence.as_ref().map(ToString::to_string).as_deref()
                                ),
                                (v.value.as_str(), v.score, Some(v.record.as_str()))
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

        resolver.rebuild(schema, std::slice::from_ref(record));
    }

    [asked, answered, keyed, voted, looked, offered, withheld]
}

/// The coarse replay's logs (kind `coarse_record`): the golden's coarse level replay of the ticket's code on the first
/// logs, with the schema as configured when the level was chosen — each lookup with the class the keys back and the class
/// the similar records vote for, whatever the level's thresholds, and whether the value layers answered.
#[test]
fn the_coarse_replay_writes_the_golden_decision_records() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| common::to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let is_coarse = |l: &Value| l["kind"] == "coarse" && l["field"] == "code";
    let schema = schema(&golden, is_coarse);
    let mut golden_out = Golden::part("records-out.jsonl", &["coarse_record"]);
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    let mut logs = Vec::new();
    let coarse = select_coarse_with(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "code",
        &early,
        choice,
        &mut |r| logs.push(r.clone()),
    );
    assert_eq!(
        coarse,
        select_coarse(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            "code",
            &early,
            choice,
        )
    );
    let level = schema
        .attribute("code")
        .coarse
        .as_ref()
        .expect("a coarse level");
    for log in &logs {
        let mut actual = line(log);
        actual["kind"] = json!("coarse_record");
        golden_out.check(actual, format!("coarse record {}", log.record));
        // The reference keeps no class names; a class it calls right is the settled value's.
        let look = log.coarse.as_ref().expect("a coarse replay's record");
        let class = level.of(log.settled.as_deref().expect("settled"));
        for (value, correct) in look
            .keys
            .iter()
            .map(|k| (&k.value, k.correct))
            .chain(look.vote.iter().map(|v| (&v.value, v.correct)))
        {
            assert_eq!(correct == Some(true), *value == class);
        }
    }
    golden_out.finish();
}
