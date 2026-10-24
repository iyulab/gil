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
//! Experimental: a keeper told to ([`Keep::restart_on_form_version`]) starts again from nothing when a record arrives
//! under a new edition of the code lists ([`Record::form_version`]) — the records before it leave the decider and the
//! replays, and the thresholds chosen on them leave the schema.
//!
//! What it keeps is the choosing and the promise. Whether to check answers again, how often and where to send them stay
//! with the host. Nothing is stored outside the process: a keeper made at start-up replays the saved records once, from
//! the start.

use std::collections::{BTreeMap, BTreeSet};

use crate::decider::Decider;
use crate::decision_record::DecisionRecord;
use crate::key_memory::KeyMemory;
use crate::lexical_memory::LexicalMemory;
use crate::replay::{
    Choice, LayerThresholds, ReplayState, Replaying, ThresholdRule, batch_check, carry_on,
    coarse_layers, in_situation, layers, value_tails,
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
    /// Experimental. Whether the first records of batches — whose batch had no settled record when they were answered
    /// ([`Record::batch`]) — have the value layers' thresholds chosen and promised apart from the rest
    /// ([`crate::schema::Attribute::first_of_batch`]), each on its own lookups and resting on the minimum number of
    /// answers. Where a source sends records in batches that are right or wrong together, a promise taken on every
    /// answer holds for the records after a batch's first and flatters the first; chosen apart, the first records are
    /// held to the target on their own. False chooses one set of thresholds for every record, and leaves none for the
    /// first records on the schema.
    pub first_of_batch: bool,
    /// Experimental. Whether a record under a new edition of the code lists ([`Record::form_version`] — new only when a
    /// value comes to mean something else) starts the keeper again from nothing: the records settled before it leave
    /// the decider and the replays, the thresholds chosen on them leave the schema, and records still arriving under an
    /// edition left behind are not taken. Carried across a renumbering, a memory answers with codes whose meaning
    /// changed, and a promise chosen on the old edition does not hold on the new. A record that names no edition is
    /// taken as of the current one; the first edition named starts nothing. False keeps every record whatever its
    /// edition.
    pub restart_on_form_version: bool,
    /// Experimental. Whether each choice reports, value by settled value, how its thresholds answered
    /// ([`LayerThresholds::value_tails`]) — on the records the value thresholds were chosen on.
    pub value_tails: bool,
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
            first_of_batch: false,
            restart_on_form_version: false,
            value_tails: false,
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
    first: BTreeMap<String, LayerThresholds>,
    coarse: BTreeMap<String, LayerThresholds>,
    tuning: Tuning,
    form_version: Option<String>,
    left: BTreeSet<String>,
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
            first: BTreeMap::new(),
            coarse: BTreeMap::new(),
            tuning,
            form_version: None,
            left: BTreeSet::new(),
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

    /// Experimental. What the last [`Keeper::rechoose`] chose for `attribute`'s first records of batches
    /// ([`Keep::first_of_batch`]); `None` before the first, when the keeper does not choose them apart, and for an
    /// attribute it does not choose for.
    pub fn chosen_first_of_batch(&self, attribute: &str) -> Option<&LayerThresholds> {
        self.first.get(attribute)
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

    /// Experimental. The edition of the code lists the keeper keeps records of ([`Keep::restart_on_form_version`]) — the
    /// last one a record named; `None` until a record names one, and always when the keeper does not restart.
    pub fn form_version(&self) -> Option<&str> {
        self.form_version.as_deref()
    }

    /// How many records were settled, a record settled again counting again — what staleness is counted on.
    pub fn settled(&self) -> usize {
        self.records.len()
    }

    /// Takes settled records: `decider` learns them ([`Decider::rebuild`]) and the next choice replays them. Records may
    /// come in any order; one settled before the last a replay has gone through has the next choice replay everything
    /// from the start. With [`Keep::restart_on_form_version`], a record under a new edition starts the keeper again
    /// from nothing before it is taken — what came before it, in this call or earlier, is forgotten.
    pub fn settle(&mut self, decider: &mut Decider, records: &[Record]) {
        if !self.keep.restart_on_form_version {
            decider.rebuild(&self.schema, records);
            self.records.extend_from_slice(records);
            return;
        }
        let mut taken: Vec<Record> = Vec::with_capacity(records.len());
        for record in records {
            match record.form_version.as_deref() {
                Some(edition) if self.left.contains(edition) => continue,
                Some(edition) if self.form_version.as_deref().is_some_and(|e| e != edition) => {
                    taken.clear();
                    self.begin(decider, edition);
                }
                Some(edition) if self.form_version.is_none() => {
                    self.form_version = Some(edition.to_owned());
                }
                _ => {}
            }
            taken.push(record.clone());
        }
        decider.rebuild(&self.schema, &taken);
        self.records.extend(taken);
    }

    /// Starts again from nothing under `edition`: the records settled so far leave `decider` and the replays, their
    /// edition is left behind, and the thresholds chosen on them leave the schema.
    fn begin(&mut self, decider: &mut Decider, edition: &str) {
        let ids: BTreeSet<String> = self.records.drain(..).map(|r| r.id).collect();
        for id in &ids {
            decider.remove(&self.schema, id);
        }
        if let Some(previous) = self.form_version.replace(edition.to_owned()) {
            self.left.insert(previous);
        }
        let mut schema = self.schema.clone();
        for attribute in self.replays.keys() {
            schema = schema.derived(attribute, |a| {
                a.key_threshold = None;
                a.memory_threshold = None;
                a.memory_floor = None;
                a.first_of_batch = None;
                if let Some(level) = a.coarse.as_mut() {
                    level.key_threshold = None;
                    level.memory_threshold = None;
                    level.memory_floor = None;
                }
            });
        }
        self.schema = schema;
        self.chosen.clear();
        self.first.clear();
        self.coarse.clear();
        self.tuning = Tuning::capture(&self.schema, 0, self.keep.target_precision);
        self.restart();
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
        let mut first = BTreeMap::new();
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
            // Apart, the value thresholds are the later records' and the first records' are chosen on their own.
            let later = self
                .keep
                .first_of_batch
                .then(|| in_situation(&replay.state.steps, false));
            let steps = later.as_deref().unwrap_or(&replay.state.steps);
            let thresholds = layers(
                steps,
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
            let value_tails = self
                .keep
                .value_tails
                .then(|| value_tails(steps, replay.state.order, &thresholds));
            let thresholds = LayerThresholds {
                batches,
                value_tails,
                ..thresholds
            };
            schema = schema
                .with_layers(attribute, &thresholds)
                .expect("a chosen threshold fits the attribute it was chosen for");
            chosen.insert(attribute.clone(), thresholds);
            if self.keep.first_of_batch {
                let firsts = layers(
                    &in_situation(&replay.state.steps, true),
                    replay.state.order,
                    definition,
                    choice,
                    self.keep.rule,
                );
                schema = schema
                    .with_first_of_batch(attribute, &firsts)
                    .expect("a chosen threshold fits the attribute it was chosen for");
                first.insert(attribute.clone(), firsts);
            } else if definition.first_of_batch.is_some() {
                schema = schema.derived(attribute, |a| a.first_of_batch = None);
            }
            if definition.coarse.is_some() {
                // On the lookups the value layers leave at the thresholds just chosen — a first record's at the first
                // records' when they are chosen apart.
                let values = schema.attribute(attribute);
                let firsts = values
                    .first_of_batch
                    .as_ref()
                    .map(|f| crate::schema::Attribute {
                        key_threshold: f.key_threshold,
                        memory_threshold: f.memory_threshold,
                        memory_floor: f.memory_floor,
                        ..values.clone()
                    });
                let level = coarse_layers(
                    &replay.state.steps,
                    replay.state.order,
                    values,
                    firsts.as_ref(),
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
        for (attribute, thresholds) in &first {
            tuning.promise_first_of_batch(attribute, thresholds);
        }
        for (attribute, level) in &coarse {
            tuning.promise_coarse(attribute, level);
        }
        self.schema = schema;
        self.chosen = chosen;
        self.first = first;
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
                        value_tails: keep.value_tails,
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

    /// `records` in batches of four, a record settled again joining the batch of its place.
    fn in_batches(records: &[Record]) -> Vec<Record> {
        records
            .iter()
            .enumerate()
            .map(|(i, r)| Record {
                batch: Some(format!("b{}", i / 4)),
                ..r.clone()
            })
            .collect()
    }

    /// Whether each record is the first of its batch: no other record settled before it named the batch.
    fn firsts(records: &[Record]) -> Vec<bool> {
        let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
        records
            .iter()
            .map(|r| match &r.batch {
                None => true,
                Some(b) => {
                    let mates = seen.entry(b.clone()).or_default();
                    let first = !mates.iter().any(|m| m != &r.id);
                    mates.push(r.id.clone());
                    first
                }
            })
            .collect()
    }

    /// `records` with the settlements of the records in the other situation made a kind `counting` leaves out — what a
    /// host would do to choose for one situation by hand.
    fn only(records: &[Record], first: bool) -> Vec<Record> {
        let marks = firsts(records);
        records
            .iter()
            .zip(marks)
            .map(|(r, f)| {
                let mut r = r.clone();
                for attribute in ["team", "queue"] {
                    let kind = if f == first {
                        r.settlements
                            .get(attribute)
                            .copied()
                            .unwrap_or(SettlementKind::Enter)
                    } else {
                        SettlementKind::Correct
                    };
                    r.settlements.insert(attribute.into(), kind);
                }
                r
            })
            .collect()
    }

    fn layers_of(t: &crate::replay::LayerThresholds) -> String {
        format!("{:?} {:?}", t.key, t.memory)
    }

    #[test]
    fn first_records_apart_are_chosen_as_a_replay_counting_only_them() {
        let records = in_batches(&settled(700));
        let keep = Keep {
            first_of_batch: true,
            ..Keep::new(0.8)
        };
        let mut keeper = Keeper::new(schema(), keep.clone());
        let mut decider = decider();
        for part in records.chunks(150) {
            keeper.settle(&mut decider, part);
            keeper.rechoose();
        }

        let counted = Keep {
            counting: Some(vec![SettlementKind::Enter, SettlementKind::Check]),
            first_of_batch: false,
            ..keep
        };
        for (first, chosen) in [
            (true, keeper.chosen_first_of_batch("team")),
            (false, keeper.chosen("team")),
        ] {
            let by_hand = select_layers(
                &mut KeyMemory::default(),
                &mut LexicalMemory::default(),
                &schema(),
                "team",
                &only(&records, first),
                counted.choice(),
                LayerOptions {
                    rule: counted.rule,
                    sink: None,
                    value_tails: counted.value_tails,
                },
            );
            assert_eq!(
                layers_of(chosen.expect("chosen")),
                layers_of(&by_hand),
                "first: {first}"
            );
        }

        let first = keeper
            .chosen_first_of_batch("team")
            .expect("chosen for first records");
        assert!(
            first.key.chosen.is_some(),
            "the first records answer on their own"
        );
        let on_schema = keeper
            .schema()
            .attribute("team")
            .first_of_batch
            .clone()
            .expect("on the schema");
        assert_eq!(
            on_schema.key_threshold,
            first.key.chosen.as_ref().map(|c| c.threshold)
        );
        assert_eq!(
            keeper
                .tuning()
                .first_of_batch_promise_of("team", Source::KeyMemory),
            first.key.chosen.as_ref().map(|c| c.promise)
        );
        // A tuning puts the thresholds for first records back.
        let applied = keeper.tuning().apply(&schema()).expect("applies");
        assert_eq!(
            applied.attribute("team").first_of_batch,
            keeper.schema().attribute("team").first_of_batch
        );
    }

    #[test]
    fn records_that_name_no_batch_are_all_first_records() {
        let records = settled(700);
        let mut keeper = Keeper::new(
            schema(),
            Keep {
                first_of_batch: true,
                ..Keep::new(0.8)
            },
        );
        let mut decider = decider();
        keeper.settle(&mut decider, &records);
        keeper.rechoose();
        let mut alike = Keeper::new(schema(), Keep::new(0.8));
        alike.settle(&mut decider, &records);
        alike.rechoose();
        assert_eq!(
            layers_of(keeper.chosen_first_of_batch("team").expect("chosen")),
            layers_of(alike.chosen("team").expect("chosen"))
        );
        assert!(keeper.chosen("team").expect("chosen").key.chosen.is_none());
    }

    #[test]
    fn a_keeper_not_choosing_first_records_apart_leaves_none_on_the_schema() {
        let records = in_batches(&settled(300));
        let mut keeper = Keeper::new(schema().for_first_of_batch(), Keep::new(0.8));
        let mut decider = decider();
        keeper.settle(&mut decider, &records);
        keeper.rechoose();
        assert!(!keeper.schema().has_first_of_batch());
        assert!(keeper.chosen_first_of_batch("team").is_none());
        assert!(keeper.tuning().attributes.iter().all(|a| !a.first_of_batch));
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
            Keep {
                value_tails: true,
                ..Keep::new(0.8)
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
                    value_tails: keep.value_tails,
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

    /// `records` under the edition `edition`, the second edition renumbering the teams.
    fn under(records: &[Record], edition: &str) -> Vec<Record> {
        records
            .iter()
            .map(|r| {
                let mut r = r.clone();
                if edition == "2" {
                    let team = r.values["team"].clone();
                    r.values.insert("team".into(), format!("{team}-2"));
                }
                r.id = format!("{}@{edition}", r.id);
                r.form_version = Some(edition.into());
                r
            })
            .collect()
    }

    fn answers(decider: &Decider, schema: &Schema, record: &Record) -> String {
        format!(
            "{:?}",
            decider.answer(schema, "probe", &record.values, &BTreeMap::new())
        )
    }

    #[test]
    fn a_new_edition_starts_the_keeper_again_from_its_first_record() {
        let keep = Keep {
            restart_on_form_version: true,
            ..Keep::new(0.8)
        };
        let records = settled(600);
        let old = under(&records[..300], "1");
        let new = under(&records[300..], "2");
        let mut keeper = Keeper::new(schema(), keep.clone());
        let mut decider = decider();
        keeper.settle(&mut decider, &old[..200]);
        keeper.rechoose();
        assert_eq!(keeper.form_version(), Some("1"));
        assert!(keeper.schema().attribute("team").key_threshold.is_some());

        // The rest of the old edition and the first of the new in one call: the new starts from its first record.
        let mut batch = old[200..].to_vec();
        batch.extend_from_slice(&new[..1]);
        keeper.settle(&mut decider, &batch);
        assert_eq!(keeper.form_version(), Some("2"));
        assert_eq!(keeper.settled(), 1);
        let team = keeper.schema().attribute("team");
        assert_eq!(
            (team.key_threshold, team.memory_threshold, team.memory_floor),
            (None, None, None)
        );
        assert!(keeper.chosen("team").is_none());
        assert_eq!(keeper.stale(), Some(Stale::Settled { chosen: 0, now: 1 }));

        // A record still arriving under the edition left behind is not taken.
        keeper.settle(&mut decider, &new[1..]);
        keeper.settle(&mut decider, &old[..5]);
        keeper.rechoose();
        assert_eq!(keeper.settled(), new.len());
        assert_eq!(of(&keeper), from_the_start(&keep, &new));

        // The decider holds the new edition alone.
        let mut alone = self::decider();
        alone.rebuild(&schema(), &new);
        for probe in [&new[0], &new[150], &old[10]] {
            assert_eq!(
                answers(&decider, keeper.schema(), probe),
                answers(&alone, keeper.schema(), probe)
            );
        }
    }

    #[test]
    fn records_naming_no_edition_or_the_first_one_start_nothing() {
        let keep = Keep {
            restart_on_form_version: true,
            ..Keep::new(0.8)
        };
        let records = settled(300);
        let mut keeper = Keeper::new(schema(), keep.clone());
        let mut decider = decider();
        keeper.settle(&mut decider, &records[..100]);
        keeper.settle(&mut decider, &under(&records[100..200], "1"));
        keeper.settle(&mut decider, &records[200..]);
        keeper.rechoose();
        assert_eq!(keeper.form_version(), Some("1"));
        assert_eq!(keeper.settled(), 300);

        // Without restarting, editions are kept whatever they say.
        let mut keeper = Keeper::new(schema(), Keep::new(0.8));
        let mut decider = self::decider();
        keeper.settle(&mut decider, &under(&records[..150], "1"));
        keeper.settle(&mut decider, &under(&records[150..], "2"));
        assert_eq!(keeper.settled(), 300);
        assert_eq!(keeper.form_version(), None);
    }
}
