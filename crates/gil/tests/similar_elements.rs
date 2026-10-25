//! The similar record layer of an attribute that takes several values (experimental) — each element offered by its share
//! of the nearest records that hold it, a threshold chosen on the elements a replay offered, a promise about one element
//! at a time. Off by default, and then the attribute, the keeper and the tuning are as they were; where the texts decide the
//! elements the threshold stands and the elements offered keep the promise on records it was not chosen on; where they do
//! not, none stands and nothing is offered; and a keeper that carries a replay on chooses what one that starts over does.

use std::collections::BTreeMap;

use gil::decider::Decider;
use gil::keeper::{Keep, Keeper};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::{LexicalMemory, Match};
use gil::replay::{Choice, select_similar_elements};
use gil::schema::{Attribute, AttributeRole, Record, Schema};
use gil::similar_vote::{ELEMENT_SEPARATOR, element_shares};

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

/// A free text and the tags it is given; with `elements`, the tags are answered by similar records.
fn desk(elements: bool) -> Schema {
    let mut tags = Attribute::new("tags", AttributeRole::Decided);
    tags.multiple = true;
    tags.similar_elements = elements;
    Schema::new(
        "desk",
        vec![Attribute::new("text", AttributeRole::Observed), tags],
    )
    .expect("a valid schema")
}

fn record(number: i64, text: String, tags: Vec<String>) -> Record {
    Record {
        id: format!("r{number:05}"),
        values: BTreeMap::from([("text".to_owned(), text)]),
        sets: BTreeMap::from([("tags".to_owned(), tags)]),
        settled_at: number * MINUTE,
        ..Record::default()
    }
}

/// Texts that name their topic among noise words; a topic's two own tags are always given and one of thirty other tags
/// sometimes — the tags are decided by the text.
fn decided_by_text(count: usize, seed: u64) -> Vec<Record> {
    let mut draws = Draws(seed);
    (1..=count)
        .map(|n| {
            let topic = draws.below(8);
            let noise: Vec<String> = (0..4).map(|_| format!("w{}", draws.below(300))).collect();
            let mut tags = vec![format!("core{topic}a"), format!("core{topic}b")];
            if draws.unit() < 0.4 {
                tags.push(format!("extra{}", draws.below(30)));
            }
            record(
                n as i64,
                format!("topic{topic}x {} topic{topic}y", noise.join(" ")),
                tags,
            )
        })
        .collect()
}

/// Texts of noise words and tags drawn apart from them — nothing in a text says what its tags are.
fn apart_from_text(count: usize, seed: u64) -> Vec<Record> {
    let mut draws = Draws(seed);
    (1..=count)
        .map(|n| {
            let noise: Vec<String> = (0..6).map(|_| format!("w{}", draws.below(300))).collect();
            let tags: Vec<String> = (0..1 + draws.below(3))
                .map(|_| format!("tag{}", draws.below(40)))
                .collect();
            record(n as i64, noise.join(" "), tags)
        })
        .collect()
}

fn text_of(record: &Record) -> BTreeMap<String, String> {
    record.values.clone()
}

/// A keeper that chose on `records`, and the decider it was given them in.
fn kept(schema: Schema, records: &[Record], target: f64) -> (Keeper, Decider) {
    let mut keeper = Keeper::new(schema, Keep::new(target));
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    keeper.settle(&mut decider, records);
    keeper.rechoose();
    (keeper, decider)
}

#[test]
fn it_is_off_unless_the_attribute_asks() {
    let attribute = Attribute::new("tags", AttributeRole::Decided);
    assert!(!attribute.similar_elements);

    let records = decided_by_text(300, 1);
    let (keeper, _) = kept(desk(false), &records, 0.9);
    assert_eq!(keeper.attributes().count(), 0, "a set is not chosen for");
    assert!(keeper.chosen("tags").is_none());
    assert_eq!(keeper.schema().attribute("tags").memory_threshold, None);
    assert_eq!(
        keeper.tuning().attributes[0].memory_threshold,
        None,
        "nothing is stored for it"
    );
    assert_eq!(keeper.tuning().attributes[0].memory_promise, None);
}

#[test]
fn a_set_takes_a_memory_threshold_only_when_it_is_answered_by_similar_elements() {
    let build = |elements: bool, threshold: Option<f64>| {
        let mut tags = Attribute::new("tags", AttributeRole::Decided);
        tags.multiple = true;
        tags.similar_elements = elements;
        tags.memory_threshold = threshold;
        Schema::new(
            "desk",
            vec![Attribute::new("text", AttributeRole::Observed), tags],
        )
    };
    assert!(build(false, Some(0.5)).is_err(), "as it always was");
    assert!(build(true, Some(0.5)).is_ok());
    assert!(build(true, None).is_ok());
    assert!(build(false, None).is_ok());

    let mut one = Attribute::new("kind", AttributeRole::Decided);
    one.similar_elements = true;
    let refused = Schema::new("desk", vec![one]).expect_err("a single value has no elements");
    assert!(refused.contains("takes several values"), "{refused}");
    let mut observed = Attribute::new("text", AttributeRole::Observed);
    observed.similar_elements = true;
    let tags = {
        let mut t = Attribute::new("tags", AttributeRole::Decided);
        t.multiple = true;
        t
    };
    assert!(Schema::new("desk", vec![observed, tags]).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn the_setting_is_written_only_when_set() {
    let off = serde_json::to_string(&desk(false)).expect("writes");
    assert!(!off.contains("similar_elements"), "{off}");
    let on = serde_json::to_string(&desk(true)).expect("writes");
    assert!(on.contains("\"similar_elements\":true"), "{on}");
    let back: Schema = serde_json::from_str(&on).expect("reads");
    assert!(back.attribute("tags").similar_elements);
    let plain: Schema = serde_json::from_str(&off).expect("reads");
    assert!(!plain.attribute("tags").similar_elements);
    assert_ne!(
        gil::tuning::fingerprint(&desk(true)),
        gil::tuning::fingerprint(&desk(false)),
        "a choice made without the layer does not stand for a schema with it"
    );
}

fn nearest(similarity: f64, answer: &str) -> Match {
    Match {
        source: "r".into(),
        similarity,
        answer: answer.to_owned(),
    }
}

#[test]
fn an_elements_share_is_the_similarity_of_the_records_holding_it_over_the_similarity_of_all() {
    let sep = ELEMENT_SEPARATOR;
    let found = [
        nearest(0.8, &format!("a{sep}b")),
        nearest(0.2, &format!("a{sep}c")),
        nearest(-0.5, "d"), // a negative similarity counts as none
        nearest(0.5, "z"),  // beyond the votes
    ];
    let shares = element_shares(&found, 3);
    assert_eq!(
        shares,
        vec![
            ("a".to_owned(), 1.0),
            ("b".to_owned(), 0.8),
            ("c".to_owned(), 0.2),
            ("d".to_owned(), 0.0),
        ]
    );
    assert!(
        element_shares(&[nearest(0.0, "a")], 5).is_empty(),
        "no similarity, no vote"
    );
    assert!(element_shares(&[], 5).is_empty());
}

#[test]
fn where_the_text_decides_the_tags_a_threshold_stands_and_the_elements_offered_keep_the_promise() {
    let records = decided_by_text(1_500, 7);
    let (learned, held) = records.split_at(1_000);
    let (keeper, decider) = kept(desk(true), learned, 0.9);

    let schema = keeper.schema();
    let threshold = schema
        .attribute("tags")
        .memory_threshold
        .expect("a threshold stands");
    let chosen = keeper.chosen("tags").expect("chosen");
    assert_eq!(chosen.key.chosen, None, "the key layer is not replayed");
    let memory = chosen.memory.chosen.as_ref().expect("chosen");
    assert_eq!(memory.threshold, threshold);
    assert!(memory.answered >= 10);
    let promise = keeper
        .tuning()
        .promise_of("tags", gil::decider::Source::SimilarRecord)
        .expect("a promise was recorded");
    assert!(promise > 0.8, "{promise}");

    // On records the threshold was not chosen on: each element offered is right or wrong on its own.
    let (mut offered, mut right, mut settled, mut found) = (0_usize, 0_usize, 0_usize, 0_usize);
    for held in held {
        let answers =
            keeper.similar_elements(&decider, &held.id, "tags", &text_of(held), &BTreeMap::new());
        let truth = &held.sets["tags"];
        settled += truth.len();
        for e in &answers {
            assert!(e.share >= threshold);
            assert_eq!(e.promise, Some(promise));
            offered += 1;
            if truth.contains(&e.value) {
                right += 1;
            }
        }
        found += truth
            .iter()
            .filter(|t| answers.iter().any(|e| &e.value == *t))
            .count();
        assert!(
            answers.windows(2).all(|w| w[0].share >= w[1].share),
            "most shared first"
        );
    }
    let precision = right as f64 / offered as f64;
    assert!(offered > 500, "{offered} elements offered");
    assert!(
        precision >= 0.9,
        "element precision {precision} on {offered}"
    );
    assert!(
        found as f64 / settled as f64 > 0.5,
        "{found} of {settled} settled elements offered"
    );
}

#[test]
fn where_the_text_says_nothing_of_the_tags_no_threshold_stands_and_nothing_is_offered() {
    let records = apart_from_text(1_200, 3);
    let (learned, held) = records.split_at(1_000);
    let (keeper, decider) = kept(desk(true), learned, 0.9);
    assert_eq!(keeper.schema().attribute("tags").memory_threshold, None);
    assert!(
        keeper
            .chosen("tags")
            .expect("replayed")
            .memory
            .chosen
            .is_none()
    );
    assert_eq!(keeper.tuning().attributes[0].memory_promise, None);
    for held in held {
        let answers =
            keeper.similar_elements(&decider, &held.id, "tags", &text_of(held), &BTreeMap::new());
        assert!(answers.is_empty());
    }
}

#[test]
fn a_keeper_that_carries_on_chooses_what_one_that_starts_over_does() {
    let records = decided_by_text(900, 11);
    let whole = {
        let (keeper, _) = kept(desk(true), &records, 0.9);
        keeper
    };

    // In three goes, choosing in between: each carries the replay on from where it stood.
    let mut keeper = Keeper::new(desk(true), Keep::new(0.9));
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    for part in records.chunks(300) {
        keeper.settle(&mut decider, part);
        keeper.rechoose();
    }
    assert_eq!(keeper.chosen("tags"), whole.chosen("tags"));
    assert_eq!(keeper.tuning(), whole.tuning());
    assert_eq!(
        keeper.schema().attribute("tags").memory_threshold,
        whole.schema().attribute("tags").memory_threshold
    );

    // A record settled before the last the replay went through starts it over, and the choice is the same.
    let mut late = records[100].clone();
    late.id = "r00100b".into();
    late.settled_at = records[100].settled_at + 1;
    let mut with_late = records.clone();
    with_late.push(late.clone());
    let mut shuffled = Keeper::new(desk(true), Keep::new(0.9));
    let mut other = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    shuffled.settle(&mut other, &records);
    shuffled.rechoose();
    shuffled.settle(&mut other, std::slice::from_ref(&late));
    shuffled.rechoose();
    let (fresh, _) = kept(desk(true), &with_late, 0.9);
    assert_eq!(shuffled.chosen("tags"), fresh.chosen("tags"));
    assert_eq!(shuffled.tuning(), fresh.tuning());

    // The replay alone is the same as the keeper's.
    let mut memory = LexicalMemory::default();
    let replay = select_similar_elements(
        &mut memory,
        &desk(true),
        "tags",
        &records,
        Choice {
            target_precision: 0.9,
            minimum_answered: 10,
            counting: None,
        },
    );
    assert_eq!(Some(&replay), whole.chosen("tags").map(|c| &c.memory));
}

#[test]
fn a_record_settled_again_replaces_what_it_was_and_a_record_removed_is_never_offered() {
    let mut tags = Attribute::new("tags", AttributeRole::Decided);
    tags.multiple = true;
    tags.similar_elements = true;
    tags.memory_threshold = Some(0.5);
    let schema = Schema::new(
        "desk",
        vec![Attribute::new("text", AttributeRole::Observed), tags],
    )
    .expect("a valid schema");
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    let first = record(1, "alpha beta".into(), vec!["a".into(), "b".into()]);
    let second = record(2, "alpha beta gamma".into(), vec!["a".into(), "c".into()]);
    decider.rebuild(&schema, &[first.clone(), second]);
    let ask = |decider: &Decider| -> Vec<(String, f64)> {
        decider
            .similar_elements(&schema, "asked", "tags", &text_of(&first), &BTreeMap::new())
            .into_iter()
            .map(|e| (e.value, e.share))
            .collect()
    };
    let names = |shares: &[(String, f64)]| shares.iter().map(|s| s.0.clone()).collect::<Vec<_>>();

    // The nearer record holds b, the other c: a is held by both, so its share is the whole vote.
    let shares = ask(&decider);
    assert_eq!(names(&shares), ["a", "b"]);
    assert_eq!(shares[0].1, 1.0);

    // Settled again with other tags, the record is what it now says.
    let again = Record {
        settled_at: 3 * MINUTE,
        sets: BTreeMap::from([("tags".to_owned(), vec!["z".to_owned()])]),
        ..first.clone()
    };
    decider.rebuild(&schema, &[again]);
    assert_eq!(names(&ask(&decider)), ["z"]);

    // Removed, it votes no more.
    decider.remove(&schema, &first.id);
    let shares = ask(&decider);
    assert_eq!(names(&shares), ["a", "c"]);
    assert_eq!(shares[0].1, 1.0);
}

#[test]
fn a_record_is_not_its_own_voter_and_the_set_chosen_so_far_is_not_evidence() {
    let records = decided_by_text(600, 13);
    let (keeper, decider) = kept(desk(true), &records, 0.9);
    let one = &records[300];
    let asked = |sets: &BTreeMap<String, Vec<String>>, id: &str| {
        keeper.similar_elements(&decider, id, "tags", &text_of(one), sets)
    };
    // Asked as itself it is passed over; asked as a stranger it votes for itself.
    let as_itself = asked(&BTreeMap::new(), &one.id);
    let as_stranger = asked(&BTreeMap::new(), "stranger");
    let share = |answers: &[gil::keeper::ElementAnswer], v: &str| {
        answers.iter().find(|e| e.value == v).map(|e| e.share)
    };
    let core = format!(
        "core{}a",
        one.values["text"]
            .trim_start_matches("topic")
            .chars()
            .next()
            .unwrap()
    );
    assert!(share(&as_stranger, &core).unwrap_or(0.0) >= share(&as_itself, &core).unwrap_or(0.0));
    // What was chosen of the attribute so far changes nothing.
    assert_eq!(asked(&one.sets, &one.id), as_itself);
}

#[test]
#[should_panic(expected = "is not answered by similar elements")]
fn asking_an_attribute_that_is_not_answered_by_similar_elements_is_a_contract_violation() {
    let records = decided_by_text(50, 2);
    let (keeper, decider) = kept(desk(false), &records, 0.9);
    let _ = keeper.similar_elements(
        &decider,
        "r",
        "tags",
        &text_of(&records[0]),
        &BTreeMap::new(),
    );
}
