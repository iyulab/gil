//! Choosing thresholds by replay, held to what the reference implementation's threshold selection, checked settlement and
//! must-catch tests expect: precision is fitted as a non-decreasing function of the score and the threshold is the lowest
//! one down to which every band meets the target; no threshold is chosen when a band falls short or the answers are too
//! few; a check counts as the decisions it stands for in precision but as one answer toward the minimum, and only when
//! its kind is counted; a promise falls back to the whole replay when the latest records hold too few answers; a key
//! that once settled a value to catch keeps its lookups but answers none of them, in either layer; and the similarity
//! floor a vote-decided threshold carries. A check is drawn with a probability above 0 and at most 1.
//!
//! The reference tests drove the fit with a scripted memory; here the key memory is real and records settled as
//! entries that are not counted build up each key, so the counted lookups land on the scores a scenario names.

use std::collections::BTreeMap;

use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, KeyState, ThresholdChoice, select_key_threshold, select_layers, select_layers_with,
};
use gil::schema::{
    Attribute, AttributeRole, CoarseLevel, Record, Schema, Settlement, SettlementKind,
};

const MINUTE: i64 = 60 * 10_000_000;
const ACCEPTED: &[SettlementKind] = &[SettlementKind::Accept];

fn choice(target_precision: f64, minimum_answered: usize) -> Choice<'static> {
    Choice {
        target_precision,
        minimum_answered,
        counting: None,
    }
}

fn counting_accepted(target_precision: f64, minimum_answered: usize) -> Choice<'static> {
    Choice {
        counting: Some(ACCEPTED),
        ..choice(target_precision, minimum_answered)
    }
}

/// Record `<prefix><n>`, settled `n` minutes after the epoch, its decided attribute settled as `kind` where given.
fn record(
    prefix: &str,
    n: i64,
    pairs: &[(&str, &str)],
    settled: Option<(&str, SettlementKind, Option<f64>)>,
) -> Record {
    let mut record = Record {
        id: format!("{prefix}{n}"),
        values: pairs
            .iter()
            .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
            .collect(),
        settled_at: n * MINUTE,
        ..Record::default()
    };
    if let Some((attribute, kind, probability)) = settled {
        record.settlements.insert(attribute.to_owned(), kind);
        if let Some(p) = probability {
            record.probabilities.insert(attribute.to_owned(), p);
        }
    }
    record
}

/// The low end of the Wilson score interval at one standard error — the promise a replay gives a choice.
fn lower(precision: f64, answered: f64) -> f64 {
    let centre = precision + (1.0 / (2.0 * answered));
    let margin =
        ((precision * (1.0 - precision) / answered) + (1.0 / (4.0 * answered * answered))).sqrt();
    (centre - margin) / (1.0 + (1.0 / answered))
}

/// A choice of the key layer whose answers all count once, with the promise of the whole replay.
fn promised(threshold: f64, precision: f64, answered: usize, lookups: usize) -> ThresholdChoice {
    ThresholdChoice {
        threshold,
        precision,
        answer_rate: answered as f64 / lookups as f64,
        answered,
        lookups,
        answered_weight: answered as f64,
        effective_answered: answered as f64,
        similarity_floor: None,
        promise: lower(precision, answered as f64),
        promise_answered: answered as f64,
    }
}

/// The score of a value settled under its key alone `n` times out of `n`, with no recency decay.
fn strength(n: usize) -> f64 {
    n as f64 / (n as f64 + 1.0)
}

/// A component observed, a team decided by it.
fn ticket() -> Schema {
    Schema {
        name: "ticket".into(),
        attributes: vec![
            Attribute::new("component", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    }
}

/// One answer per scenario entry: under a component of its own, `priors` entries of team `a` (not counted), then one
/// accepted record — of team `a` when the answer is to be right, `b` when not — answered at [`strength`]`(priors)`.
fn scenario(answers: &[(usize, bool)]) -> Vec<Record> {
    let mut records = Vec::new();
    let mut n = 0;
    for (i, (priors, right)) in answers.iter().enumerate() {
        let component = format!("c{i}");
        for _ in 0..*priors {
            n += 1;
            records.push(record(
                "d",
                n,
                &[("component", &component), ("team", "a")],
                Some(("team", SettlementKind::Enter, None)),
            ));
        }
        n += 1;
        records.push(record(
            "d",
            n,
            &[
                ("component", &component),
                ("team", if *right { "a" } else { "b" }),
            ],
            Some(("team", SettlementKind::Accept, None)),
        ));
    }
    records
}

fn replayed(records: &[Record], choice: Choice) -> gil::replay::ThresholdReplay {
    select_key_threshold(
        &mut KeyMemory::new(60, 1.0),
        &ticket(),
        "team",
        records,
        choice,
    )
}

#[test]
fn the_lowest_threshold_that_still_meets_the_target_is_chosen() {
    // Scores and whether the answer matched: 5/6 ✓, 4/5 ✓, 3/4 ✗, 2/3 ✓, 1/2 ✗. Fitted as non-decreasing in the score:
    // [4/5, 5/6] right always, [2/3, 3/4] half the time (2/3 alone would outdo 3/4), 1/2 never.
    let records = scenario(&[(5, true), (4, true), (3, false), (2, true), (1, false)]);

    let strict = replayed(&records, counting_accepted(0.95, 1));
    let top = promised(strength(4), 1.0, 2, 5);
    assert_eq!(strict.chosen.as_ref(), Some(&top));
    assert_eq!(strict.most_precise.as_ref(), Some(&top));
    assert_eq!((strict.lookups, strict.candidates), (5, 5));

    // Together from 2/3 up the answers are right three times in four, but the band at 2/3–3/4 only half the time.
    let loose = replayed(&records, counting_accepted(0.75, 1));
    assert_eq!(loose.chosen.as_ref(), Some(&top));

    let lower_target = replayed(&records, counting_accepted(0.5, 1));
    assert_eq!(lower_target.chosen, Some(promised(strength(2), 0.75, 4, 5)));
    // The most precise threshold is not the one chosen: that is the lowest to meet the target.
    assert_eq!(lower_target.most_precise.as_ref(), Some(&top));
}

#[test]
fn no_threshold_is_chosen_out_of_reach_or_on_too_few_answers_but_the_replay_says_how_close_it_came()
{
    let wrong = replayed(
        &scenario(&[(2, false), (1, false)]),
        counting_accepted(0.5, 1),
    );
    assert_eq!(wrong.chosen, None);
    assert_eq!(wrong.most_precise, Some(promised(strength(1), 0.0, 2, 2)));
    assert_eq!((wrong.lookups, wrong.candidates), (2, 2));

    // Two answers, fewer than the three a precision may rest on.
    let few = replayed(
        &scenario(&[(2, true), (1, true)]),
        counting_accepted(0.5, 3),
    );
    assert_eq!((few.chosen, few.most_precise), (None, None));
    assert_eq!((few.lookups, few.candidates), (2, 2));
}

#[test]
fn answers_that_meet_the_target_together_are_not_chosen_when_a_band_among_them_falls_short() {
    // 3/4 ✓ 3/4 ✓ 2/3 ✓ 2/3 ✗: bands [3/4] right always and [2/3] half the time. At least four answers must back a
    // precision, and the four are right three times in four — the target, taken together — but the band at 2/3 is not.
    let records = scenario(&[(3, true), (3, true), (2, true), (2, false)]);

    let replay = replayed(&records, counting_accepted(0.75, 4));

    assert_eq!(replay.chosen, None);
    assert_eq!(replay.most_precise, Some(promised(strength(2), 0.75, 4, 4)));
    assert_eq!((replay.lookups, replay.candidates), (4, 4));
}

#[test]
#[should_panic(expected = "is not a decided attribute")]
fn an_observed_attribute_has_no_threshold_to_choose() {
    select_key_threshold(
        &mut KeyMemory::new(60, 1.0),
        &ticket(),
        "component",
        &scenario(&[(1, true)]),
        choice(0.9, 1),
    );
}

#[test]
#[should_panic(expected = "target_precision must be in (0, 1]")]
fn a_target_above_1_is_refused() {
    replayed(&scenario(&[(1, true)]), choice(1.5, 1));
}

#[test]
#[should_panic(expected = "minimum_answered must be at least 1")]
fn a_minimum_of_no_answers_is_refused() {
    replayed(&scenario(&[(1, true)]), choice(0.9, 0));
}

#[test]
fn the_promise_falls_back_to_the_whole_replay_when_the_latest_records_answer_too_little() {
    // Five vpn tickets answered rightly at 1/2 … 4/5, then 5,000 records without a team, then one more answered at 5/6:
    // only that last answer lies among the latest 5,000 records replayed.
    let vpn = |n: i64| record("d", n, &[("component", "vpn"), ("team", "network")], None);
    let records: Vec<Record> = (1..=5)
        .map(vpn)
        .chain((6..=5_005).map(|n| record("d", n, &[("component", "vpn")], None)))
        .chain([vpn(5_006)])
        .collect();

    let one = replayed(&records, choice(0.9, 1)).chosen.unwrap();
    assert_eq!((one.threshold, one.precision), (strength(1), 1.0));
    assert_eq!(one.effective_answered, 5.0);
    assert_eq!((one.promise, one.promise_answered), (lower(1.0, 1.0), 1.0)); // the latest answer alone

    let two = replayed(&records, choice(0.9, 2)).chosen.unwrap();
    assert_eq!(two.threshold, one.threshold);
    assert_eq!((two.promise, two.promise_answered), (lower(1.0, 5.0), 5.0)); // one is too few: the whole replay
}

/// A summary observed, a team decided and trusted at any score of the similar record layer.
fn summarized() -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.memory_threshold = Some(0.3);
    Schema {
        name: "ticket".into(),
        attributes: vec![Attribute::new("summary", AttributeRole::Observed), team],
    }
}

/// A vpn ticket settled `day` days in, its team settled as `kind`.
fn checked(
    id: &str,
    team: &str,
    day: i64,
    kind: SettlementKind,
    probability: Option<f64>,
) -> Record {
    Record {
        settled_at: day * 24 * 60 * MINUTE,
        ..record(
            id,
            0,
            &[("summary", "vpn drops"), ("team", team)],
            Some(("team", kind, probability)),
        )
    }
}

#[test]
fn choosing_thresholds_counts_a_check_as_the_decisions_it_stands_for() {
    // d2 was one of four filled decisions, drawn with a quarter's chance and found right; d3, settled by a person, was
    // answered wrongly. Counted once each that is one right in two; d2 standing for its four, four right in five.
    let records = [
        checked("d1", "network", 0, SettlementKind::Accept, None),
        checked("d2", "network", 1, SettlementKind::Check, Some(0.25)),
        checked("d3", "facilities", 2, SettlementKind::Accept, None),
    ];
    let schema = summarized();

    let replay = select_key_threshold(
        &mut KeyMemory::default(),
        &schema,
        "team",
        &records,
        choice(0.75, 1),
    );
    assert_eq!(replay.lookups, 2);
    let chosen = replay.chosen.unwrap();
    assert!((chosen.precision - 0.8).abs() < 1e-12);
    assert!((chosen.answer_rate - 1.0).abs() < 1e-12);
    assert_eq!((chosen.answered, chosen.lookups), (2, 2));
    assert!((chosen.answered_weight - 5.0).abs() < 1e-12);
    assert!((chosen.effective_answered - 25.0 / 17.0).abs() < 1e-12); // (4 + 1)² ÷ (4² + 1²)

    let layers = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "team",
        &records,
        choice(0.75, 1),
    );
    assert!((layers.key.chosen.unwrap().precision - 0.8).abs() < 1e-12);

    // The same values entered by a person count once each: one right in two misses the target.
    let entered = [
        records[0].clone(),
        checked("d2", "network", 1, SettlementKind::Enter, None),
        records[2].clone(),
    ];
    let replay = select_key_threshold(
        &mut KeyMemory::default(),
        &schema,
        "team",
        &entered,
        choice(0.75, 1),
    );
    assert_eq!(replay.chosen, None);
}

#[test]
fn a_check_drawn_rarely_weighs_much_but_counts_as_one_answer_toward_the_minimum() {
    let records = [
        checked("d1", "network", 0, SettlementKind::Accept, None),
        checked("d2", "network", 1, SettlementKind::Check, Some(0.1)),
    ];
    let schema = summarized();

    let one = select_key_threshold(
        &mut KeyMemory::default(),
        &schema,
        "team",
        &records,
        choice(0.5, 1),
    )
    .chosen
    .unwrap();
    assert!((one.answered_weight - 10.0).abs() < 1e-12);
    assert!((one.effective_answered - 1.0).abs() < 1e-12);

    // Ten decisions' weight, one decision's evidence.
    let two = select_key_threshold(
        &mut KeyMemory::default(),
        &schema,
        "team",
        &records,
        choice(0.5, 2),
    );
    assert_eq!((two.chosen, two.most_precise), (None, None));
}

#[test]
fn counting_by_kind_takes_a_check_only_when_it_is_named() {
    let records = [
        checked("d1", "network", 0, SettlementKind::Accept, None),
        checked("d2", "network", 1, SettlementKind::Check, Some(0.25)),
    ];
    let schema = summarized();
    let by_hand = [
        SettlementKind::Accept,
        SettlementKind::Correct,
        SettlementKind::Enter,
    ];
    let with_check = [
        SettlementKind::Accept,
        SettlementKind::Correct,
        SettlementKind::Enter,
        SettlementKind::Check,
    ];
    let lookups = |counting: &[SettlementKind]| {
        let choice = Choice {
            counting: Some(counting),
            ..choice(0.5, 1)
        };
        select_key_threshold(&mut KeyMemory::default(), &schema, "team", &records, choice).lookups
    };

    assert_eq!(lookups(&by_hand), 0);
    assert_eq!(lookups(&with_check), 1);
}

#[test]
fn a_check_is_drawn_with_a_probability_above_0_and_at_most_1() {
    for probability in [0.0, -0.5, 1.5, f64::NAN] {
        let drawn = std::panic::catch_unwind(|| Settlement::check("network", probability));
        assert!(drawn.is_err(), "a check drawn with {probability}");
        let built = Settlement::new(
            SettlementKind::Check,
            Some("network"),
            None,
            Some(probability),
        );
        assert_eq!(
            built.unwrap_err(),
            "a check is drawn with a probability above 0 and at most 1",
            "a check built with {probability}"
        );
    }

    assert_eq!(Settlement::check("network", 0.25).probability, 0.25);
    assert_eq!(
        Settlement::check("network", 1.0).kind,
        SettlementKind::Check
    );
    assert_eq!(
        Settlement::new(SettlementKind::Check, Some("network"), None, Some(1.0)),
        Ok(Settlement::check("network", 1.0))
    );
    // Only a check carries a probability other than 1.
    for settled in [
        Settlement::accept("network"),
        Settlement::fill("network"),
        Settlement::enter("network"),
    ] {
        assert_eq!(settled.probability, 1.0);
    }
}

/// An observed template and a decided risk trusted from 0.5, with the values it must not miss.
fn log(must_catch: Option<&[&str]>) -> Schema {
    let mut risk = Attribute::new("risk", AttributeRole::Decided);
    risk.key_threshold = Some(0.5);
    risk.must_catch = must_catch.map(|m| m.iter().map(|v| (*v).to_owned()).collect());
    Schema {
        name: "log".into(),
        attributes: vec![Attribute::new("template", AttributeRole::Observed), risk],
    }
}

fn line(n: i64, template: &str, risk: &str) -> Record {
    record("l", n, &[("template", template), ("risk", risk)], None)
}

/// Under t1 a 3 was settled once among many 1s; under t2 only 1s.
fn lines() -> Vec<Record> {
    (1..=20)
        .map(|n| line(n, "t1", "1"))
        .chain([line(21, "t1", "3")])
        .chain((22..=41).map(|n| line(n, "t1", "1")))
        .chain((42..=51).map(|n| line(n, "t2", "1")))
        .collect()
}

#[test]
fn choosing_the_key_threshold_does_not_count_answers_the_key_layer_would_not_give() {
    let replay = |schema: &Schema| {
        select_key_threshold(
            &mut KeyMemory::default(),
            schema,
            "risk",
            &lines(),
            choice(0.5, 1),
        )
    };
    let plain = replay(&log(None));
    let watched = replay(&log(Some(&["3"])));

    assert_eq!(watched.lookups, plain.lookups); // every line is still asked about
    // The 20 t1 lines after the 3 are not the key layer's to answer any more.
    assert_eq!(plain.candidates - watched.candidates, 20);
}

#[test]
fn a_key_that_settled_a_value_to_catch_keeps_its_lookups_but_answers_none_of_them() {
    // Under t1: four 1s accepted (three answered rightly at 1/2, 2/3, 3/4), a 3 entered by a person (not counted), then
    // two more 1s accepted — looked up under t1 and, once the 3 is there, held back.
    let accepted = |n: i64, risk: &str| {
        record(
            "l",
            n,
            &[("template", "t1"), ("risk", risk)],
            Some(("risk", SettlementKind::Accept, None)),
        )
    };
    let records = [
        accepted(1, "1"),
        accepted(2, "1"),
        accepted(3, "1"),
        accepted(4, "1"),
        record(
            "l",
            5,
            &[("template", "t1"), ("risk", "3")],
            Some(("risk", SettlementKind::Enter, None)),
        ),
        accepted(6, "1"),
        accepted(7, "1"),
    ];
    let replay = |schema: &Schema| {
        select_key_threshold(
            &mut KeyMemory::new(60, 1.0),
            schema,
            "risk",
            &records,
            counting_accepted(0.5, 1),
        )
    };

    let watched = replay(&log(Some(&["3"])));
    assert_eq!((watched.lookups, watched.candidates), (5, 3));
    assert_eq!(
        watched.chosen.as_ref().map(|c| c.threshold),
        Some(strength(1))
    );
    assert_eq!(
        watched.keys,
        Some(vec![KeyState {
            key: "template: t1".into(),
            lookups: 5,
            answered: 3,
            precision: Some(1.0),
            answering: false,
            answered_weight: 3.0,
            effective_answered: 3.0,
        }])
    );

    let plain = replay(&log(None));
    let state = &plain.keys.unwrap()[0];
    assert_eq!(
        (
            state.lookups,
            state.answered,
            state.precision,
            state.answering
        ),
        (5, 5, Some(1.0), true)
    );
}

/// As [`log`], with the text each line says: lines under one template read alike, so the similar record layer, trusted
/// at any score, answers wherever the key layer does not.
fn written(must_catch: Option<&[&str]>) -> Schema {
    let mut risk = Attribute::new("risk", AttributeRole::Decided);
    risk.key_fields = Some(vec!["template".into()]);
    risk.key_threshold = Some(0.5);
    risk.memory_threshold = Some(0.0);
    risk.must_catch = must_catch.map(|m| m.iter().map(|v| (*v).to_owned()).collect());
    risk.coarse = Some(CoarseLevel {
        classes: Some(BTreeMap::from([
            ("1".to_owned(), "low".to_owned()),
            ("3".to_owned(), "high".to_owned()),
        ])),
        key_threshold: Some(0.0),
        memory_threshold: Some(0.0),
        ..CoarseLevel::default()
    });
    Schema {
        name: "log".into(),
        attributes: vec![
            Attribute::new("template", AttributeRole::Observed),
            Attribute::new("message", AttributeRole::Observed),
            risk,
        ],
    }
}

fn written_lines() -> Vec<Record> {
    lines()
        .into_iter()
        .map(|mut l| {
            let message = format!("disk check {} finished", l.values["template"]);
            l.values.insert("message".into(), message);
            l
        })
        .collect()
}

#[test]
fn choosing_the_similar_record_threshold_does_not_count_lookups_no_layer_would_answer() {
    let layers = |schema: &Schema| {
        select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            schema,
            "risk",
            &written_lines(),
            choice(0.5, 1),
        )
    };
    let plain = layers(&written(None));
    let watched = layers(&written(Some(&["3"])));

    // The t1 lines after the 3 are no layer's to answer: they are not left to the similar record layer either.
    assert_eq!(watched.memory.lookups, plain.memory.lookups);
}

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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A summary observed, a category decided among `votes` nearest records.
fn intake(votes: usize) -> Schema {
    let mut category = Attribute::new("category", AttributeRole::Decided);
    category.similar_votes = votes;
    Schema {
        name: "intake".into(),
        attributes: vec![Attribute::new("summary", AttributeRole::Observed), category],
    }
}

/// Requests whose category mostly follows the first word of their summary.
fn requests(seed: u64, count: i64) -> Vec<Record> {
    const TOPICS: [&str; 5] = ["vpn", "printer", "badge", "laptop", "mail"];
    const WORDS: [&str; 6] = ["broken", "slow", "missing", "stuck", "noisy", "late"];
    let mut draws = Draws(seed);
    (1..=count)
        .map(|n| {
            let topic = draws.below(TOPICS.len());
            let category = if draws.below(5) == 0 {
                draws.below(TOPICS.len())
            } else {
                topic
            };
            let summary = format!(
                "{} {} request {}",
                TOPICS[topic],
                WORDS[draws.below(WORDS.len())],
                draws.below(1_000)
            );
            record(
                "r",
                n,
                &[("summary", &summary), ("category", TOPICS[category])],
                None,
            )
        })
        .collect()
}

/// The similar record layer's replay, and for each of its answers decided by a vote, its score and the similarity of its
/// nearest voter. Summaries are longer than the key memory's keys, so the key layer never answers and every lookup is
/// left to the similar record layer.
fn similar(schema: &Schema, records: &[Record]) -> (gil::replay::ThresholdReplay, Vec<(f64, f64)>) {
    let mut voted = Vec::new();
    let layers = select_layers_with(
        &mut KeyMemory::new(4, 1.0),
        &mut LexicalMemory::default(),
        schema,
        "category",
        records,
        choice(0.01, 1),
        &mut |d: &DecisionRecord| {
            if let Some(look) = d.similar.as_ref().filter(|l| l.voted) {
                voted.push((look.score, look.nearest));
            }
        },
    );
    assert_eq!(layers.key.chosen, None);
    (layers.memory, voted)
}

/// The nearest similarities of the vote-decided answers at or above `threshold`, lowest first.
fn nearest_at(voted: &[(f64, f64)], threshold: f64) -> Vec<f64> {
    let mut nearest: Vec<f64> = voted
        .iter()
        .filter(|(score, _)| *score >= threshold)
        .map(|(_, n)| *n)
        .collect();
    nearest.sort_by(f64::total_cmp);
    nearest
}

#[test]
fn the_floor_is_the_similarity_only_a_hundredth_of_a_thresholds_vote_decided_answers_fall_below() {
    let (memory, voted) = similar(&intake(10), &requests(7, 400));
    let chosen = memory.chosen.unwrap();
    let nearest = nearest_at(&voted, chosen.threshold);
    assert!(
        nearest.len() > 200,
        "{} answers decided by a vote",
        nearest.len()
    );

    let floor = chosen.similarity_floor.unwrap();
    assert!(nearest.contains(&floor));
    let below = nearest.iter().filter(|n| **n < floor).count();
    assert!(
        below >= 1,
        "a hundredth of the answers lies below the floor"
    );
    assert!((below as f64) <= 0.01 * nearest.len() as f64);
    assert!(memory.most_precise.unwrap().similarity_floor.is_some());
}

#[test]
fn with_too_few_answers_for_a_hundredth_the_floor_is_the_lowest_nearest_similarity() {
    let (memory, voted) = similar(&intake(10), &requests(7, 40));
    let chosen = memory.chosen.unwrap();
    let nearest = nearest_at(&voted, chosen.threshold);
    assert!(!nearest.is_empty() && nearest.len() < 101);

    assert_eq!(chosen.similarity_floor, Some(nearest[0]));
}

#[test]
fn a_threshold_no_vote_decided_answer_reaches_has_no_floor() {
    // A single voter's score is its similarity: the threshold is the floor.
    let (one, _) = similar(&intake(1), &requests(7, 40));
    assert_eq!(one.chosen.unwrap().similarity_floor, None);
    assert_eq!(one.most_precise.unwrap().similarity_floor, None);

    // Every request reads the same, so the record memory keeps only the latest: each lookup has a lone voter.
    let same: Vec<Record> = (1..=20)
        .map(|n| {
            record(
                "r",
                n,
                &[
                    ("summary", "vpn drops every few minutes"),
                    ("category", "vpn"),
                ],
                None,
            )
        })
        .collect();
    let (lone, voted) = similar(&intake(10), &same);
    assert!(voted.is_empty());
    let chosen = lone.chosen.unwrap();
    assert_eq!((chosen.precision, chosen.similarity_floor), (1.0, None));
}
