//! One attribute answered on its own and the order of an answer's layers, held to what the reference implementation's
//! resolver tests expect (its typed suggestion, field suggestion, key threshold, settlement origin and Hangul tests): typed
//! text narrows the key layer's values and is held to the threshold for that many characters, a value trusted with fewer
//! characters stays trusted while the text leads to it, a Hangul syllable still being composed leads to the values its
//! keystrokes begin; one attribute is answered as the whole record would answer it, and only while it is open; a key too
//! weak to answer is a guess that still keeps an external judgement out, and guesses come as a weaker key, then the most
//! frequent value, then the nearest record; a saved record is never its own evidence. Choosing thresholds replays every
//! record a person settled but counts only the kinds asked for, and neither asks about nor remembers a filled value; a key
//! that decides the attribute is chosen to answer, one that does not is not trusted on the strength of repeats.
//!
//! The reference drew its scenarios from `System.Random`; here a seeded SplitMix64 draws them, and only properties that
//! hold whatever the draws are kept. Not carried over: the reference's lookup counts (the core's memories are not
//! injected, so a test cannot count lookups), its list of similar records beside an answer (the core reports the record
//! a candidate rests on, not its neighbours) and its threshold choice for the similar record layer alone (the core chooses
//! it on the lookups the key layer leaves).

use std::collections::BTreeMap;

use gil::decider::{Answer, Decider, Source};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{Choice, select_key_threshold, select_layers, select_typed_key_thresholds};
use gil::schema::{Attribute, AttributePolicy, AttributeRole, Record, Schema, SettlementKind};

const MINUTE: i64 = 60 * 10_000_000;

/// A small deterministic generator (SplitMix64), so the records are the same on every run.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn decided(name: &str, edit: impl FnOnce(&mut Attribute)) -> Attribute {
    let mut attribute = Attribute::new(name, AttributeRole::Decided);
    edit(&mut attribute);
    attribute
}

fn observed(name: &str) -> Attribute {
    Attribute::new(name, AttributeRole::Observed)
}

fn schema(name: &str, attributes: Vec<Attribute>) -> Schema {
    Schema::new(name, attributes).expect("a valid schema")
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

/// Record `id`, settled `minute` minutes after the epoch.
fn record(id: &str, minute: i64, pairs: &[(&str, &str)]) -> Record {
    Record {
        id: id.into(),
        values: values(pairs),
        settled_at: minute * MINUTE,
        ..Record::default()
    }
}

fn decider(schema: &Schema, records: &[Record], similar: bool) -> Decider {
    let mut decider = Decider::new(KeyMemory::default(), similar.then(LexicalMemory::default));
    decider.rebuild(schema, records);
    decider
}

fn answer_one(
    decider: &Decider,
    schema: &Schema,
    record: &str,
    attribute: &str,
    pairs: &[(&str, &str)],
    typed: Option<&str>,
) -> Answer {
    decider.answer_one(
        schema,
        record,
        attribute,
        &values(pairs),
        &BTreeMap::new(),
        typed,
    )
}

fn only(decider: &Decider, schema: &Schema, record: &str, pairs: &[(&str, &str)]) -> Answer {
    let mut answers = decider.answer(schema, record, &values(pairs), &BTreeMap::new());
    assert_eq!(answers.len(), 1);
    answers.remove(0)
}

fn offered(answer: &Answer) -> Vec<&str> {
    answer.candidates.iter().map(|c| c.value.as_str()).collect()
}

fn choice(target_precision: f64, minimum_answered: usize) -> Choice<'static> {
    Choice {
        target_precision,
        minimum_answered,
        counting: None,
    }
}

// --- Typed text ------------------------------------------------------------------------------------------------------

fn typed_ticket(
    key_threshold: Option<f64>,
    typed: Option<Vec<Option<f64>>>,
    memory_threshold: Option<f64>,
) -> Schema {
    schema(
        "ticket",
        vec![
            observed("channel"),
            observed("summary"),
            decided("team", |t| {
                t.key_threshold = key_threshold;
                t.typed_key_thresholds = typed;
                t.memory_threshold = memory_threshold;
                t.depends_on = Some(vec!["channel".into()]);
            }),
        ],
    )
}

fn phone(number: i64, channel: &str, team: &str) -> Record {
    let summary = format!("ticket {number}");
    record(
        &format!("d{number}"),
        number,
        &[("channel", channel), ("summary", &summary), ("team", team)],
    )
}

/// Under channel=phone: "network" three times, "billing" twice, "backend" once.
fn phone_history() -> Vec<Record> {
    [
        "network", "network", "billing", "network", "billing", "backend",
    ]
    .iter()
    .zip(1..)
    .map(|(team, n)| phone(n, "phone", team))
    .collect()
}

const PHONE: &[(&str, &str)] = &[("channel", "phone"), ("summary", "new ticket")];

#[test]
fn typed_text_narrows_the_candidates_and_is_held_to_its_own_threshold() {
    let schema = typed_ticket(Some(0.9), Some(vec![Some(0.25)]), None);
    let decider = decider(&schema, &phone_history(), false);

    let untyped = only(&decider, &schema, "d9", PHONE);
    let typed = answer_one(&decider, &schema, "d9", "team", PHONE, Some("B"));

    assert_eq!(untyped.candidates[0].value, "network");
    assert!(!untyped.answered()); // network scores about 3/7 under channel=phone, below 0.9
    assert_eq!(offered(&typed), ["billing", "backend"]);
    assert!((typed.candidates[0].score - 2.0 / 7.0).abs() < 0.02);
    assert!(typed.answered()); // about 2/7 reaches the threshold for one typed character
    assert_eq!(typed.needs_external, None);
    assert_eq!(typed.coarse, None);
}

#[test]
fn a_value_trusted_with_fewer_characters_stays_trusted_while_the_text_leads_to_it() {
    // Only one typed character has a threshold. "billing", trusted at "b", is still the best value at "bi", "bil" and the
    // whole value, with the same score: typing on does not withdraw it.
    let schema = typed_ticket(Some(0.9), Some(vec![Some(0.25)]), None);
    let decider = decider(&schema, &phone_history(), false);

    let at_b = answer_one(&decider, &schema, "d9", "team", PHONE, Some("b"));
    for text in ["b", "bi", "BIL", "billing"] {
        let answer = answer_one(&decider, &schema, "d9", "team", PHONE, Some(text));
        assert_eq!(answer.candidates[0].value, "billing", "{text}");
        assert!(
            answer.answered(),
            "\"{text}\" still leads to the value trusted at \"b\""
        );
        assert_eq!(answer.candidates[0].score, at_b.candidates[0].score);
    }
}

#[test]
fn past_the_typed_thresholds_a_value_not_trusted_before_is_a_guess() {
    // At "ba" the best value is "backend", never the trusted value with fewer characters typed ("b" trusted "billing"),
    // and two characters have no threshold.
    let schema = typed_ticket(Some(0.9), Some(vec![Some(0.25)]), None);
    let decider = decider(&schema, &phone_history(), false);

    let answer = answer_one(&decider, &schema, "d9", "team", PHONE, Some("ba"));

    assert_eq!(offered(&answer), ["backend"]);
    assert!(!answer.answered());
}

#[test]
fn a_value_trusted_before_typing_stays_trusted_while_it_is_typed() {
    // network scores about 0.40 under channel=phone; no typed thresholds at all.
    let schema = typed_ticket(Some(0.35), None, None);
    let decider = decider(&schema, &phone_history(), false);

    let untyped = only(&decider, &schema, "d9", PHONE);
    let typing = answer_one(&decider, &schema, "d9", "team", PHONE, Some("netw"));
    let elsewhere = answer_one(&decider, &schema, "d9", "team", PHONE, Some("b"));

    assert!(untyped.answered());
    assert_eq!(typing.candidates[0].value, "network");
    assert!(typing.answered());
    assert!(!elsewhere.answered()); // "billing" was never trusted, and one character has no threshold
}

#[test]
fn while_typing_no_external_judgement_is_asked_for_even_where_no_memory_has_evidence() {
    let schema = typed_ticket(None, None, None);
    let decider = decider(&schema, &[], false);

    let untyped = only(&decider, &schema, "d9", PHONE);
    let typed = answer_one(&decider, &schema, "d9", "team", PHONE, Some("b"));

    assert!(untyped.needs_external.is_some());
    assert_eq!(typed.needs_external, None);
    assert!(typed.candidates.is_empty());
}

#[test]
fn a_composing_hangul_syllable_leads_to_the_value_it_is_on_the_way_to() {
    let schema = schema(
        "handover",
        vec![
            observed("team"),
            decided("owner", |o| o.typed_key_thresholds = Some(vec![Some(0.2)])),
        ],
    );
    let history: Vec<Record> = (1..=6)
        .map(|i| {
            let owner = if i % 2 == 0 {
                "박물관"
            } else {
                "김치찌개"
            };
            record(&format!("d{i}"), i, &[("team", "가"), ("owner", owner)])
        })
        .collect();
    let decider = decider(&schema, &history, false);

    // "바" is on the way to "박" — its final consonant is not typed yet.
    let composing = answer_one(
        &decider,
        &schema,
        "d9",
        "owner",
        &[("team", "가")],
        Some("바"),
    );

    assert_eq!(offered(&composing), ["박물관"]);
    assert!(composing.answered());
}

#[test]
#[should_panic(expected = "'channel' is not a decided attribute")]
fn text_is_typed_only_into_a_decided_attribute() {
    let schema = typed_ticket(Some(0.9), None, None);
    answer_one(
        &decider(&schema, &[], false),
        &schema,
        "d9",
        "channel",
        PHONE,
        Some("ph"),
    );
}

#[test]
#[should_panic(expected = "'team' already has a value")]
fn text_is_not_typed_into_an_attribute_with_a_value() {
    let schema = typed_ticket(Some(0.9), None, None);
    answer_one(
        &decider(&schema, &[], false),
        &schema,
        "d9",
        "team",
        &[("channel", "phone"), ("team", "network")],
        Some("n"),
    );
}

#[test]
fn typed_thresholds_are_chosen_on_the_records_typed_that_far() {
    // Under channel=phone "network" is settled every time but one in four. With the key threshold answering "network"
    // rightly, only the records settled otherwise are typed into — and those decide the threshold for one character, not
    // every record.
    let records: Vec<Record> = (1..=80)
        .map(|i| {
            let team = match (i % 4, i % 8) {
                (0, 0) => "backend",
                (0, _) => "billing",
                _ => "network",
            };
            phone(i, "phone", team)
        })
        .collect();
    let select = |key_threshold| {
        select_typed_key_thresholds(
            &mut KeyMemory::default(),
            &typed_ticket(key_threshold, None, None),
            "team",
            &records,
            choice(0.8, 5),
            2,
        )
    };

    let chosen = select(Some(0.5));
    let everything = select(None);

    assert_eq!(chosen.by_length.len(), 2);
    assert_eq!(chosen.thresholds().len(), 2);
    let not_network = records
        .iter()
        .filter(|r| r.values["team"] != "network")
        .count();
    assert_eq!(chosen.by_length[0].lookups, not_network); // "network" answered rightly with nothing typed
    assert_eq!(everything.by_length[0].lookups, records.len() - 1);
}

#[test]
fn typed_thresholds_leave_out_the_records_a_wrong_claim_still_leads_to() {
    // "network" is trusted before typing. On a "netops" record it is wrong, yet "n", "ne" and "net" still lead to it: the
    // person sees the same claim, not a new answer, so those lengths are not chosen on that record. From "neto" on the
    // typing has left it behind and the record counts again. "billing" leaves it at "b".
    let records: Vec<Record> = (1..=80)
        .map(|i| {
            let team = match (i % 4, i % 8) {
                (0, 0) => "netops",
                (0, _) => "billing",
                _ => "network",
            };
            phone(i, "phone", team)
        })
        .collect();

    let chosen = select_typed_key_thresholds(
        &mut KeyMemory::default(),
        &typed_ticket(Some(0.5), None, None),
        "team",
        &records,
        choice(0.8, 5),
        4,
    );

    let count = |team: &str| records.iter().filter(|r| r.values["team"] == team).count();
    assert_eq!(chosen.by_length[0].lookups, count("billing"));
    assert!(chosen.by_length[2].lookups <= count("billing"));
    assert!(chosen.by_length[3].lookups >= count("netops"));
}

// --- One attribute alone ---------------------------------------------------------------------------------------------

fn desk() -> Schema {
    schema(
        "ticket",
        vec![
            observed("channel"),
            observed("summary"),
            decided("team", |t| {
                t.key_threshold = Some(0.5);
                t.typed_key_thresholds = Some(vec![Some(0.25)]);
                t.memory_threshold = Some(0.1);
                t.depends_on = Some(vec!["channel".into(), "summary".into()]);
            }),
            decided("priority", |p| {
                p.memory_threshold = Some(0.1);
                p.depends_on = Some(vec!["summary".into()]);
            }),
            decided("internal", |i| i.policy = AttributePolicy::Off),
        ],
    )
}

fn desk_history() -> Vec<Record> {
    let desk = |n: i64, channel: &str, summary: &str, team: &str, priority: &str| {
        record(
            &format!("d{n}"),
            n,
            &[
                ("channel", channel),
                ("summary", summary),
                ("team", team),
                ("priority", priority),
            ],
        )
    };
    vec![
        desk(1, "phone", "card payment failed", "billing", "high"),
        desk(2, "phone", "router keeps dropping", "network", "low"),
        desk(3, "phone", "card payment failed again", "billing", "high"),
        desk(4, "email", "router lights off", "network", "low"),
    ]
}

const ASKED: &[(&str, &str)] = &[
    ("channel", "phone"),
    ("summary", "card payment failed twice"),
];

#[test]
fn one_attribute_is_answered_as_the_whole_record_would_answer_it() {
    let schema = desk();
    let decider = decider(&schema, &desk_history(), true);

    let all = decider.answer(&schema, "d9", &values(ASKED), &BTreeMap::new());
    assert_eq!(
        all.iter().map(|a| a.attribute.as_str()).collect::<Vec<_>>(),
        ["team", "priority"] // not the attribute that is off
    );
    for whole in &all {
        assert_eq!(
            &answer_one(&decider, &schema, "d9", &whole.attribute, ASKED, None),
            whole
        );
    }
}

#[test]
fn typing_into_an_attribute_offers_only_values_that_begin_with_the_text() {
    let schema = desk();
    let decider = decider(&schema, &desk_history(), true);

    let typed = answer_one(&decider, &schema, "d9", "team", ASKED, Some("b"));
    let other = answer_one(&decider, &schema, "d9", "team", ASKED, Some("n"));

    assert_eq!(typed.candidates[0].value, "billing");
    assert!(typed.candidates.iter().all(|c| c.value.starts_with('b')));
    assert!(!other.candidates.is_empty());
    assert!(other.candidates.iter().all(|c| c.value.starts_with('n')));
}

#[test]
#[should_panic(expected = "'internal' is off")]
fn an_attribute_that_is_off_is_not_asked_about_alone() {
    let schema = desk();
    answer_one(
        &decider(&schema, &desk_history(), true),
        &schema,
        "d9",
        "internal",
        ASKED,
        None,
    );
}

#[test]
#[should_panic(expected = "'team' already has a value")]
fn an_attribute_with_a_value_is_not_asked_about_alone() {
    let schema = desk();
    let with_team = [ASKED, &[("team", "billing")]].concat();
    answer_one(
        &decider(&schema, &desk_history(), true),
        &schema,
        "d9",
        "team",
        &with_team,
        None,
    );
}

#[test]
#[should_panic(expected = "has no attribute 'nothing'")]
fn an_unknown_attribute_is_not_asked_about() {
    let schema = desk();
    answer_one(
        &decider(&schema, &desk_history(), true),
        &schema,
        "d9",
        "nothing",
        ASKED,
        None,
    );
}

// --- Guesses and the record's own evidence ---------------------------------------------------------------------------

#[test]
fn a_key_below_the_threshold_is_a_guess_and_still_keeps_an_external_judgement_out() {
    let schema = schema(
        "ticket",
        vec![
            observed("component"),
            decided("team", |t| t.key_threshold = Some(0.6)),
        ],
    );
    let mut decider = Decider::new(KeyMemory::new(60, 1.0), None);
    decider.rebuild(
        &schema,
        &[
            record("d1", 1, &[("component", "vpn"), ("team", "network")]),
            record("d2", 2, &[("component", "vpn"), ("team", "network")]),
            record("d3", 3, &[("component", "printer"), ("team", "facilities")]),
        ],
    );
    let asked = |id: &str, component: &str| {
        let mut records: Vec<DecisionRecord> = Vec::new();
        let answers = decider.answer_with(
            &schema,
            id,
            &values(&[("component", component)]),
            &BTreeMap::new(),
            &mut |r| records.push(r.clone()),
        );
        (answers.into_iter().next().unwrap(), records.remove(0))
    };

    // vpn: 2 of 2, strength 2/3 — answers. printer: 1 of 1, strength 1/2 — a guess.
    let (strong, strong_record) = asked("d4", "vpn");
    let (weak, weak_record) = asked("d5", "printer");

    assert!(strong.answered() && strong.candidates[0].trusted);
    assert!(!weak.answered());
    assert_eq!(weak.source, Source::SettledFieldMemory);
    let guess = &weak.candidates[0];
    assert_eq!(
        (guess.value.as_str(), guess.score, guess.trusted),
        ("facilities", 0.5, false)
    );
    assert_eq!(
        guess.evidence.as_ref().map(ToString::to_string).as_deref(),
        Some("component: printer")
    );
    // A value the record's own keys back, however weakly, beats an external judgement's guess.
    assert_eq!(weak.needs_external, None);
    assert!(weak_record.answered.is_none());
    let answered = strong_record.answered.unwrap();
    assert_eq!(
        (answered.layer, answered.value.as_str()),
        (Source::SettledFieldMemory, "network")
    );
}

fn summaries(memory_threshold: Option<f64>) -> (Schema, Decider) {
    let schema = schema(
        "ticket",
        vec![
            observed("summary"),
            decided("team", |t| t.memory_threshold = memory_threshold),
        ],
    );
    let decider = decider(
        &schema,
        &[
            record(
                "d1",
                1,
                &[
                    ("summary", "printer on floor 3 is jammed"),
                    ("team", "facilities"),
                ],
            ),
            record("d2", 2, &[("summary", "vpn drops"), ("team", "network")]),
            record("d3", 3, &[("summary", "wifi drops"), ("team", "network")]),
        ],
        true,
    );
    (schema, decider)
}

fn layered(answer: &Answer) -> Vec<(&str, Source, Option<String>, bool)> {
    answer
        .candidates
        .iter()
        .map(|c| {
            (
                c.value.as_str(),
                c.source,
                c.evidence.as_ref().map(ToString::to_string),
                c.trusted,
            )
        })
        .collect()
}

#[test]
fn the_nearest_record_below_the_similarity_threshold_is_the_last_guess() {
    let (schema, decider) = summaries(Some(0.99));

    let team = only(
        &decider,
        &schema,
        "d4",
        &[("summary", "printer on floor 2 is jammed")],
    );

    // Below its threshold, the nearest record is a weaker guess than the attribute's most frequent value.
    assert!(!team.answered());
    assert_eq!(
        layered(&team),
        [
            ("network", Source::SettledFieldMemory, None, false),
            (
                "facilities",
                Source::SimilarDocument,
                Some("d1".into()),
                false
            ),
        ]
    );
}

#[test]
fn without_a_similarity_threshold_a_near_identical_record_is_still_offered_as_the_last_guess() {
    let (schema, decider) = summaries(None);

    let team = only(
        &decider,
        &schema,
        "d4",
        &[("summary", "printer on floor 3 is jammed again")],
    );

    assert!(!team.answered());
    assert_eq!(
        layered(&team),
        [
            ("network", Source::SettledFieldMemory, None, false),
            (
                "facilities",
                Source::SimilarDocument,
                Some("d1".into()),
                false
            ),
        ]
    );
}

#[test]
fn guesses_come_as_a_weaker_key_then_the_most_frequent_value_then_the_nearest_record() {
    let schema = schema(
        "ticket",
        vec![
            observed("summary"),
            observed("site"),
            decided("team", |t| {
                t.depends_on = Some(vec!["summary".into(), "site".into()]);
                t.key_threshold = Some(0.99);
                t.memory_threshold = Some(0.99);
            }),
        ],
    );
    let doc = |n: i64, summary: &str, site: &str, team: &str| {
        record(
            &format!("d{n}"),
            n,
            &[("summary", summary), ("site", site), ("team", team)],
        )
    };
    let decider = decider(
        &schema,
        &[
            doc(1, "printer on floor 3 is jammed", "north", "facilities"),
            doc(2, "vpn drops", "south", "network"),
            doc(3, "wifi drops", "south", "network"),
            doc(4, "badge reader is dead", "east", "security"),
        ],
        true,
    );

    let team = only(
        &decider,
        &schema,
        "d9",
        &[
            ("summary", "printer on floor 2 is jammed"),
            ("site", "east"),
        ],
    );

    assert!(!team.answered());
    assert_eq!(
        team.candidates
            .iter()
            .map(|c| (c.value.as_str(), c.source, c.trusted))
            .collect::<Vec<_>>(),
        [
            ("security", Source::SettledFieldMemory, false), // under the weak key site: east
            ("network", Source::SettledFieldMemory, false),  // the most frequent value
            ("facilities", Source::SimilarDocument, false), // the nearest record, below its threshold
        ]
    );
    assert_eq!(
        team.candidates[0]
            .evidence
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("site: east")
    );
    assert_eq!(team.candidates[1].evidence, None);
}

#[test]
fn a_saved_record_is_passed_over_for_the_next_similar_one() {
    let schema = schema(
        "ticket",
        vec![
            observed("summary"),
            decided("team", |t| t.memory_threshold = Some(0.5)),
        ],
    );
    let decider = decider(
        &schema,
        &[
            record(
                "d1",
                1,
                &[
                    ("summary", "printer on floor 3 is jammed"),
                    ("team", "facilities"),
                ],
            ),
            record(
                "d2",
                2,
                &[
                    ("summary", "printer on floor 4 is jammed"),
                    ("team", "facilities"),
                ],
            ),
        ],
        true,
    );

    // Asking about d1: its own saved version is the most similar record, and is passed over.
    let team = only(
        &decider,
        &schema,
        "d1",
        &[("summary", "printer on floor 3 is jammed")],
    );

    assert!(team.answered());
    assert_eq!(team.source, Source::SimilarDocument);
    assert_eq!(
        team.candidates[0]
            .evidence
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("d2")
    );
}

#[test]
fn answering_from_values_writes_nothing_and_a_saved_record_is_not_its_own_evidence() {
    let schema = schema(
        "ticket",
        vec![
            observed("component"),
            observed("summary"),
            decided("team", |t| {
                t.key_threshold = Some(0.3);
                t.memory_threshold = Some(0.3);
            }),
            decided("assessment", |a| a.policy = AttributePolicy::Off),
        ],
    );
    let saved = record(
        "d1",
        0,
        &[
            ("component", "badge"),
            ("summary", "door will not open"),
            ("team", "security"),
        ],
    );
    let decider = decider(&schema, std::slice::from_ref(&saved), true);
    let asked = [("component", "badge"), ("summary", "door will not open")];

    // Asking about the saved record: its own settlement backs nothing, by key or by similarity.
    let own = decider.answer(&schema, "d1", &values(&asked), &BTreeMap::new());
    assert_eq!(own.len(), 1); // assessment is off
    assert_eq!(own[0].attribute, "team");
    assert!(!own[0].answered() && own[0].candidates.is_empty());

    // Another record with the same values is backed by it, and asking twice gives the same: asking wrote nothing.
    for _ in 0..2 {
        let other = only(&decider, &schema, "d2", &asked);
        assert!(other.answered());
        assert_eq!(other.candidates[0].value, "security");
    }
    assert!(
        decider
            .answer(
                &schema,
                "d2",
                &values(&[("component", "badge"), ("team", "network")]),
                &BTreeMap::new()
            )
            .is_empty()
    );
}

// --- Choosing thresholds by how values were settled ------------------------------------------------------------------

fn origin() -> Schema {
    schema(
        "ticket",
        vec![
            observed("summary"),
            decided("team", |t| t.memory_threshold = Some(0.3)),
        ],
    )
}

fn settled(
    id: &str,
    summary: &str,
    team: &str,
    minute: i64,
    kind: Option<SettlementKind>,
) -> Record {
    let mut record = record(id, minute, &[("summary", summary), ("team", team)]);
    if let Some(kind) = kind {
        record.settlements.insert("team".into(), kind);
    }
    record
}

#[test]
fn counting_only_entered_settlements_replays_every_record_but_asks_only_those() {
    use SettlementKind::{Accept, Enter, Fill};
    let records = [
        settled("d1", "vpn drops", "network", 0, Some(Accept)),
        settled("d2", "vpn drops", "network", 1, Some(Enter)),
        settled("d3", "vpn drops", "network", 2, Some(Fill)),
        settled("d4", "vpn drops", "facilities", 3, Some(Enter)),
        settled("d5", "vpn drops", "network", 4, None), // saved before settlements were kept
    ];
    let entered = Choice {
        counting: Some(&[Enter]),
        ..choice(0.5, 1)
    };

    let mut keys = KeyMemory::default();
    let replay = select_key_threshold(&mut keys, &origin(), "team", &records, entered);
    assert_eq!(replay.lookups, 2); // d2 and d4 — d1 meets an empty memory, the rest are not counted
    assert_eq!(keys.count("ticket"), 4); // every record a person settled is replayed into memory — the filled d3 is not

    let all = select_key_threshold(
        &mut KeyMemory::default(),
        &origin(),
        "team",
        &records,
        choice(0.5, 1),
    );
    assert_eq!(all.lookups, 3); // d2, d4 and d5, the one not known included — the filled d3 is never asked about

    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &origin(),
        "team",
        &records,
        Choice {
            counting: Some(&[Enter, Accept]),
            ..choice(0.5, 1)
        },
    );
    assert_eq!(layers.key.lookups, 2);
}

#[test]
fn a_value_settled_over_another_suggestion_is_counted_only_where_asked_for() {
    use SettlementKind::{AcceptOther, CorrectOther, Enter};
    let records = [
        settled("d1", "vpn drops", "network", 0, Some(AcceptOther)),
        settled("d2", "vpn drops", "network", 1, Some(CorrectOther)),
        settled("d3", "vpn drops", "network", 2, Some(AcceptOther)),
    ];
    let replay = |counting| {
        select_key_threshold(
            &mut KeyMemory::default(),
            &origin(),
            "team",
            &records,
            Choice {
                counting,
                ..choice(1.0, 1)
            },
        )
    };

    assert_eq!(replay(None).lookups, 2);
    assert_eq!(replay(Some(&[Enter])).lookups, 0);
}

#[test]
fn choosing_thresholds_neither_asks_about_nor_remembers_a_filled_value() {
    use SettlementKind::{Accept, Correct, Enter, Fill};
    let records = [
        settled("d1", "vpn drops", "network", 0, Some(Accept)),
        settled("d2", "vpn drops", "facilities", 1, Some(Fill)),
        settled("d3", "vpn drops", "facilities", 2, Some(Fill)),
        settled("d4", "vpn drops", "network", 3, Some(Accept)),
    ];

    // Were the fills remembered, d4 would meet two "facilities" against one "network" and be answered wrongly.
    let mut keys = KeyMemory::default();
    let replay = select_key_threshold(&mut keys, &origin(), "team", &records, choice(1.0, 1));
    assert_eq!(replay.lookups, 1);
    assert!(replay.chosen.is_some());
    let schema = origin();
    let ranked = keys.rank(
        &gil::key_memory::Lookup::new(&schema, "team", &values(&[("summary", "vpn drops")])),
        3,
    );
    assert_eq!(
        ranked.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
        ["network"]
    );

    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "team",
        &records,
        choice(1.0, 1),
    );
    assert_eq!(layers.key.lookups, 1);
    assert_eq!(layers.memory.lookups, 0); // the similar record layer is left only what the key did not answer — nothing

    // A filled value checked later and entered is asked about against what d1 taught; the filled d2 is still left out.
    let by_hand: &[SettlementKind] = &[Accept, Correct, Enter];
    let filled = [
        settled("d1", "vpn drops", "network", 0, Some(Accept)),
        settled("d2", "vpn drops", "network", 1, Some(Fill)),
        settled("d3", "vpn drops", "network", 2, Some(Fill)),
    ];
    let counted = Choice {
        counting: Some(by_hand),
        ..choice(1.0, 1)
    };
    let none = select_key_threshold(&mut KeyMemory::default(), &schema, "team", &filled, counted);
    assert_eq!(none.lookups, 0); // nothing the host filled is asked about

    let checked_later = [
        filled[0].clone(),
        filled[1].clone(),
        settled("d3", "vpn drops", "network", 2, Some(Enter)),
    ];
    let mut keys = KeyMemory::default();
    let replay = select_key_threshold(&mut keys, &schema, "team", &checked_later, counted);
    assert_eq!(replay.lookups, 1);
    assert!(replay.chosen.is_some());
    assert_eq!(keys.count("ticket"), 2);
}

#[test]
fn counting_nothing_in_particular_replays_as_records_without_their_kinds() {
    use SettlementKind::{Accept, Correct, Enter};
    let with_kinds = [
        settled("d1", "vpn drops", "network", 0, Some(Accept)),
        settled("d2", "vpn drops", "facilities", 1, Some(Enter)),
        settled("d3", "printer jammed", "facilities", 2, Some(Enter)),
        settled("d4", "vpn drops", "network", 3, Some(Correct)),
    ];
    let without_kinds: Vec<Record> = with_kinds
        .iter()
        .map(|r| Record {
            settlements: BTreeMap::new(),
            ..r.clone()
        })
        .collect();
    let key = |records: &[Record]| {
        select_key_threshold(
            &mut KeyMemory::default(),
            &origin(),
            "team",
            records,
            choice(0.5, 1),
        )
    };
    let layers = |records: &[Record]| {
        select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &origin(),
            "team",
            records,
            choice(0.5, 1),
        )
    };

    assert_eq!(key(&with_kinds), key(&without_kinds));
    assert_eq!(layers(&with_kinds), layers(&without_kinds));
}

// --- Keys that decide the attribute and keys that do not -------------------------------------------------------------

const DEPARTMENTS: [&str; 5] = ["sales", "finance", "legal", "support", "research"];

/// Each team's requests; the team follows from what the request says, never from the department.
const TEAMS: [(&str, [&str; 4]); 5] = [
    (
        "network",
        [
            "vpn drops every few minutes",
            "wifi is very slow",
            "cannot reach the intranet",
            "ethernet port is dead",
        ],
    ),
    (
        "facilities",
        [
            "printer is jammed",
            "the light keeps flickering",
            "door lock is broken",
            "heating is off",
        ],
    ),
    (
        "accounts",
        [
            "please reset my password",
            "my account is locked",
            "cannot log in to email",
            "lost my mfa device",
        ],
    ),
    (
        "hardware",
        [
            "laptop screen is cracked",
            "keyboard keys are stuck",
            "mouse stopped working",
            "battery is swollen",
        ],
    ),
    (
        "software",
        [
            "spreadsheet app crashes",
            "license has expired",
            "the update failed",
            "app freezes on start",
        ],
    ),
];

fn intake(key_threshold: Option<f64>, memory_threshold: Option<f64>) -> Schema {
    let mut department = observed("department");
    department.candidates = Some(DEPARTMENTS.iter().map(|d| (*d).to_owned()).collect());
    schema(
        "intake",
        vec![
            observed("request"),
            department,
            decided("team", |t| {
                t.key_threshold = key_threshold;
                t.memory_threshold = memory_threshold;
            }),
        ],
    )
}

/// Free-text requests whose team the text decides, filed from a department drawn at random.
fn requests(draws: &mut Draws, count: i64, first: i64) -> Vec<Record> {
    (first..first + count)
        .map(|i| {
            let (team, phrases) = TEAMS[draws.below(TEAMS.len())];
            let request = format!(
                "{} since {}",
                phrases[draws.below(phrases.len())],
                1 + draws.below(998)
            );
            let department = DEPARTMENTS[draws.below(DEPARTMENTS.len())];
            record(
                &format!("r{i}"),
                i,
                &[
                    ("request", &request),
                    ("department", department),
                    ("team", team),
                ],
            )
        })
        .collect()
}

fn without_team(record: &Record) -> BTreeMap<String, String> {
    let mut values = record.values.clone();
    values.remove("team");
    values
}

#[test]
fn a_key_that_does_not_decide_the_attribute_never_outranks_a_similar_record() {
    let mut draws = Draws(7);
    let history = requests(&mut draws, 300, 1);
    let asked = requests(&mut draws, 100, 1000);
    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &intake(None, None),
        "team",
        &history,
        choice(0.8, 10),
    );
    assert!(layers.key.chosen.is_none()); // the department alone never reaches the target precision
    let similarity = layers.memory.chosen.expect("the request text answers well");

    // The department's key, chosen or not, does not keep the similar record from answering.
    let schema = intake(
        layers.key.chosen.map(|c| c.threshold),
        Some(similarity.threshold),
    );
    let decider = decider(&schema, &history, true);
    let (mut answered, mut right) = (0, 0);
    for record in &asked {
        let team = decider
            .answer(&schema, &record.id, &without_team(record), &BTreeMap::new())
            .remove(0);
        if team.answered() {
            answered += 1;
            right += usize::from(team.candidates[0].value == record.values["team"]);
        } else {
            // A guess is still listed, but the department alone never answers.
            assert!(!team.candidates.is_empty());
        }
    }

    assert!(answered > 50, "{answered}");
    assert!(
        right as f64 / answered as f64 >= 0.8,
        "{right} of {answered}"
    );
}

#[test]
fn a_key_that_decides_the_attribute_is_chosen_to_answer() {
    let mut draws = Draws(3);
    let components = [
        "vpn", "printer", "badge", "mail", "laptop", "phone", "desk", "license",
    ];
    let teams = [
        "network",
        "facilities",
        "security",
        "accounts",
        "hardware",
        "telecom",
        "facilities",
        "software",
    ];
    let ticket = |key_threshold| {
        schema(
            "ticket",
            vec![
                observed("component"),
                observed("summary"),
                decided("team", |t| t.key_threshold = key_threshold),
            ],
        )
    };
    let history: Vec<Record> = (1..=200)
        .map(|i| {
            let c = draws.below(components.len());
            let summary = format!(
                "issue number {} reported on {} of the month",
                draws.below(100_000),
                1 + draws.below(27)
            );
            record(
                &format!("t{i}"),
                i,
                &[
                    ("component", components[c]),
                    ("summary", &summary),
                    ("team", teams[c]),
                ],
            )
        })
        .collect();

    let replay = select_key_threshold(
        &mut KeyMemory::default(),
        &ticket(None),
        "team",
        &history,
        choice(0.95, 10),
    );
    let chosen = replay.chosen.expect("the component decides the team");
    assert_eq!((chosen.precision, chosen.lookups), (1.0, 199));
    assert!(chosen.answer_rate > 0.9); // only a component's first few settlements are too weak to answer

    let schema = ticket(Some(chosen.threshold));
    let decider = decider(&schema, &history, false);
    let team = only(
        &decider,
        &schema,
        "t201",
        &[("component", "badge"), ("summary", "door will not open")],
    );
    assert!(team.answered());
    assert_eq!(team.source, Source::SettledFieldMemory);
    assert_eq!(team.candidates[0].value, "security");
}

#[test]
fn a_weak_key_is_not_trusted_because_repeats_of_other_records_answer_well() {
    // Twenty request texts, each filed ten times with the same team; the source is unrelated to the team. Once a text has
    // been seen, its own key answers every repeat rightly. A text seen for the first time has only its source as a known
    // key, whose most frequent team is right about one time in four — those answers must not ride on the repeats'
    // precision.
    let mut draws = Draws(11);
    let sources = ["email", "phone", "chat", "portal"];
    let teams = ["network", "facilities", "accounts", "hardware", "software"];
    let ticket = |key_threshold| {
        let mut source = observed("source");
        source.candidates = Some(sources.iter().map(|s| (*s).to_owned()).collect());
        schema(
            "ticket",
            vec![
                observed("request"),
                source,
                decided("team", |t| t.key_threshold = key_threshold),
            ],
        )
    };
    let mut filed: Vec<(String, &str)> = (0..20)
        .flat_map(|t| {
            std::iter::repeat_n(
                (format!("request template {t}"), teams[t % teams.len()]),
                10,
            )
        })
        .collect();
    for i in (1..filed.len()).rev() {
        filed.swap(i, draws.below(i + 1)); // shuffled
    }
    let history: Vec<Record> = filed
        .iter()
        .zip(1..)
        .map(|((text, team), i)| {
            record(
                &format!("t{i}"),
                i,
                &[
                    ("request", text),
                    ("source", sources[draws.below(sources.len())]),
                    ("team", team),
                ],
            )
        })
        .collect();

    let chosen = select_key_threshold(
        &mut KeyMemory::default(),
        &ticket(None),
        "team",
        &history,
        choice(0.9, 10),
    )
    .chosen
    .expect("repeats still answer");
    let schema = ticket(Some(chosen.threshold));
    let mut keys = KeyMemory::default();
    for record in &history {
        keys.put(&schema, record);
    }
    let best = |pairs: &[(&str, &str)]| {
        let known = values(pairs);
        keys.rank(&gil::key_memory::Lookup::new(&schema, "team", &known), 1)
            .remove(0)
    };

    for source in sources {
        let weak = best(&[("request", "a request never seen"), ("source", source)]);
        assert!(
            !weak.trusted,
            "the source '{source}' alone backs {} about one time in four (threshold {})",
            weak.value, chosen.threshold
        );
    }
    let repeat = best(&[("request", "request template 3"), ("source", "phone")]);
    assert_eq!((repeat.value.as_str(), repeat.trusted), ("hardware", true));
}

#[test]
fn the_similarity_threshold_is_chosen_on_the_lookups_no_key_answers() {
    // Most requests name a component that decides the team, and their text names it too. The rest come from a catch-all
    // component whose team varies; their texts are just as similar to each other, but the nearest one's team is right
    // about one time in five. The similarity threshold is chosen on what the key leaves — the catch-all requests — and
    // so is not chosen at all, and answers none of them.
    let mut draws = Draws(5);
    let components = ["vpn", "printer", "badge", "laptop", "mail"];
    let teams = ["network", "facilities", "security", "hardware", "accounts"];
    let ticket = |key_threshold, memory_threshold| {
        schema(
            "ticket",
            vec![
                observed("component"),
                observed("summary"),
                decided("team", |t| {
                    t.similar_votes = 1;
                    t.key_threshold = key_threshold;
                    t.memory_threshold = memory_threshold;
                }),
            ],
        )
    };
    let mut request = |i: i64| {
        let (component, team) = if draws.unit() < 0.85 {
            let n = i as usize % components.len();
            (components[n], teams[n])
        } else {
            ("other", teams[draws.below(teams.len())])
        };
        let summary = format!(
            "{component} routine problem report {}",
            10_000 + draws.below(90_000)
        );
        record(
            &format!("t{i}"),
            i,
            &[
                ("component", component),
                ("summary", &summary),
                ("team", team),
            ],
        )
    };
    let history: Vec<Record> = (1..=400).map(&mut request).collect();
    let asked: Vec<Record> = (401..=800)
        .map(&mut request)
        .filter(|r| r.values["component"] == "other")
        .collect();

    let layered = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &ticket(None, None),
        "team",
        &history,
        choice(0.8, 10),
    );
    let key = layered
        .key
        .chosen
        .expect("the named components decide the team");
    assert!(layered.memory.chosen.is_none()); // what the key leaves never reaches the target
    assert!(layered.memory.most_precise.unwrap().precision < 0.8); // and the replay says by how much

    let schema = ticket(Some(key.threshold), None);
    let decider = decider(&schema, &history, true);
    let similar_answers = asked
        .iter()
        .filter(|r| {
            let team = decider
                .answer(&schema, &r.id, &without_team(r), &BTreeMap::new())
                .remove(0);
            team.answered() && team.source == Source::SimilarDocument
        })
        .count();
    assert_eq!(similar_answers, 0);
}

#[test]
fn a_record_settled_again_is_not_its_own_evidence_in_the_replay() {
    // Every request is saved twice under its own id, unchanged, and its team is drawn at random: nothing but the request
    // itself predicts it. A lookup for a saved record passes over its saved version, so neither layer can answer these
    // well, and the replay must not find otherwise by asking the second save about the first.
    let mut draws = Draws(5);
    let teams = ["network", "facilities", "security", "accounts", "hardware"];
    let schema = schema("ticket", vec![observed("summary"), decided("team", |_| {})]);
    let history: Vec<Record> = (1..=100)
        .flat_map(|i| {
            let summary = format!("request {i} about item {}", i * 7);
            let team = teams[draws.below(teams.len())];
            let saved = record(
                &format!("r{i}"),
                2 * i,
                &[("summary", &summary), ("team", team)],
            );
            let again = Record {
                settled_at: (2 * i + 1) * MINUTE,
                ..saved.clone()
            };
            [saved, again]
        })
        .collect();

    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "team",
        &history,
        choice(0.8, 10),
    );

    assert!(layers.key.chosen.is_none()); // no request's summary is seen twice under different ids
    assert!(layers.memory.chosen.is_none());
    assert!(layers.memory.most_precise.unwrap().precision < 0.5); // the nearest other request, not the saved version
}
