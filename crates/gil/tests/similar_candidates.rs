//! The similar record layer's runner-up values as candidates (experimental) — an attribute that sets `similar_candidates`
//! gets, after the value the vote chose, the values that drew less of the vote, each scored by its share of the vote and
//! never trusted. Off by default, and then the answer, the schema's serialization and the tuning are as they were; the
//! value the vote chose, its score and whether it is trusted do not depend on the setting; the runners-up come in the
//! order the vote ranks them; only a decided attribute with a single value takes the setting.

use std::collections::BTreeMap;

use gil::decider::{Answer, Candidate, Decider, Source};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::{LexicalMemory, Match};
use gil::schema::{Attribute, AttributeRole, Record, Schema};
use gil::similar_vote;

const MINUTE: i64 = 60 * 10_000_000;

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

fn doc(n: i64, pairs: &[(&str, &str)]) -> Record {
    Record {
        id: format!("d{n}"),
        values: values(pairs),
        settled_at: n * MINUTE,
        ..Record::default()
    }
}

/// A note observed, a category decided among the ten nearest records; a threshold of 0 trusts the vote's winner.
fn intake(candidates: usize) -> Schema {
    let mut category = Attribute::new("category", AttributeRole::Decided);
    category.memory_threshold = Some(0.0);
    category.similar_candidates = candidates;
    Schema::new(
        "intake",
        vec![Attribute::new("note", AttributeRole::Observed), category],
    )
    .expect("a valid schema")
}

/// Three outages, two billing records and one refund, all about the same router.
fn history() -> Vec<Record> {
    [
        (
            "router keeps dropping the connection every evening",
            "billing",
        ),
        ("router keeps dropping the connection at night", "outage"),
        ("router keeps dropping connection in the evening", "outage"),
        (
            "the router is dropping the connection every night",
            "outage",
        ),
        ("invoice shows a charge twice this month", "billing"),
        (
            "router connection dropped and the invoice was refunded",
            "refund",
        ),
    ]
    .iter()
    .zip(1..)
    .map(|((note, category), n)| doc(n, &[("note", note), ("category", category)]))
    .collect()
}

const ASKED: &str = "router keeps dropping the connection every evening again";

fn suggest(schema: &Schema) -> Answer {
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(schema, &history());
    decider.answer_one(
        schema,
        "d9",
        "category",
        &values(&[("note", ASKED)]),
        &BTreeMap::new(),
        None,
    )
}

/// The records nearest the asked note, nearest first — what the vote is cast among.
fn nearest(schema: &Schema) -> Vec<Match> {
    let mut memory = LexicalMemory::default();
    let task = Decider::task(schema, "category");
    for record in history() {
        let evidence = schema.evidence("category", &record.values, &BTreeMap::new());
        memory.remember(&task, &record.id, &evidence, &record.values["category"]);
    }
    let asked = schema.evidence("category", &values(&[("note", ASKED)]), &BTreeMap::new());
    memory.nearest(&task, &asked, 10)
}

fn similar(answer: &Answer) -> Vec<&Candidate> {
    answer
        .candidates
        .iter()
        .filter(|c| c.source == Source::SimilarRecord)
        .collect()
}

#[test]
fn off_by_default_the_layer_offers_only_the_value_it_voted_for() {
    for setting in [0, 1] {
        let answer = suggest(&intake(setting));
        assert_eq!(similar(&answer).len(), 1, "setting {setting}");
    }
    assert_eq!(
        Attribute::new("category", AttributeRole::Decided).similar_candidates,
        0
    );
}

#[test]
fn the_value_the_vote_chose_does_not_depend_on_the_setting() {
    let off = suggest(&intake(0));
    let on = suggest(&intake(3));
    let (off_first, on_first) = (&off.candidates[0], &on.candidates[0]);
    assert_eq!(off_first, on_first);
    assert!(on_first.trusted);
    assert_eq!(off.source, on.source);
    assert_eq!(on.candidates.len(), 3, "the winner and two runners-up");
}

#[test]
fn the_runners_up_follow_in_the_order_the_vote_ranks_them_each_by_its_share() {
    let schema = intake(3);
    let answer = suggest(&schema);
    let nearest = nearest(&schema);
    let ranking = similar_vote::ranking(&nearest, 10);

    // The ranking starts with the value `decide` chooses.
    let (winner, _) = similar_vote::decide(&nearest, 10).expect("records voted");
    assert_eq!(ranking[0].0.answer, winner.answer);

    let offered = similar(&answer);
    assert_eq!(
        offered.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
        ranking
            .iter()
            .take(3)
            .map(|(m, _)| m.answer.as_str())
            .collect::<Vec<_>>()
    );
    assert!(!ranking.is_empty() && ranking.len() >= 3);

    // A runner-up's score is its share: the weight of the records voting for it over all the weight cast.
    let total: f64 = nearest.iter().map(|m| m.similarity.max(0.0)).sum();
    for candidate in &offered[1..] {
        let weight: f64 = nearest
            .iter()
            .filter(|m| m.answer == candidate.value)
            .map(|m| m.similarity.max(0.0))
            .sum();
        assert!(
            (candidate.score - weight / total).abs() < 1e-9,
            "{candidate:?}"
        );
        assert!(
            !candidate.trusted,
            "a runner-up is never trusted: {candidate:?}"
        );
    }
    // The shares of all the values sum to one.
    let shares: f64 = ranking.iter().map(|(_, share)| share).sum();
    assert!((shares - 1.0).abs() < 1e-9);
    // Heaviest first.
    assert!(ranking.windows(2).all(|w| w[0].1 >= w[1].1));
}

#[test]
fn a_larger_setting_than_the_values_that_voted_offers_them_all() {
    let answer = suggest(&intake(10));
    assert_eq!(
        similar(&answer).len(),
        3,
        "outage, billing and refund voted; nothing is made up"
    );
}

#[test]
fn the_ranking_agrees_with_decide_on_the_winner_and_takes_ties_to_the_nearer_voter() {
    let m = |source: &str, similarity: f64, answer: &str| Match {
        source: source.into(),
        similarity,
        answer: answer.into(),
    };
    // Two voters together outweigh a nearer one.
    let tied = vec![m("a", 0.9, "x"), m("b", 0.45, "y"), m("c", 0.5, "y")];
    let ranking = similar_vote::ranking(&tied, 10);
    assert_eq!(ranking[0].0.answer, "y", "0.45 + 0.5 outweighs 0.9");
    let level = vec![m("a", 0.6, "x"), m("b", 0.3, "y"), m("c", 0.3, "y")];
    let ranking = similar_vote::ranking(&level, 10);
    assert_eq!(
        ranking[0].0.answer, "x",
        "equal weight: the nearer voter's value first"
    );
    assert_eq!(
        similar_vote::decide(&level, 10).map(|(w, _)| w.answer.clone()),
        Some(ranking[0].0.answer.clone())
    );
    // Nobody similar: the shares are 0, not a division by nothing.
    let none = vec![m("a", -0.5, "x"), m("b", -0.2, "y")];
    assert!(
        similar_vote::ranking(&none, 10)
            .iter()
            .all(|(_, s)| *s == 0.0)
    );
    assert!(similar_vote::ranking(&[], 10).is_empty());
}

#[test]
fn only_a_decided_attribute_with_a_single_value_takes_the_setting() {
    let mut observed = Attribute::new("note", AttributeRole::Observed);
    observed.similar_candidates = 3;
    assert!(
        Schema::new(
            "intake",
            vec![observed, Attribute::new("category", AttributeRole::Decided)]
        )
        .is_err()
    );

    let mut tags = Attribute::new("tags", AttributeRole::Decided);
    tags.multiple = true;
    tags.similar_candidates = 3;
    assert!(
        Schema::new(
            "intake",
            vec![Attribute::new("note", AttributeRole::Observed), tags]
        )
        .is_err()
    );
}

#[test]
fn the_thresholds_a_replay_chooses_do_not_depend_on_the_setting() {
    assert_eq!(
        gil::tuning::fingerprint(&intake(3)),
        gil::tuning::fingerprint(&intake(0)),
        "the setting adds candidates, it does not change what a threshold is chosen on"
    );
}

#[cfg(feature = "serde")]
#[test]
fn the_setting_is_written_only_when_set() {
    let off = serde_json::to_string(&intake(0)).expect("writes");
    assert!(!off.contains("similar_candidates"), "{off}");
    let on = serde_json::to_string(&intake(3)).expect("writes");
    assert!(on.contains("\"similar_candidates\":3"), "{on}");
    let back: Schema = serde_json::from_str(&on).expect("reads");
    assert_eq!(back.attribute("category").similar_candidates, 3);
    let plain: Schema = serde_json::from_str(&off).expect("reads");
    assert_eq!(plain.attribute("category").similar_candidates, 0);
}
