//! The similar record layer's vote and an attribute's coarse level, held to what the reference implementation's similar
//! vote, coarse level and shared evidence tests expect: neighbours that agree outvote a nearer record, with the margin
//! as the score and the nearest voter as the evidence; a single vote lets the nearest record decide on its similarity;
//! records outside the domain do not vote; a vote whose nearest voter is less similar than the floor is a guess, while a
//! single voter has no floor; while typing, the vote is never trusted and records whose value does not begin with the
//! typed text drop out; a replay chooses its thresholds on the scores suggestions carry. A coarse class is offered only
//! when no layer answers the value and nothing is typed: the keys' class at its key threshold, else the vote's at its
//! memory threshold and floor, else a guess — by prefix or by a table; a replay chooses the level's thresholds on the
//! lookups the value layers leave. Several attributes that read the same evidence read one text.
//!
//! The reference tests that wrapped the record memory in one ranking only its nearest record, or embedded texts in
//! batches, have no counterpart: the core's record memory is the lexical memory itself. A level's class of a value and
//! the schema's checks of a vote count or a coarse level are held in `schema.rs` and `schema_check.rs`; the similarity
//! floor a replay chooses in `threshold_choice.rs`.

use std::collections::BTreeMap;

use gil::decider::{Answer, Candidate, Decider, Evidence, Source};
use gil::decision_record::{DecisionRecord, SimilarLook};
use gil::key_memory::{KeyMemory, Lookup};
use gil::lexical_memory::{LexicalMemory, Match};
use gil::replay::{Choice, select_coarse, select_layers};
use gil::schema::{Attribute, AttributeRole, CoarseLevel, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;

fn choice(target_precision: f64, minimum_answered: usize) -> Choice<'static> {
    Choice {
        target_precision,
        minimum_answered,
        counting: None,
    }
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

/// Record `d<n>`, settled `n` minutes after the epoch.
fn doc(n: i64, pairs: &[(&str, &str)]) -> Record {
    Record {
        id: format!("d{n}"),
        values: values(pairs),
        settled_at: n * MINUTE,
        ..Record::default()
    }
}

/// The record a candidate rests on, and its similarity.
fn record_of(candidate: &Candidate) -> (&str, f64) {
    match &candidate.evidence {
        Some(Evidence::Record { id, similarity }) => (id, *similarity),
        other => panic!("a similar record's candidate rests on a record, not {other:?}"),
    }
}

fn similar_candidate(answer: &Answer) -> &Candidate {
    let similar: Vec<&Candidate> = answer
        .candidates
        .iter()
        .filter(|c| c.source == Source::SimilarDocument)
        .collect();
    assert_eq!(similar.len(), 1, "one candidate from the vote");
    similar[0]
}

// The similar record layer's vote.

/// A note observed, a category decided among `votes` nearest records. A threshold of 0 trusts the vote's candidate, so it
/// comes first rather than behind the attribute's most frequent value.
fn intake(votes: usize, candidates: Option<&[&str]>) -> Schema {
    let mut category = Attribute::new("category", AttributeRole::Decided);
    category.memory_threshold = Some(0.0);
    category.similar_votes = votes;
    category.candidates = candidates.map(|c| c.iter().map(|v| (*v).to_owned()).collect());
    Schema {
        name: "intake".into(),
        attributes: vec![Attribute::new("note", AttributeRole::Observed), category],
    }
}

fn with_floor(mut schema: Schema, floor: f64) -> Schema {
    schema.attributes[1].memory_floor = Some(floor);
    schema
}

/// The nearest record says "billing"; the three after it, a little less similar, say "outage".
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
    ]
    .iter()
    .zip(1..)
    .map(|((note, category), n)| doc(n, &[("note", note), ("category", category)]))
    .collect()
}

const ASKED: &str = "router keeps dropping the connection every evening again";

fn suggest(schema: &Schema, typed: Option<&str>) -> Answer {
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(schema, &history());
    decider.answer_one(
        schema,
        "d9",
        "category",
        &values(&[("note", ASKED)]),
        &BTreeMap::new(),
        typed,
    )
}

/// The records nearest the asked note, nearest first, among those the schema admits — what the vote is cast among.
fn nearest(schema: &Schema) -> Vec<Match> {
    let mut memory = LexicalMemory::default();
    let task = Decider::task(schema, "category");
    for record in history() {
        let evidence = schema.evidence("category", &record.values, &BTreeMap::new());
        memory.remember(&task, &record.id, &evidence, &record.values["category"]);
    }
    let asked = schema.evidence("category", &values(&[("note", ASKED)]), &BTreeMap::new());
    memory
        .nearest(&task, &asked, 10)
        .into_iter()
        .filter(|m| schema.attributes[1].admits(&m.answer))
        .collect()
}

#[test]
fn neighbours_that_agree_outvote_a_nearer_record() {
    let schema = intake(10, None);
    let answer = suggest(&schema, None);
    let nearest = nearest(&schema);
    assert_eq!(nearest[0].source, "d1"); // the nearest says billing

    let similar = similar_candidate(&answer);
    assert_eq!(similar.value, "outage");
    // Its evidence is its nearest voter.
    let outage = nearest.iter().find(|m| m.answer == "outage").unwrap();
    assert_eq!(
        record_of(similar),
        (outage.source.as_str(), outage.similarity)
    );

    // The margin: the winner's weight less the runner-up's, over all the weight cast.
    let weight = |answer: &str| -> f64 {
        nearest
            .iter()
            .filter(|m| m.answer == answer)
            .map(|m| m.similarity)
            .sum()
    };
    let total: f64 = nearest.iter().map(|m| m.similarity).sum();
    assert!((similar.score - (weight("outage") - weight("billing")) / total).abs() < 1e-9);
}

#[test]
fn one_vote_lets_the_nearest_record_decide_on_its_similarity() {
    let schema = intake(1, None);
    let answer = suggest(&schema, None);
    let nearest = nearest(&schema);

    let similar = similar_candidate(&answer);
    assert_eq!(similar.value, "billing");
    assert_eq!(record_of(similar), ("d1", nearest[0].similarity));
    assert_eq!(similar.score, nearest[0].similarity);
    assert!(similar.trusted);
}

#[test]
fn records_outside_the_domain_do_not_vote() {
    // Without "outage" in the list only the two billing records vote, and they agree.
    let schema = intake(10, Some(&["billing", "refund"]));
    let answer = suggest(&schema, None);

    let similar = similar_candidate(&answer);
    assert_eq!(similar.value, "billing");
    assert_eq!(similar.score, 1.0);
    assert!(answer.candidates.iter().all(|c| c.value != "outage"));
}

#[test]
fn a_vote_among_records_less_like_the_draft_than_the_floor_is_a_guess() {
    // Neighbours agreeing on "outage" carry the vote past a threshold of 0 — but its nearest voter is less similar than
    // the floor, so the value is offered, not trusted.
    let trusted = suggest(&intake(10, None), None);
    let outage = record_of(similar_candidate(&trusted)).1;
    let floored = suggest(&with_floor(intake(10, None), outage + 1e-9), None);
    let at_floor = suggest(&with_floor(intake(10, None), outage), None);

    assert!(trusted.answered());
    assert_eq!(trusted.candidates[0].value, "outage");
    // Not trusted, the vote is a guess like any below a threshold: offered, possibly behind the attribute's other guesses.
    assert!(!floored.answered());
    assert!(floored.candidates.iter().any(|c| c.value == "outage"));
    assert!(floored.candidates.iter().all(|c| !c.trusted));
    assert!(at_floor.answered());
}

#[test]
fn a_single_voter_has_no_floor() {
    // A single voter's score is its similarity: the threshold is the floor, and no floor applies.
    let lone = suggest(&with_floor(intake(1, None), 2.0), None);

    assert!(lone.answered());
    assert_eq!(lone.source, Source::SimilarDocument);
    assert_eq!(lone.candidates[0].value, "billing");
}

/// The answer for the asked note with `typed` typed, and what the vote came to whatever the threshold.
fn typing(schema: &Schema, typed: &str) -> (Answer, Option<SimilarLook>) {
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(schema, &history());
    let asked = values(&[("note", ASKED)]);
    let q = Lookup::new(schema, "category", &asked)
        .typed(Some(typed))
        .excluding(Some("d9"));
    let mut look = None;
    let answer = decider.answer_one_with(&q, &mut |d: &DecisionRecord| look = d.similar.clone());
    (answer, look)
}

#[test]
fn while_typing_the_vote_is_a_guess_among_records_whose_value_begins_with_the_text() {
    // At a threshold of 0 the vote answers "outage" untyped (see above). Typed "B", the three outage records drop out
    // and the two billing records agree — but no similar record is trusted while a person types.
    let schema = intake(10, None);
    let (billing, look) = typing(&schema, "B");
    let look = look.expect("a vote");
    assert_eq!(
        (look.value.as_str(), look.score, look.voted),
        ("billing", 1.0, true)
    );
    assert!(!billing.answered());
    assert!(
        billing
            .candidates
            .iter()
            .all(|c| c.value == "billing" && !c.trusted)
    );

    let (outage, look) = typing(&schema, "out");
    let look = look.expect("a vote");
    assert_eq!((look.value.as_str(), look.score), ("outage", 1.0));
    assert!(!outage.answered());
    assert!(
        outage
            .candidates
            .iter()
            .all(|c| c.value == "outage" && !c.trusted)
    );

    // Nothing begins with it: no record votes, and nothing is offered.
    let (none, look) = typing(&schema, "x");
    assert!(look.is_none());
    assert!(none.candidates.is_empty());
}

#[test]
fn a_replay_scores_lookups_as_suggestions_do() {
    // The replay asks about each record with the ones before it remembered, as these suggestions do; a threshold is a
    // score some lookup had, so the one it chooses is exactly the score a suggestion for that record carried — on the
    // vote's scale, not the nearest record's similarity.
    let schema = intake(10, None);
    let records = history();
    let mut scores = Vec::new();
    for i in 1..records.len() {
        let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
        decider.rebuild(&schema, &records[..i]);
        let mut asked = records[i].values.clone();
        asked.remove("category");
        let answer = decider.answer_one(
            &schema,
            &records[i].id,
            "category",
            &asked,
            &BTreeMap::new(),
            None,
        );
        scores.push(similar_candidate(&answer).score);
    }

    let replay = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "category",
        &records,
        choice(0.01, 1),
    )
    .memory;

    assert_eq!(replay.candidates, scores.len());
    assert!(scores.contains(&replay.chosen.expect("a threshold").threshold));
    assert!(scores.contains(&replay.most_precise.expect("a most precise").threshold));
}

// The coarse level.

/// A part and a note observed, a code decided with the given coarse level and value thresholds.
fn report(coarse: Option<CoarseLevel>, key_threshold: Option<f64>) -> Schema {
    let mut code = Attribute::new("code", AttributeRole::Decided);
    code.coarse = coarse;
    code.key_threshold = key_threshold;
    Schema {
        name: "report".into(),
        attributes: vec![
            Attribute::new("part", AttributeRole::Observed),
            Attribute::new("note", AttributeRole::Observed),
            code,
        ],
    }
}

fn level(prefix: usize, edit: impl FnOnce(&mut CoarseLevel)) -> Option<CoarseLevel> {
    let mut level = CoarseLevel::by_prefix(prefix);
    edit(&mut level);
    Some(level)
}

/// Flight controls (chapter 27) under three codes, so no single code is backed well; landing gear (32) apart.
fn reports() -> Vec<Record> {
    [
        (
            "aileron",
            "aileron stiff on approach, lubricated quadrant",
            "2710",
        ),
        ("aileron", "aileron cable tension low on approach", "2711"),
        ("aileron", "aileron stiff in roll, lubricated hinge", "2712"),
        (
            "elevator",
            "elevator stiff on approach, lubricated hinge",
            "2730",
        ),
        (
            "gear",
            "main gear did not retract, replaced actuator",
            "3230",
        ),
        (
            "gear",
            "nose gear shimmy on landing, replaced damper",
            "3250",
        ),
    ]
    .iter()
    .zip(1..)
    .map(|((part, note, code), n)| doc(n, &[("part", part), ("note", note), ("code", code)]))
    .collect()
}

/// The systems the codes belong to, named as a table would name them rather than by their leading digits.
fn systems() -> BTreeMap<String, String> {
    [
        ("2710", "flight controls"),
        ("2711", "flight controls"),
        ("2712", "flight controls"),
        ("2730", "flight controls"),
        ("3230", "landing gear"),
        ("3250", "landing gear"),
    ]
    .iter()
    .map(|(code, system)| ((*code).to_owned(), (*system).to_owned()))
    .collect()
}

fn by_systems(edit: impl FnOnce(&mut CoarseLevel)) -> Option<CoarseLevel> {
    let mut level = CoarseLevel {
        classes: Some(systems()),
        ..CoarseLevel::default()
    };
    edit(&mut level);
    Some(level)
}

fn aileron() -> BTreeMap<String, String> {
    values(&[
        ("part", "aileron"),
        ("note", "aileron stiff on approach again"),
    ])
}

fn suggest_code(schema: &Schema, asked: &BTreeMap<String, String>, typed: Option<&str>) -> Answer {
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(schema, &reports());
    decider.answer_one(schema, "d9", "code", asked, &BTreeMap::new(), typed)
}

fn coarse_of(answer: &Answer) -> &Candidate {
    answer.coarse.as_ref().expect("a coarse class")
}

#[test]
fn the_class_the_keys_back_is_offered_when_no_layer_answers_the_value() {
    // The keys split "aileron" three ways, so no code is trusted; together they back chapter 27.
    let answer = suggest_code(
        &report(level(2, |l| l.key_threshold = Some(0.5)), None),
        &aileron(),
        None,
    );

    assert!(!answer.answered());
    let coarse = coarse_of(&answer);
    assert_eq!(coarse.value, "27");
    assert_eq!(coarse.source, Source::SettledFieldMemory);
    assert!(coarse.trusted);
    assert_eq!(
        coarse.evidence.as_ref().map(ToString::to_string).as_deref(),
        Some("part: aileron")
    );
    // Candidates stay settled values.
    assert!(answer.candidates.iter().all(|c| c.value.len() == 4));
}

#[test]
fn where_the_keys_do_not_reach_their_threshold_the_vote_of_similar_records_can() {
    let answer = suggest_code(
        &report(
            level(2, |l| {
                l.key_threshold = Some(100.0);
                l.memory_threshold = Some(0.0);
            }),
            None,
        ),
        &aileron(),
        None,
    );

    let coarse = coarse_of(&answer);
    assert_eq!(coarse.value, "27");
    assert_eq!(coarse.source, Source::SimilarDocument);
    assert!(coarse.trusted);
    assert!(record_of(coarse).0.starts_with('d'));
}

#[test]
fn without_a_threshold_met_the_class_is_a_guess() {
    let answer = suggest_code(&report(level(2, |_| {}), None), &aileron(), None);

    let coarse = coarse_of(&answer);
    assert_eq!(coarse.value, "27");
    assert!(!coarse.trusted);
}

#[test]
fn a_class_voted_among_records_less_like_the_draft_than_the_floor_is_a_guess() {
    let answer = suggest_code(
        &report(
            level(2, |l| {
                l.memory_threshold = Some(0.0);
                l.memory_floor = Some(2.0);
            }),
            None,
        ),
        &values(&[("note", "aileron stiff on approach again")]),
        None,
    );

    let coarse = coarse_of(&answer);
    assert_eq!(coarse.source, Source::SimilarDocument);
    assert!(!coarse.trusted);
}

#[test]
fn no_class_is_offered_when_a_layer_answers_the_value_or_while_typing() {
    let answered = suggest_code(
        &report(level(2, |l| l.key_threshold = Some(0.0)), Some(0.0)),
        &aileron(),
        None,
    );
    let typing = suggest_code(
        &report(level(2, |l| l.key_threshold = Some(0.0)), None),
        &aileron(),
        Some("2"),
    );
    let none = suggest_code(&report(None, None), &aileron(), None);

    assert!(answered.answered());
    assert!(answered.coarse.is_none());
    assert!(typing.coarse.is_none());
    assert!(none.coarse.is_none());
}

#[test]
fn a_replay_chooses_the_level_s_thresholds_on_the_scores_suggestions_carry() {
    // Each record asked about with the ones before it remembered, as the replay asks: the threshold it chooses for the
    // keys' class is a score some such suggestion carried for the level.
    let schema = report(level(2, |_| {}), None);
    let records = reports();
    let mut key_scores = Vec::new();
    for i in 1..records.len() {
        let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
        decider.rebuild(&schema, &records[..i]);
        let mut asked = records[i].values.clone();
        asked.remove("code");
        let answer = decider.answer_one(
            &schema,
            &records[i].id,
            "code",
            &asked,
            &BTreeMap::new(),
            None,
        );
        if let Some(keys) = answer
            .coarse
            .filter(|c| c.source == Source::SettledFieldMemory)
        {
            key_scores.push(keys.score);
        }
    }

    let layers = select_coarse(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "code",
        &records,
        choice(0.01, 1),
    );

    let chosen = layers.key.chosen.expect("a threshold");
    assert!(key_scores.contains(&chosen.threshold));
    assert_eq!(layers.key.candidates, key_scores.len());
}

#[test]
fn a_replay_leaves_the_lookups_the_value_layers_answered_out() {
    // A key threshold of 0 answers every lookup the keys reach; only the first, before any key, is left.
    let layers = |schema: &Schema| {
        select_coarse(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            schema,
            "code",
            &reports(),
            choice(0.5, 1),
        )
    };
    let none = layers(&report(level(2, |_| {}), Some(0.0)));
    let all = layers(&report(level(2, |_| {}), None));

    assert!(none.key.lookups < all.key.lookups);
    assert_eq!(all.key.lookups, reports().len() - 1);
}

#[test]
fn a_table_of_classes_offers_the_class_it_names() {
    let answer = suggest_code(
        &report(by_systems(|l| l.key_threshold = Some(0.5)), None),
        &aileron(),
        None,
    );

    assert!(!answer.answered());
    let coarse = coarse_of(&answer);
    assert_eq!(coarse.value, "flight controls");
    assert!(coarse.trusted);
    // The settled values stay as they were.
    assert!(answer.candidates.iter().all(|c| c.value.len() == 4));
}

#[test]
fn a_replay_counts_a_class_right_by_the_table() {
    // Each part keeps to one system but never repeats a code: by the table every class the keys back is right, while
    // with each code a class of its own none is.
    let layers = |coarse: Option<CoarseLevel>| {
        select_coarse(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &report(coarse, None),
            "code",
            &reports(),
            choice(0.5, 1),
        )
    };
    let by_system = layers(by_systems(|_| {}));
    let by_code = layers(level(4, |_| {}));

    let chosen = by_system.key.chosen.expect("a threshold");
    assert_eq!(chosen.precision, 1.0);
    assert_eq!(chosen.answered, by_system.key.candidates);
    assert_eq!(by_code.key.candidates, by_system.key.candidates);
    assert!(by_code.key.chosen.is_none());
    assert_eq!(
        by_code.key.most_precise.expect("a most precise").precision,
        0.0
    );
}

// Shared evidence.

/// Two observed attributes and three decided ones, the decided ones evidence for one another or not.
fn session(judged_as_evidence: bool) -> Schema {
    let topic = |name: &str| {
        let mut a = Attribute::new(name, AttributeRole::Decided);
        a.use_as_evidence = judged_as_evidence;
        a
    };
    Schema {
        name: "session".into(),
        attributes: vec![
            Attribute::new("narrative", AttributeRole::Observed),
            Attribute::new("grade", AttributeRole::Observed),
            topic("topic0"),
            topic("topic1"),
            topic("topic2"),
        ],
    }
}

#[test]
fn decided_attributes_that_are_no_evidence_for_one_another_read_one_text() {
    let separate = session(false);
    let asked = values(&[("narrative", "a note never seen"), ("grade", "1")]);
    for topic in ["topic0", "topic1", "topic2"] {
        assert_eq!(
            separate.evidence(topic, &asked, &BTreeMap::new()),
            "narrative: a note never seen\ngrade: 1"
        );
    }

    // As evidence for one another, each reads the others' settled values: a different text each.
    let together = session(true);
    let settled = values(&[
        ("narrative", "session note 1"),
        ("grade", "1"),
        ("topic0", "a1"),
        ("topic1", "b1"),
        ("topic2", "c1"),
    ]);
    let texts: Vec<String> = ["topic0", "topic1", "topic2"]
        .iter()
        .map(|t| together.evidence(t, &settled, &BTreeMap::new()))
        .collect();
    assert_ne!(texts[0], texts[1]);
    assert_ne!(texts[1], texts[2]);
    assert_ne!(texts[0], texts[2]);
}
