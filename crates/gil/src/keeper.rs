//! The promise-keeping loop as one object: settled records go in once, the decider learns them, and the thresholds are
//! chosen again when they are stale — on a replay carried on from where the last choice left it, not from the start.
//!
//! Without it a host settles each record twice (into the decider and into the records it will replay), asks
//! [`Tuning::stale`] itself, replays every record again when the choice is stale — the cost grows with the square of the
//! records — and puts the choice back on the schema in the right order. A [`Keeper`] holds the records and, for each
//! decided attribute it keeps, the replay as it stands: [`Keeper::settle`] puts records into the host's decider and into
//! the next choice, [`Keeper::stale`] says when the choice no longer stands, and [`Keeper::rechoose`] replays only the
//! records settled since and chooses again — the same thresholds, to the bit, as [`crate::replay::select_layers`] on all
//! the records. An attribute with a coarse level has the level's thresholds chosen on the same replay, after its value
//! layers' — the same, to the bit, as [`crate::replay::select_coarse`] on all the records with the value thresholds just
//! chosen — so a level costs no second replay. The host answers with its decider and [`Keeper::schema`]; an answer's
//! promise is [`Keeper::tuning`]'s.
//! The decider stays the host's, so that records being filled in ([`crate::open_record::OpenRecord`]) use it as before;
//! give the keeper the same decider every time.
//!
//! What it keeps is the choosing and the promise. Whether to check answers again, how often and where to send them stay
//! with the host. Nothing is stored outside the process: a keeper made at start-up replays the saved records once, from
//! the start.

use std::collections::BTreeMap;

use crate::decider::Decider;
use crate::decision_record::DecisionRecord;
use crate::key_memory::KeyMemory;
use crate::lexical_memory::LexicalMemory;
use crate::replay::{
    Choice, LayerThresholds, ReplayState, Replaying, ThresholdRule, batch_check, carry_on,
    coarse_layers, layers,
};
use crate::schema::{AttributeRole, Record, Schema, SettlementKind};
use crate::tuning::{Stale, Tuning};

/// What a keeper chooses thresholds for — [`Choice`] with the kinds owned, and the rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Keep {
    /// The precision the answers must meet, in (0, 1].
    pub target_precision: f64,
    /// The fewest independent answers a threshold may rest on, at least 1.
    pub minimum_answered: usize,
    /// The kinds of settlement whose lookups are counted — `None` counts every settlement.
    pub counting: Option<Vec<SettlementKind>>,
    /// How the two thresholds are chosen.
    pub rule: ThresholdRule,
}

impl Keep {
    /// Thresholds for `target_precision`, resting on at least ten answers, counting every settlement, by
    /// [`ThresholdRule::EveryBand`].
    pub fn new(target_precision: f64) -> Self {
        Self {
            target_precision,
            minimum_answered: 10,
            counting: None,
            rule: ThresholdRule::EveryBand,
        }
    }

    fn choice(&self) -> Choice<'_> {
        Choice {
            target_precision: self.target_precision,
            minimum_answered: self.minimum_answered,
            counting: self.counting.as_deref(),
        }
    }
}

/// One attribute's replay as it stands: its memories, its steps, and how many of the keeper's records it has gone
/// through.
struct Replay {
    key_memory: KeyMemory,
    memory: LexicalMemory,
    state: ReplayState,
    through: usize,
}

/// The settled records and the replays the thresholds are chosen on, for the decided attributes of one schema that take
/// one value. Not thread-safe.
pub struct Keeper {
    schema: Schema,
    keep: Keep,
    threads: Option<usize>,
    records: Vec<Record>,
    replays: BTreeMap<String, Replay>,
    chosen: BTreeMap<String, LayerThresholds>,
    coarse: BTreeMap<String, LayerThresholds>,
    tuning: Tuning,
}

impl Keeper {
    /// A keeper of `schema` with nothing settled, choosing for every decided attribute that takes one value. The
    /// thresholds `schema` holds stand until the first [`Keeper::rechoose`]; a keeper with nothing chosen is stale as
    /// soon as a record is settled.
    ///
    /// # Panics
    /// When `keep` asks for a target outside (0, 1] or fewer than one answer.
    pub fn new(schema: Schema, keep: Keep) -> Self {
        keep.choice().check();
        let tuning = Tuning::capture(&schema, 0, keep.target_precision);
        let mut keeper = Self {
            schema,
            keep,
            threads: None,
            records: Vec::new(),
            replays: BTreeMap::new(),
            chosen: BTreeMap::new(),
            coarse: BTreeMap::new(),
            tuning,
        };
        keeper.restart();
        keeper
    }

    /// The keeper with a similar-record lookup in its replays split among at most `threads` threads — see
    /// [`LexicalMemory::with_threads`]. The choice is the same whatever the number. Set it before settling anything.
    ///
    /// # Panics
    /// When `threads` is 0, or records were settled already.
    pub fn with_threads(mut self, threads: usize) -> Self {
        assert!(threads >= 1, "threads must be at least 1");
        assert!(
            self.records.is_empty(),
            "set the threads before settling records"
        );
        self.threads = Some(threads);
        self.restart();
        self
    }

    /// The schema as chosen: the one given, with the thresholds the last [`Keeper::rechoose`] put on its attributes.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The last choice as a tuning, with each attribute's promises — [`Tuning::promise_of`] gives an answer's.
    pub fn tuning(&self) -> &Tuning {
        &self.tuning
    }

    /// What the last [`Keeper::rechoose`] chose for `attribute`, with how it did on the replay; `None` before the first,
    /// and for an attribute the keeper does not choose for.
    pub fn chosen(&self, attribute: &str) -> Option<&LayerThresholds> {
        self.chosen.get(attribute)
    }

    /// What the last [`Keeper::rechoose`] chose for `attribute`'s coarse level; `None` before the first, and for an
    /// attribute with no coarse level or that the keeper does not choose for.
    pub fn chosen_coarse(&self, attribute: &str) -> Option<&LayerThresholds> {
        self.coarse.get(attribute)
    }

    /// The attributes the keeper chooses thresholds for.
    pub fn attributes(&self) -> impl Iterator<Item = &str> {
        self.replays.keys().map(String::as_str)
    }

    /// How many records were settled, a record settled again counting again — what staleness is counted on.
    pub fn settled(&self) -> usize {
        self.records.len()
    }

    /// Takes settled records: `decider` learns them ([`Decider::rebuild`]) and the next choice replays them. Records may
    /// come in any order; one settled before the last a replay has gone through has the next choice replay everything
    /// from the start.
    pub fn settle(&mut self, decider: &mut Decider, records: &[Record]) {
        decider.rebuild(&self.schema, records);
        self.records.extend_from_slice(records);
    }

    /// Takes a record the host deleted out of `decider` ([`Decider::remove`]) and out of the records; the next choice
    /// replays from the start, without it.
    pub fn remove(&mut self, decider: &mut Decider, record: &str) {
        decider.remove(&self.schema, record);
        self.records.retain(|r| r.id != record);
        self.restart();
    }

    /// Why the last choice no longer stands, or `None` while it does — [`Tuning::stale`] on the records settled.
    pub fn stale(&self) -> Option<Stale> {
        self.tuning.stale(&self.schema, self.records.len())
    }

    /// Chooses every attribute's thresholds again on all the records settled — replaying only those settled since the
    /// last choice where it can — puts them on the schema and captures the tuning with their promises.
    pub fn rechoose(&mut self) -> &Tuning {
        self.rechoose_with(&mut |_| {})
    }

    /// As [`Keeper::rechoose`], handing `sink` the decision record of every record the replays go through this time, in
    /// settlement order, attribute by attribute. For an attribute with a coarse level the record says what the level made
    /// of it ([`DecisionRecord::coarse`]), with whether the value layers answered first at the thresholds the schema held
    /// when the record was replayed.
    pub fn rechoose_with(&mut self, sink: &mut dyn FnMut(&DecisionRecord)) -> &Tuning {
        if self.replays.values().any(|r| !self.carries_on(r)) {
            self.restart();
        }

        let choice = self.keep.choice();
        let mut schema = self.schema.clone();
        let mut chosen = BTreeMap::new();
        let mut coarse = BTreeMap::new();
        for (attribute, replay) in &mut self.replays {
            let definition = self.schema.attribute(attribute);
            let replaying = if definition.coarse.is_some() {
                Replaying::Coarse(&mut *sink)
            } else {
                Replaying::Values(&mut *sink)
            };
            carry_on(
                &mut replay.state,
                &mut replay.key_memory,
                &mut replay.memory,
                &self.schema,
                attribute,
                &self.records[replay.through..],
                choice.counting,
                replaying,
            );
            replay.through = self.records.len();
            let thresholds = layers(
                &replay.state.steps,
                replay.state.order,
                definition,
                choice,
                self.keep.rule,
            );
            let batches = batch_check(
                &replay.state.steps,
                replay.state.order,
                definition,
                choice,
                self.keep.rule,
                &thresholds,
            );
            let thresholds = LayerThresholds {
                batches,
                ..thresholds
            };
            schema = schema
                .with_layers(attribute, &thresholds)
                .expect("a chosen threshold fits the attribute it was chosen for");
            chosen.insert(attribute.clone(), thresholds);
            if definition.coarse.is_some() {
                // On the lookups the value layers leave at the thresholds just chosen.
                let level = coarse_layers(
                    &replay.state.steps,
                    replay.state.order,
                    schema.attribute(attribute),
                    choice,
                );
                schema = schema
                    .with_coarse(attribute, &level)
                    .expect("a chosen threshold fits the level it was chosen for");
                coarse.insert(attribute.clone(), level);
            }
        }

        let mut tuning = Tuning::capture(&schema, self.records.len(), self.keep.target_precision);
        for (attribute, thresholds) in &chosen {
            tuning.promise(attribute, thresholds);
        }
        for (attribute, level) in &coarse {
            tuning.promise_coarse(attribute, level);
        }
        self.schema = schema;
        self.chosen = chosen;
        self.coarse = coarse;
        self.tuning = tuning;
        &self.tuning
    }

    /// Whether the records not yet replayed all come after the last the replay went through.
    fn carries_on(&self, replay: &Replay) -> bool {
        let Some((at, id)) = &replay.state.last else {
            return true;
        };
        self.records[replay.through..]
            .iter()
            .all(|r| (r.settled_at, r.id.as_str()) > (*at, id.as_str()))
    }

    /// Every replay back to the start, with empty memories.
    fn restart(&mut self) {
        let attributes: Vec<String> = self
            .schema
            .attributes()
            .iter()
            .filter(|a| a.role == AttributeRole::Decided && !a.multiple)
            .map(|a| a.name.clone())
            .collect();
        self.replays = attributes
            .into_iter()
            .map(|attribute| {
                let replay = Replay {
                    key_memory: KeyMemory::default(),
                    memory: self.lexical(),
                    state: ReplayState::default(),
                    through: 0,
                };
                (attribute, replay)
            })
            .collect();
    }

    fn lexical(&self) -> LexicalMemory {
        match self.threads {
            Some(threads) => LexicalMemory::default().with_threads(threads),
            None => LexicalMemory::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Keep, Keeper};
    use crate::decider::{Decider, Source};
    use crate::key_memory::KeyMemory;
    use crate::lexical_memory::LexicalMemory;
    use crate::replay::{LayerOptions, ThresholdRule, select_coarse, select_layers};
    use crate::schema::{Attribute, AttributeRole, CoarseLevel, Record, Schema, SettlementKind};
    use crate::tuning::Stale;
    use std::collections::BTreeMap;

    fn decider() -> Decider {
        Decider::new(KeyMemory::default(), Some(LexicalMemory::default()))
    }

    fn schema() -> Schema {
        Schema::new(
            "ticket",
            vec![
                Attribute::new("product", AttributeRole::Observed),
                Attribute::new("summary", AttributeRole::Observed),
                Attribute::new("team", AttributeRole::Decided),
                Attribute::new("queue", AttributeRole::Decided),
            ],
        )
        .expect("a valid schema")
    }

    /// Records alike in their products and apart in their summaries, some settled again later under the same id, some
    /// checked.
    fn settled(n: usize) -> Vec<Record> {
        let teams = ["network", "billing", "hardware", "auth"];
        let words = [
            "vpn drops",
            "invoice twice",
            "screen broken",
            "password reset",
            "slow link",
            "refund",
        ];
        (0..n)
            .map(|i| {
                let id = if i % 9 == 8 {
                    format!("t{}", i / 3)
                } else {
                    format!("t{i}")
                };
                let team = teams[(i * 7 + i / 5) % teams.len()];
                let mut record = Record {
                    id,
                    values: BTreeMap::from([
                        ("product".into(), format!("p{}", i % 5)),
                        (
                            "summary".into(),
                            format!("{} {}", words[(i * 3) % words.len()], i % 4),
                        ),
                        ("team".into(), team.into()),
                        ("queue".into(), format!("q{}", (i / 2) % 3)),
                    ]),
                    settled_at: 1_000 + i as i64,
                    ..Record::default()
                };
                let kind = if i % 7 == 0 {
                    record.probabilities.insert("team".into(), 0.25);
                    SettlementKind::Check
                } else {
                    SettlementKind::Enter
                };
                record.settlements.insert("team".into(), kind);
                record
            })
            .collect()
    }

    /// The thresholds `select_layers` chooses on `records` from the start, attribute by attribute.
    fn from_the_start(keep: &Keep, records: &[Record]) -> Vec<String> {
        ["queue", "team"]
            .iter()
            .map(|attribute| {
                let chosen = select_layers(
                    &mut KeyMemory::default(),
                    &mut LexicalMemory::default(),
                    &schema(),
                    attribute,
                    records,
                    keep.choice(),
                    LayerOptions {
                        rule: keep.rule,
                        sink: None,
                    },
                );
                format!("{chosen:?}")
            })
            .collect()
    }

    fn of(keeper: &Keeper) -> Vec<String> {
        ["queue", "team"]
            .iter()
            .map(|a| format!("{:?}", keeper.chosen(a).expect("chosen")))
            .collect()
    }

    #[test]
    fn rechoosing_as_records_arrive_chooses_as_a_replay_from_the_start() {
        let records = settled(700);
        for keep in [
            Keep::new(0.8),
            Keep {
                counting: Some(vec![SettlementKind::Enter, SettlementKind::Check]),
                rule: ThresholdRule::Pooled,
                ..Keep::new(0.75)
            },
        ] {
            let mut keeper = Keeper::new(schema(), keep.clone());
            let mut decider = decider();
            let mut done = 0;
            for at in [1, 40, 41, 300, 650, 700] {
                keeper.settle(&mut decider, &records[done..at]);
                done = at;
                keeper.rechoose();
                assert_eq!(
                    of(&keeper),
                    from_the_start(&keep, &records[..at]),
                    "at {at}"
                );
            }

            // The schema carries the choice, and the tuning its promises.
            let team = keeper.chosen("team").unwrap();
            let on_schema = keeper.schema().attribute("team");
            assert_eq!(
                on_schema.key_threshold,
                team.key.chosen.as_ref().map(|c| c.threshold)
            );
            assert_eq!(keeper.tuning().settled, 700);
            assert_eq!(
                keeper.tuning().promise_of("team", Source::KeyMemory),
                team.key.chosen.as_ref().map(|c| c.promise)
            );
        }
    }

    /// The schema with a coarse level on `team`: the network and the hardware teams one class, billing and auth another.
    fn coarse_schema() -> Schema {
        let mut team = Attribute::new("team", AttributeRole::Decided);
        team.coarse = Some(CoarseLevel {
            classes: Some(BTreeMap::from([
                ("network".to_owned(), "infra".to_owned()),
                ("hardware".to_owned(), "infra".to_owned()),
                ("billing".to_owned(), "account".to_owned()),
                ("auth".to_owned(), "account".to_owned()),
            ])),
            ..CoarseLevel::default()
        });
        Schema::new(
            "ticket",
            vec![
                Attribute::new("product", AttributeRole::Observed),
                Attribute::new("summary", AttributeRole::Observed),
                team,
                Attribute::new("queue", AttributeRole::Decided),
            ],
        )
        .expect("a valid schema")
    }

    #[test]
    fn a_coarse_level_is_chosen_as_a_coarse_replay_from_the_start_after_the_values() {
        // The product says the class; within it, the team is a coin the records do not show.
        let records: Vec<Record> = settled(700)
            .into_iter()
            .enumerate()
            .map(|(i, mut r)| {
                let infra = ["p0", "p1", "p2"].contains(&r.values["product"].as_str());
                let team = match (infra, (i * 13 + i / 7) % 2 == 0) {
                    (true, true) => "network",
                    (true, false) => "hardware",
                    (false, true) => "billing",
                    (false, false) => "auth",
                };
                r.values.insert("team".into(), team.into());
                r
            })
            .collect();
        let keep = Keep::new(0.8);
        let mut keeper = Keeper::new(coarse_schema(), keep.clone());
        let mut decider = decider();
        let mut done = 0;
        for at in [1, 40, 300, 700] {
            keeper.settle(&mut decider, &records[done..at]);
            done = at;
            keeper.rechoose();

            let values = select_layers(
                &mut KeyMemory::default(),
                &mut LexicalMemory::default(),
                &coarse_schema(),
                "team",
                &records[..at],
                keep.choice(),
                LayerOptions {
                    rule: keep.rule,
                    sink: None,
                },
            );
            let with_values = coarse_schema().with_layers("team", &values).unwrap();
            let level = select_coarse(
                &mut KeyMemory::default(),
                &mut LexicalMemory::default(),
                &with_values,
                "team",
                &records[..at],
                keep.choice(),
            );
            assert_eq!(
                format!("{:?}", keeper.chosen("team").unwrap()),
                format!("{values:?}"),
                "values at {at}"
            );
            assert_eq!(
                format!("{:?}", keeper.chosen_coarse("team").unwrap()),
                format!("{level:?}"),
                "level at {at}"
            );
            assert_eq!(
                keeper
                    .tuning()
                    .coarse_promise_of("team", crate::decider::Source::SimilarRecord),
                level.memory.chosen.as_ref().map(|c| c.promise)
            );
            let on_schema = keeper.schema().attribute("team").coarse.as_ref().unwrap();
            assert_eq!(
                on_schema.memory_threshold,
                level.memory.chosen.as_ref().map(|c| c.threshold)
            );
        }
        assert!(keeper.chosen_coarse("queue").is_none());
        assert!(
            keeper
                .chosen_coarse("team")
                .unwrap()
                .memory
                .chosen
                .is_some()
                || keeper.chosen_coarse("team").unwrap().key.chosen.is_some(),
            "the level chose something to compare"
        );
    }

    #[test]
    fn a_record_out_of_order_or_removed_replays_from_the_start() {
        let keep = Keep::new(0.8);
        let records = settled(300);
        let mut keeper = Keeper::new(schema(), keep.clone()).with_threads(2);
        let mut decider = decider();
        keeper.settle(&mut decider, &records[..200]);
        keeper.rechoose();
        // Settled late but dated early: the next choice starts again.
        let mut late = records[250].clone();
        late.settled_at = 1_050;
        late.id = "late".into();
        keeper.settle(&mut decider, &records[200..]);
        keeper.settle(&mut decider, std::slice::from_ref(&late));
        keeper.rechoose();
        let mut all = records.clone();
        all.push(late);
        assert_eq!(of(&keeper), from_the_start(&keep, &all));

        keeper.remove(&mut decider, "t10");
        keeper.rechoose();
        all.retain(|r| r.id != "t10");
        assert_eq!(of(&keeper), from_the_start(&keep, &all));
        assert_eq!(keeper.settled(), all.len());
    }

    #[test]
    fn the_choice_is_stale_by_the_tuning_rule() {
        let records = settled(120);
        let mut keeper = Keeper::new(schema(), Keep::new(0.8));
        let mut decider = decider();
        assert_eq!(keeper.stale(), None);
        keeper.settle(&mut decider, &records[..1]);
        assert_eq!(keeper.stale(), Some(Stale::Settled { chosen: 0, now: 1 }));
        keeper.settle(&mut decider, &records[1..100]);
        keeper.rechoose();
        assert_eq!(keeper.stale(), None);
        keeper.settle(&mut decider, &records[100..109]);
        assert_eq!(keeper.stale(), None);
        keeper.settle(&mut decider, &records[109..110]);
        assert_eq!(
            keeper.stale(),
            Some(Stale::Settled {
                chosen: 100,
                now: 110
            })
        );
    }

    #[test]
    fn the_decider_settled_through_the_keeper_answers_as_one_rebuilt() {
        let records = settled(200);
        let mut keeper = Keeper::new(schema(), Keep::new(0.8));
        let mut through = decider();
        keeper.settle(&mut through, &records[..120]);
        keeper.settle(&mut through, &records[120..]);
        keeper.remove(&mut through, "t10");
        let mut rebuilt = decider();
        rebuilt.rebuild(&schema(), &records);
        rebuilt.remove(&schema(), "t10");
        keeper.rechoose();
        let values = BTreeMap::from([
            ("product".to_owned(), "p3".to_owned()),
            ("summary".to_owned(), "vpn drops 2".to_owned()),
        ]);
        let answers = |d: &Decider| {
            format!(
                "{:?}",
                d.answer(keeper.schema(), "new", &values, &BTreeMap::new())
            )
        };
        assert_eq!(answers(&through), answers(&rebuilt));
    }

    #[test]
    fn several_valued_attributes_are_not_kept() {
        let mut tags = Attribute::new("tags", AttributeRole::Decided);
        tags.multiple = true;
        let mut attributes = schema().attributes().to_vec();
        attributes.push(tags);
        let keeper = Keeper::new(
            Schema::new("ticket", attributes).expect("a valid schema"),
            Keep::new(0.8),
        );
        assert_eq!(keeper.attributes().collect::<Vec<_>>(), ["queue", "team"]);
    }
}
