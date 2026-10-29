//! Python bindings for the Gil decision core.
//!
//! The extension module (`gil._gil`) takes schemas and records as JSON and hands results back as JSON text; the
//! `gil` package around it reads that text into dictionaries. Records are gathered and returned whole rather than
//! passed to a Python callback, which would cost a call back into the interpreter per lookup.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use gil::ask_planner;
use gil::decider::{Answer, ExternalCandidate};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record;
use gil::replay::{
    Choice, LayerOptions, LayerThresholds, ScoredDecision, ThresholdRule, scored_decisions,
    select_coarse_with, select_key_concentrations, select_key_fields, select_layers,
    select_score_threshold,
};
use gil::schema::{Record, Schema, Settlement, SettlementKind};
use gil::tuning::Tuning;
use gil::usage::usage;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde_json::{Value, json};

type Parsed<T> = Result<T, String>;

fn invalid(message: String) -> PyErr {
    PyValueError::new_err(message)
}

// ---- reading ----

/// A schema as the core's `serde` form reads it — the settings an attribute leaves out take `Attribute::new`'s defaults;
/// reading checks it as the reference implementation checks one, so a schema that breaks a rule does not read.
fn schema(text: &str) -> Parsed<Schema> {
    serde_json::from_str(text).map_err(|e| format!("a schema does not read: {e}"))
}

fn settlement(name: &str) -> Parsed<SettlementKind> {
    SettlementKind::parse(name).ok_or_else(|| format!("unknown settlement kind '{name}'"))
}

/// A record as the core's `serde` form reads it — `settled_at` as ticks or as a time, an entry given as null left out.
fn record(text: &str) -> Parsed<Record> {
    serde_json::from_str(text).map_err(|e| format!("a record does not read: {e}"))
}

// ---- writing ----

/// A result in the core's `serde` form, as JSON text.
fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("a core result serializes")
}

/// An answer in the core's `serde` form, with `answered` (whether the first candidate is trusted) beside it.
fn answer_json(a: &Answer) -> Value {
    let mut value = serde_json::to_value(a).expect("an answer serializes");
    value["answered"] = json!(a.answered());
    value
}

fn answers_json(answers: &[Answer]) -> String {
    Value::Array(answers.iter().map(answer_json).collect()).to_string()
}

// ---- the module ----

/// Settled records, read once and kept on the Rust side.
#[pyclass(frozen, module = "gil._gil")]
struct Records {
    records: Vec<Record>,
}

#[pymethods]
impl Records {
    /// Records from JSON lines, one record per line, in settlement order.
    #[staticmethod]
    fn from_json_lines(text: &str) -> PyResult<Self> {
        let records = text
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| record(line).map_err(|e| format!("line {}: {e}", i + 1)))
            .collect::<Parsed<Vec<_>>>()
            .map_err(invalid)?;
        Ok(Self { records })
    }

    fn __len__(&self) -> usize {
        self.records.len()
    }
}

/// Checks a schema's JSON; raises `ValueError` when it does not read.
#[pyfunction]
fn check_schema(schema_json: &str) -> PyResult<()> {
    schema(schema_json).map(|_| ()).map_err(invalid)
}

/// The schema and the counted settlement kinds, read from the arguments every selection takes. What the core's contract
/// rules out (an attribute that is not decided, a target outside (0, 1], a selection the attribute has no layer for) is
/// left to the core, whose message `guarded` raises.
fn selection_input(
    schema_json: &str,
    counting: Option<Vec<String>>,
) -> PyResult<(Schema, Option<Vec<SettlementKind>>)> {
    let schema = schema(schema_json).map_err(invalid)?;
    let counting = counting
        .map(|kinds| {
            kinds
                .iter()
                .map(|k| settlement(k))
                .collect::<Parsed<Vec<_>>>()
        })
        .transpose()
        .map_err(invalid)?;
    Ok((schema, counting))
}

/// A threshold rule by its name.
fn threshold_rule(name: &str) -> PyResult<ThresholdRule> {
    match name {
        "every_band" => Ok(ThresholdRule::EveryBand),
        "pooled" => Ok(ThresholdRule::Pooled),
        "least_cost" => Ok(ThresholdRule::LeastCost),
        other => Err(invalid(format!(
            "unknown threshold rule '{other}' (every_band, pooled or least_cost)"
        ))),
    }
}

/// A similar record memory whose lookups are split among at most `threads` threads, or as many as the core's default.
fn record_memory(threads: Option<usize>) -> PyResult<LexicalMemory> {
    match threads {
        None => Ok(LexicalMemory::default()),
        Some(0) => Err(invalid("threads must be at least 1".to_owned())),
        Some(n) => Ok(LexicalMemory::default().with_threads(n)),
    }
}

/// Replays the records in order and chooses the attribute's key and memory thresholds, as `select_layers` does. Returns
/// the thresholds as JSON and, when `decisions` is true, the decision records as JSON lines. Experimental:
/// `value_tails` reports, value by settled value, how the chosen thresholds answered.
#[pyfunction]
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, decisions=true, rule="every_band", threads=None, value_tails=false))]
#[allow(clippy::too_many_arguments)]
fn select_layers_json(
    schema_json: &str,
    attribute: &str,
    records: &Records,
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<Vec<String>>,
    decisions: bool,
    rule: &str,
    threads: Option<usize>,
    value_tails: bool,
) -> PyResult<(String, Option<String>)> {
    let rule = threshold_rule(rule)?;
    let (schema, counting) = selection_input(schema_json, counting)?;
    let mut memory = record_memory(threads)?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = guarded(|| {
        select_layers(
            &mut KeyMemory::default(),
            &mut memory,
            &schema,
            attribute,
            &records.records,
            choice,
            LayerOptions {
                rule,
                sink: Some(&mut |r| push_record(&mut lines, r)),
                value_tails,
            },
        )
    })?;
    Ok((to_json(&layers), lines))
}

fn push_record(lines: &mut Option<String>, r: &DecisionRecord) {
    if let Some(out) = lines.as_mut() {
        out.push_str(&to_json(r));
        out.push('\n');
    }
}

/// Replays the records in order and chooses the thresholds of the attribute's coarse level, as `select_coarse` does — on
/// the lookups the value layers leave at the attribute's thresholds. Returns them as JSON and, when `decisions` is true, the
/// decision records with what the coarse level made of each lookup as JSON lines.
#[pyfunction]
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, decisions=true, threads=None))]
#[allow(clippy::too_many_arguments)]
fn select_coarse_json(
    schema_json: &str,
    attribute: &str,
    records: &Records,
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<Vec<String>>,
    decisions: bool,
    threads: Option<usize>,
) -> PyResult<(String, Option<String>)> {
    let (schema, counting) = selection_input(schema_json, counting)?;
    let mut memory = record_memory(threads)?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = guarded(|| {
        select_coarse_with(
            &mut KeyMemory::default(),
            &mut memory,
            &schema,
            attribute,
            &records.records,
            choice,
            &mut |r| push_record(&mut lines, r),
        )
    })?;
    Ok((to_json(&layers), lines))
}

/// Experimental. Chooses the threshold of a judge outside Gil from its scores and what people settled, as
/// `select_score_threshold` does. `decisions_json` is a JSON array of `{"id", "settled_at", "score", "right", "weight"}`
/// (`score` left out or `null` when the judge gave no candidate; `right` false and `weight` 1 when left out). Returns the
/// replay as JSON.
#[pyfunction]
fn select_score_threshold_json(
    decisions_json: &str,
    target_precision: f64,
    minimum_answered: usize,
) -> PyResult<String> {
    let decisions: Vec<ScoredDecision> = serde_json::from_str(decisions_json)
        .map_err(|e| invalid(format!("the decisions do not read: {e}")))?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: None,
    };
    let replay = guarded(|| select_score_threshold(&decisions, choice))?;
    Ok(to_json(&replay))
}

/// Experimental. The decisions a judge outside Gil made about `attribute`, read from the records as `scored_decisions`
/// does, as a JSON array of `{"id", "settled_at", "score", "right", "weight"}` — what `select_score_threshold_json` takes.
#[pyfunction]
fn scored_decisions_json(records: &Records, attribute: &str) -> String {
    serde_json::to_string(&scored_decisions(&records.records, attribute))
        .expect("decisions serialize")
}

/// Chooses, for each attribute that keys the decided attribute, how readily its keys are believed after only a few settlements,
/// as `select_key_concentrations` does. Returns the concentrations as a JSON object of attribute to concentration.
#[pyfunction]
fn select_key_concentrations_json(
    schema_json: &str,
    attribute: &str,
    records: &Records,
) -> PyResult<String> {
    let schema = schema(schema_json).map_err(invalid)?;
    let found = guarded(|| {
        select_key_concentrations(
            &mut KeyMemory::default(),
            &schema,
            attribute,
            &records.records,
        )
    })?;
    Ok(json!(found).to_string())
}

/// Chooses the attributes that should key the decided attribute, as `select_key_fields` does: each attribute it rests on alone, then
/// greedy additions while the key layer's gain over the target grows. Returns the choice as JSON.
#[pyfunction]
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, typed_longest=0))]
fn select_key_fields_json(
    schema_json: &str,
    attribute: &str,
    records: &Records,
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<Vec<String>>,
    typed_longest: usize,
) -> PyResult<String> {
    let (schema, counting) = selection_input(schema_json, counting)?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let chosen =
        guarded(|| select_key_fields(&schema, attribute, &records.records, choice, typed_longest))?;
    Ok(to_json(&chosen))
}

/// Refuses, with a `ValueError`, a record that keeps an attribute where its kind does not belong — the core would take it
/// for a broken contract partway through a change and leave the decider unusable.
fn placed(schema: &Schema, records: &[Record]) -> PyResult<()> {
    for record in records {
        schema
            .check_placement(&record.values, &record.sets)
            .map_err(|m| invalid(format!("record '{}': {m}", record.id)))?;
    }
    Ok(())
}

/// Runs a core call, turning a broken contract (the core panics on one) into a `ValueError` with the core's message.
fn guarded<T>(call: impl FnOnce() -> T) -> PyResult<T> {
    catch_unwind(AssertUnwindSafe(call)).map_err(|panic| invalid(panic_message(&*panic)))
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "the core refused the call".to_owned())
}

/// Runs a core call that changes a decider (and a record), its contract already checked: a panic now is a defect that
/// may have stopped the change halfway. `Err` carries its message; the caller marks what it was changing broken.
fn changing<T>(call: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(call)).map_err(|panic| panic_message(&*panic))
}

/// The `RuntimeError` for a defect that broke `what`.
fn defect(what: &str, message: String) -> PyErr {
    PyRuntimeError::new_err(format!(
        "the core panicked while changing the {what}, which can no longer be used: {message}"
    ))
}

/// Refuses an object a defect broke.
fn usable(broken: bool, what: &str) -> PyResult<()> {
    if broken {
        return Err(PyRuntimeError::new_err(format!(
            "the {what} is broken: an earlier call that changed it panicked partway; build it again"
        )));
    }
    Ok(())
}

/// The memories that answer: the settled key memory and, unless left out, the lexical similar record memory.
#[pyclass(module = "gil._gil", unsendable)]
struct Decider {
    inner: gil::decider::Decider,
    broken: bool,
}

#[pymethods]
impl Decider {
    /// A decider whose memories hold `records` (settled records of the schema, in settlement order), with the similar
    /// record memory unless `similar_records` is false, its lookups split among at most `threads` threads.
    #[new]
    #[pyo3(signature = (schema_json, records=None, similar_records=true, threads=None))]
    fn new(
        schema_json: &str,
        records: Option<&Records>,
        similar_records: bool,
        threads: Option<usize>,
    ) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let memory = record_memory(threads)?;
        let mut inner =
            gil::decider::Decider::new(KeyMemory::default(), similar_records.then_some(memory));
        if let Some(records) = records {
            placed(&schema, &records.records)?;
            changing(|| inner.rebuild(&schema, &records.records))
                .map_err(|m| defect("decider", m))?;
        }
        Ok(Self {
            inner,
            broken: false,
        })
    }

    /// Puts `records` (settled records of the schema, in settlement order, later than those it holds) into the memories.
    fn put(&mut self, schema_json: &str, records: &Records) -> PyResult<()> {
        usable(self.broken, "decider")?;
        let schema = schema(schema_json).map_err(invalid)?;
        placed(&schema, &records.records)?;
        let inner = &mut self.inner;
        changing(|| inner.rebuild(&schema, &records.records)).map_err(|m| {
            self.broken = true;
            defect("decider", m)
        })
    }

    /// Takes record `record_id` out of the memories — a record the host deleted; nothing happens when they do not hold
    /// it. The key memory is then as if the record had never been put, and the similar record memory never offers it
    /// again.
    fn remove(&mut self, schema_json: &str, record_id: &str) -> PyResult<()> {
        usable(self.broken, "decider")?;
        let schema = schema(schema_json).map_err(invalid)?;
        let inner = &mut self.inner;
        changing(|| inner.remove(&schema, record_id)).map_err(|m| {
            self.broken = true;
            defect("decider", m)
        })
    }

    /// Experimental. Whether a record of `batch` now is the first of its batch: memory holds no settled record of the
    /// batch but `excluding`. A record that names no batch always is.
    #[pyo3(signature = (batch, excluding=None))]
    fn is_first_of_batch(&self, batch: Option<&str>, excluding: Option<&str>) -> PyResult<bool> {
        usable(self.broken, "decider")?;
        Ok(self.inner.is_first_of_batch(batch, excluding))
    }

    /// The answer for one open decided attribute of record `id` from the values given (a JSON object, and the sets of
    /// the attributes that take several as another), writing nothing to memory; the saved version of the record, if
    /// memory holds one, is not evidence for itself. As JSON — with `decision`, the lookup's decision record, when
    /// `decisions` is true.
    #[pyo3(signature = (schema_json, id, attribute, values_json, sets_json=None, decisions=false))]
    fn answer(
        &self,
        schema_json: &str,
        id: &str,
        attribute: &str,
        values_json: &str,
        sets_json: Option<&str>,
        decisions: bool,
    ) -> PyResult<String> {
        usable(self.broken, "decider")?;
        let schema = schema(schema_json).map_err(invalid)?;
        let values: BTreeMap<String, String> = serde_json::from_str(values_json)
            .map_err(|e| invalid(format!("values are an object of strings: {e}")))?;
        let sets: BTreeMap<String, Vec<String>> = match sets_json {
            Some(text) => serde_json::from_str(text)
                .map_err(|e| invalid(format!("sets are an object of string lists: {e}")))?,
            None => BTreeMap::new(),
        };
        let mut kept = None;
        let found = guarded(|| {
            let q = gil::key_memory::Lookup::new(&schema, attribute, &values)
                .sets(&sets)
                .excluding(Some(id));
            self.inner.answer_one_with(&q, &mut |r| {
                if decisions {
                    kept = Some(serde_json::to_value(r).expect("a decision record serializes"));
                }
            })
        })?;
        let mut value = answer_json(&found);
        if let Some(record) = kept {
            value["decision"] = record;
        }
        Ok(value.to_string())
    }
}

/// The promise-keeping loop of one schema: the records settled and the replays the thresholds are chosen on.
#[pyclass(module = "gil._gil", unsendable)]
struct Keeper {
    inner: gil::keeper::Keeper,
    broken: bool,
}

/// A keeper's choice as JSON: `{"tuning": …, "chosen": {attribute: thresholds, …}, "coarse": {attribute: thresholds, …},
/// "first_of_batch": {attribute: thresholds, …}, "form_version": …}` — `coarse` for the attributes with a coarse level,
/// `first_of_batch` when the keeper chooses the first records of batches apart, `form_version` the edition it keeps.
fn keeper_json(keeper: &gil::keeper::Keeper) -> String {
    let chosen: serde_json::Map<String, Value> = keeper
        .attributes()
        .filter_map(|a| {
            keeper.chosen(a).map(|t| {
                (
                    a.to_owned(),
                    serde_json::to_value(t).expect("thresholds serialize"),
                )
            })
        })
        .collect();
    let coarse: serde_json::Map<String, Value> = keeper
        .attributes()
        .filter_map(|a| {
            keeper.chosen_coarse(a).map(|t| {
                (
                    a.to_owned(),
                    serde_json::to_value(t).expect("thresholds serialize"),
                )
            })
        })
        .collect();
    let first: serde_json::Map<String, Value> = keeper
        .attributes()
        .filter_map(|a| {
            keeper.chosen_first_of_batch(a).map(|t| {
                (
                    a.to_owned(),
                    serde_json::to_value(t).expect("thresholds serialize"),
                )
            })
        })
        .collect();
    json!({
        "tuning": keeper.tuning(),
        "chosen": chosen,
        "coarse": coarse,
        "first_of_batch": first,
        "form_version": keeper.form_version(),
    })
    .to_string()
}

#[pymethods]
impl Keeper {
    /// A keeper of the schema with nothing settled, choosing for every decided attribute that takes one value - and,
    /// experimentally, the memory threshold of one that takes several and is answered by similar elements.
    #[new]
    #[pyo3(signature = (schema_json, target_precision, minimum_answered, counting=None, rule="every_band", threads=None, first_of_batch=false, restart_on_form_version=false, value_tails=false, watch_realized=false))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        schema_json: &str,
        target_precision: f64,
        minimum_answered: usize,
        counting: Option<Vec<String>>,
        rule: &str,
        threads: Option<usize>,
        first_of_batch: bool,
        restart_on_form_version: bool,
        value_tails: bool,
        watch_realized: bool,
    ) -> PyResult<Self> {
        let rule = threshold_rule(rule)?;
        let (schema, counting) = selection_input(schema_json, counting)?;
        if threads == Some(0) {
            return Err(invalid("threads must be at least 1".to_owned()));
        }
        let keep = gil::keeper::Keep {
            target_precision,
            minimum_answered,
            counting,
            rule,
            first_of_batch,
            restart_on_form_version,
            value_tails,
            watch_realized,
        };
        let inner = guarded(|| {
            let keeper = gil::keeper::Keeper::new(schema, keep);
            match threads {
                Some(n) => keeper.with_threads(n),
                None => keeper,
            }
        })?;
        Ok(Self {
            inner,
            broken: false,
        })
    }

    /// Puts `records` into `decider` and into the next choice.
    fn settle(&mut self, decider: &mut Decider, records: &Records) -> PyResult<()> {
        usable(self.broken, "keeper")?;
        usable(decider.broken, "decider")?;
        placed(self.inner.schema(), &records.records)?;
        let (inner, memories) = (&mut self.inner, &mut decider.inner);
        changing(|| inner.settle(memories, &records.records)).map_err(|m| {
            self.broken = true;
            decider.broken = true;
            defect("keeper", m)
        })
    }

    /// Takes record `record_id`, which the host deleted, out of `decider` and out of the records.
    fn remove(&mut self, decider: &mut Decider, record_id: &str) -> PyResult<()> {
        usable(self.broken, "keeper")?;
        usable(decider.broken, "decider")?;
        let (inner, memories) = (&mut self.inner, &mut decider.inner);
        changing(|| inner.remove(memories, record_id)).map_err(|m| {
            self.broken = true;
            decider.broken = true;
            defect("keeper", m)
        })
    }

    /// Why the last choice no longer stands, as JSON, or `None` while it does.
    fn stale(&self) -> PyResult<Option<String>> {
        usable(self.broken, "keeper")?;
        Ok(self.inner.stale().map(|s| to_json(&s)))
    }

    /// How many records were settled, a record settled again counting again.
    fn settled(&self) -> usize {
        self.inner.settled()
    }

    /// Experimental. The share of `attribute`'s next settled values likely to be ones no record settled yet, as JSON
    /// `{"once", "settled", "mass"}`, or `None` for an attribute the keeper does not choose for or before a record
    /// settled it.
    fn unseen_mass(&self, attribute: &str) -> PyResult<Option<String>> {
        usable(self.broken, "keeper")?;
        Ok(self
            .inner
            .unseen_mass(attribute)
            .map(|m| json!({"once": m.once, "settled": m.settled, "mass": m.mass()}).to_string()))
    }

    /// Experimental. The elements of `attribute`, which takes several values and is answered by similar elements
    /// (`"similar_elements": true`), that the similar record layer offers for record `record_id` from the values (and
    /// sets) given, as a JSON list of `{"value", "share", "promise"}`, most shared first. The set chosen of `attribute`
    /// so far is not evidence.
    #[pyo3(signature = (decider, record_id, attribute, values_json, sets_json=None))]
    fn similar_elements(
        &self,
        decider: PyRef<'_, Decider>,
        record_id: &str,
        attribute: &str,
        values_json: &str,
        sets_json: Option<&str>,
    ) -> PyResult<String> {
        usable(self.broken, "keeper")?;
        usable(decider.broken, "decider")?;
        let values: BTreeMap<String, String> = serde_json::from_str(values_json)
            .map_err(|e| invalid(format!("values are an object of strings: {e}")))?;
        let sets: BTreeMap<String, Vec<String>> = match sets_json {
            Some(text) => serde_json::from_str(text)
                .map_err(|e| invalid(format!("sets are an object of string lists: {e}")))?,
            None => BTreeMap::new(),
        };
        self.inner
            .schema()
            .check_placement(&values, &sets)
            .map_err(|m| invalid(format!("record '{record_id}': {m}")))?;
        let found = guarded(|| {
            self.inner
                .similar_elements(&decider.inner, record_id, attribute, &values, &sets)
        })?;
        Ok(to_json(&found))
    }

    /// The edition of the code lists the keeper keeps records of, or `None`. Experimental.
    fn form_version(&self) -> Option<String> {
        self.inner.form_version().map(str::to_owned)
    }

    /// Chooses again; returns the choice as JSON and, when `decisions` is true, the decision records of the records
    /// replayed this time as JSON lines.
    #[pyo3(signature = (decisions=false))]
    fn rechoose(&mut self, decisions: bool) -> PyResult<(String, Option<String>)> {
        usable(self.broken, "keeper")?;
        let mut lines = decisions.then(String::new);
        let inner = &mut self.inner;
        changing(|| {
            inner.rechoose_with(&mut |r| push_record(&mut lines, r));
        })
        .map_err(|m| {
            self.broken = true;
            defect("keeper", m)
        })?;
        Ok((keeper_json(&self.inner), lines))
    }

    /// The choice as it stands, as JSON.
    fn state(&self) -> PyResult<String> {
        usable(self.broken, "keeper")?;
        Ok(keeper_json(&self.inner))
    }

    /// The schema as chosen, as JSON.
    fn schema_json(&self) -> PyResult<String> {
        usable(self.broken, "keeper")?;
        Ok(to_json(self.inner.schema()))
    }
}

/// Which observed attribute to ask a person for first, so that a decided attribute gets answered. Experimental.
#[pyclass(module = "gil._gil", unsendable)]
struct AskPlanner {
    inner: ask_planner::AskPlanner,
}

#[pymethods]
impl AskPlanner {
    /// A planner for the decided single-valued attribute `attribute`, the observed attributes `askable` a person could
    /// be asked for, and the share `budget` of the typing asking every time would take that asking may spend.
    #[new]
    fn new(
        schema_json: &str,
        attribute: &str,
        askable: Vec<String>,
        budget: f64,
    ) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let inner =
            ask_planner::AskPlanner::new(&schema, attribute, askable, budget).map_err(invalid)?;
        Ok(Self { inner })
    }

    /// Learns from a settled record (JSON, as records are read), in settlement order, asking `decider` — whose memories
    /// may hold the record but none settled after it.
    fn learn(&mut self, decider: PyRef<'_, Decider>, record_json: &str) -> PyResult<()> {
        usable(decider.broken, "decider")?;
        let settled = record(record_json).map_err(invalid)?;
        let inner = &mut self.inner;
        guarded(|| inner.learn(&decider.inner, &settled))
    }

    /// Advice for an open record from the decided attribute's answer (JSON, as an answer is given) and the record's
    /// values (a JSON object), as JSON.
    fn advise(&self, answer_json: &str, values_json: &str) -> PyResult<String> {
        let answer: Answer = serde_json::from_str(answer_json)
            .map_err(|e| invalid(format!("an answer does not read: {e}")))?;
        let values: BTreeMap<String, String> = serde_json::from_str(values_json)
            .map_err(|e| invalid(format!("values are an object of strings: {e}")))?;
        guarded(|| self.inner.advise(&answer, &values)).map(|a| to_json(&a))
    }
}

fn settlement_of(
    kind: &str,
    value: Option<&str>,
    values: Option<Vec<String>>,
    probability: Option<f64>,
) -> Parsed<Settlement> {
    let values: Option<Vec<&str>> = values
        .as_ref()
        .map(|v| v.iter().map(String::as_str).collect());
    Settlement::new(settlement(kind)?, value, values.as_deref(), probability)
}

/// One record of a schema while its values arrive. Each change puts it into the decider's memory again and returns the
/// fresh answers it causes, as JSON.
#[pyclass(module = "gil._gil", unsendable)]
struct OpenRecord {
    inner: open_record::OpenRecord,
    broken: bool,
}

#[pymethods]
impl OpenRecord {
    /// A new record `id`, starting at `settled_at` (100-nanosecond ticks since the Unix epoch, UTC).
    /// Experimental: `batch` names the batch the record is one of.
    #[new]
    #[pyo3(signature = (schema_json, id, settled_at, batch=None))]
    fn new(schema_json: &str, id: &str, settled_at: i64, batch: Option<&str>) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        guarded(|| {
            let open = open_record::OpenRecord::new(schema, id, settled_at);
            match batch {
                Some(batch) => open.with_batch(batch),
                None => open,
            }
        })
        .map(|inner| Self {
            inner,
            broken: false,
        })
    }

    /// Experimental. The batch the record names, if any.
    #[getter]
    fn batch(&self) -> Option<String> {
        self.inner.batch().map(str::to_owned)
    }

    /// Experimental. Whether the record is answered as the first of its batch now.
    fn is_first_of_batch(&self, decider: &Decider) -> PyResult<bool> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        Ok(self.inner.is_first_of_batch(&decider.inner))
    }

    /// A saved record (JSON, as a snapshot gives it) reopened as it was left.
    #[staticmethod]
    fn reopen(schema_json: &str, record_json: &str) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let saved = record(record_json).map_err(invalid)?;
        guarded(|| open_record::OpenRecord::reopen(schema, &saved)).map(|inner| Self {
            inner,
            broken: false,
        })
    }

    #[getter]
    fn id(&self) -> String {
        self.inner.id().to_owned()
    }

    /// Sets (or with `None` clears) an observed attribute's value; the answers it changes, as JSON.
    #[pyo3(signature = (decider, attribute, value))]
    fn observe(
        &mut self,
        mut decider: PyRefMut<'_, Decider>,
        attribute: &str,
        value: Option<&str>,
    ) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        self.inner
            .check_observe(attribute)
            .map_err(|e| invalid(e.to_string()))?;
        let memories = &mut decider.inner;
        match changing(|| self.inner.observe(memories, attribute, value)) {
            Ok(found) => Ok(answers_json(&found)),
            Err(message) => {
                self.broken = true;
                decider.broken = true;
                Err(defect("record and its decider", message))
            }
        }
    }

    /// Settles a decided attribute at `now` (ticks) — `kind` by name, with the value, values or probability it needs, and
    /// what was in view (JSON `{"value", "source", "promise"}`) for the record to keep; the answers it changes, as JSON.
    #[pyo3(signature = (decider, attribute, kind, now, value=None, values=None, probability=None, in_view=None))]
    #[allow(clippy::too_many_arguments)]
    fn settle(
        &mut self,
        mut decider: PyRefMut<'_, Decider>,
        attribute: &str,
        kind: &str,
        now: i64,
        value: Option<&str>,
        values: Option<Vec<String>>,
        probability: Option<f64>,
        in_view: Option<&str>,
    ) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        let mut settlement = settlement_of(kind, value, values, probability).map_err(invalid)?;
        settlement.in_view = in_view
            .map(|text| {
                serde_json::from_str(text)
                    .map_err(|e| invalid(format!("what was in view does not read: {e}")))
            })
            .transpose()?;
        self.inner
            .check_settle(attribute, &settlement)
            .map_err(|e| invalid(e.to_string()))?;
        let memories = &mut decider.inner;
        match changing(|| self.inner.settle(memories, attribute, &settlement, now)) {
            Ok(found) => Ok(answers_json(&found)),
            Err(message) => {
                self.broken = true;
                decider.broken = true;
                Err(defect("record and its decider", message))
            }
        }
    }

    /// Answers for every open decided attribute, as JSON — each with `decision`, its lookup's decision record, when
    /// `decisions` is true.
    #[pyo3(signature = (decider, decisions=false))]
    fn answer(&self, decider: PyRef<'_, Decider>, decisions: bool) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        let mut kept: Vec<Value> = Vec::new();
        let found = guarded(|| {
            self.inner.answer_with(&decider.inner, &mut |r| {
                if decisions {
                    kept.push(serde_json::to_value(r).expect("a decision record serializes"));
                }
            })
        })?;
        let mut items: Vec<Value> = found.iter().map(answer_json).collect();
        for (item, record) in items.iter_mut().zip(kept) {
            item["decision"] = record;
        }
        Ok(Value::Array(items).to_string())
    }

    /// The answer for one open decided attribute alone — with `partial`, the text typed into it so far — as JSON.
    #[pyo3(signature = (decider, attribute, partial=None))]
    fn answer_partial(
        &self,
        decider: PyRef<'_, Decider>,
        attribute: &str,
        partial: Option<&str>,
    ) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        guarded(|| {
            self.inner
                .answer_partial(&decider.inner, attribute, partial)
        })
        .map(|a| answer_json(&a).to_string())
    }

    /// The answer for an open decided attribute with an external judgement's ranked values in it (JSON list of
    /// `{"value", "probability", "trusted"}`), as JSON.
    fn offer_external(
        &self,
        decider: PyRef<'_, Decider>,
        attribute: &str,
        ranked_json: &str,
    ) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        let ranked: Value =
            serde_json::from_str(ranked_json).map_err(|e| invalid(e.to_string()))?;
        let ranked = ranked
            .as_array()
            .ok_or_else(|| invalid("an external judgement is a list".to_owned()))?
            .iter()
            .map(|c| {
                Ok(ExternalCandidate {
                    value: c["value"]
                        .as_str()
                        .ok_or("an external value is a string")?
                        .to_owned(),
                    probability: c["probability"]
                        .as_f64()
                        .ok_or("an external probability is a number")?,
                    trusted: c.get("trusted").and_then(Value::as_bool).unwrap_or(false),
                })
            })
            .collect::<Parsed<Vec<_>>>()
            .map_err(invalid)?;
        guarded(|| {
            self.inner
                .offer_external(&decider.inner, attribute, &ranked)
        })
        .map(|a| answer_json(&a).to_string())
    }

    /// The record as it stands, to save — JSON in the form records are read in.
    fn snapshot(&self) -> PyResult<String> {
        usable(self.broken, "record")?;
        Ok(serde_json::to_string(&self.inner.snapshot()).expect("a record serializes"))
    }
}

fn tuning(text: &str) -> PyResult<Tuning> {
    serde_json::from_str(text).map_err(|e| invalid(format!("a tuning does not read: {e}")))
}

/// The chosen settings of the schema's decided attributes, chosen on `settled` records for `target_precision`, as JSON.
#[pyfunction]
fn tuning_capture_json(
    schema_json: &str,
    settled: usize,
    target_precision: f64,
) -> PyResult<String> {
    let schema = schema(schema_json).map_err(invalid)?;
    Ok(
        serde_json::to_string(&Tuning::capture(&schema, settled, target_precision))
            .expect("a tuning serializes"),
    )
}

/// The tuning with what `attribute`'s layer replay (JSON, as `select_layers_json` gives it) promised — its coarse
/// level's, from `select_coarse_json`, when `coarse` — as JSON.
#[pyfunction]
#[pyo3(signature = (tuning_json, attribute, layers_json, coarse = false, first_of_batch = false))]
fn tuning_promise_json(
    tuning_json: &str,
    attribute: &str,
    layers_json: &str,
    coarse: bool,
    first_of_batch: bool,
) -> PyResult<String> {
    let mut tuned = tuning(tuning_json)?;
    let layers: LayerThresholds = serde_json::from_str(layers_json)
        .map_err(|e| invalid(format!("a layer replay does not read: {e}")))?;
    if coarse && first_of_batch {
        return Err(invalid(
            "a promise is the coarse level's or the first records', not both".to_owned(),
        ));
    }
    if coarse {
        guarded(|| tuned.promise_coarse(attribute, &layers))?;
    } else if first_of_batch {
        guarded(|| tuned.promise_first_of_batch(attribute, &layers))?;
    } else {
        guarded(|| tuned.promise(attribute, &layers))?;
    }
    Ok(to_json(&tuned))
}

/// The schema with what `attribute`'s layer replay (JSON, as `select_layers_json` gives it) chose put on the attribute —
/// on its coarse level when `coarse` — as JSON.
#[pyfunction]
fn schema_with_layers_json(
    schema_json: &str,
    attribute: &str,
    layers_json: &str,
    coarse: bool,
) -> PyResult<String> {
    let schema = schema(schema_json).map_err(invalid)?;
    let layers: LayerThresholds = serde_json::from_str(layers_json)
        .map_err(|e| invalid(format!("a layer replay does not read: {e}")))?;
    let changed = if coarse {
        schema.with_coarse(attribute, &layers)
    } else {
        schema.with_layers(attribute, &layers)
    }
    .map_err(invalid)?;
    Ok(serde_json::to_string(&changed).expect("a schema serializes"))
}

/// Experimental. The schema with what a replay of the first records of batches chose put on `attribute`'s thresholds for
/// them, as JSON.
#[pyfunction]
fn schema_with_first_of_batch_json(
    schema_json: &str,
    attribute: &str,
    layers_json: &str,
) -> PyResult<String> {
    let schema = schema(schema_json).map_err(invalid)?;
    let layers: LayerThresholds = serde_json::from_str(layers_json)
        .map_err(|e| invalid(format!("a layer replay does not read: {e}")))?;
    let changed = schema
        .with_first_of_batch(attribute, &layers)
        .map_err(invalid)?;
    Ok(serde_json::to_string(&changed).expect("a schema serializes"))
}

/// Experimental. The schema a first record of a batch is answered with, as JSON.
#[pyfunction]
fn schema_for_first_of_batch_json(schema_json: &str) -> PyResult<String> {
    let schema = schema(schema_json).map_err(invalid)?;
    Ok(serde_json::to_string(&schema.for_first_of_batch()).expect("a schema serializes"))
}

/// The schema with the tuning's settings put back on its attributes, as JSON.
#[pyfunction]
fn tuning_apply_json(tuning_json: &str, schema_json: &str) -> PyResult<String> {
    let schema = tuning(tuning_json)?
        .apply(&schema(schema_json).map_err(invalid)?)
        .map_err(invalid)?;
    Ok(serde_json::to_string(&schema).expect("a schema serializes"))
}

/// Why the tuning no longer stands for the schema with `settled` records, as JSON — `None` while it does.
#[pyfunction]
fn tuning_stale_json(
    tuning_json: &str,
    schema_json: &str,
    settled: usize,
) -> PyResult<Option<String>> {
    let schema = schema(schema_json).map_err(invalid)?;
    Ok(tuning(tuning_json)?
        .stale(&schema, settled)
        .map(|s| serde_json::to_string(&s).expect("a reason serializes")))
}

/// The usage counts of the decisions the records settled, by attribute, as JSON.
#[pyfunction]
fn usage_json(records: &Records) -> String {
    serde_json::to_string(&usage(&records.records)).expect("counts serialize")
}

#[pymodule]
fn _gil(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // A broken contract reaches Python as an exception carrying the core's message; the default hook would also print
    // it to stderr. The hook belongs to this extension's own copy of the Rust runtime.
    std::panic::set_hook(Box::new(|_| {}));
    m.add_class::<Records>()?;
    m.add_class::<Decider>()?;
    m.add_class::<OpenRecord>()?;
    m.add_class::<AskPlanner>()?;
    m.add_class::<Keeper>()?;
    m.add_function(wrap_pyfunction!(tuning_capture_json, m)?)?;
    m.add_function(wrap_pyfunction!(schema_with_layers_json, m)?)?;
    m.add_function(wrap_pyfunction!(schema_with_first_of_batch_json, m)?)?;
    m.add_function(wrap_pyfunction!(schema_for_first_of_batch_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_apply_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_promise_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_stale_json, m)?)?;
    m.add_function(wrap_pyfunction!(usage_json, m)?)?;
    m.add_function(wrap_pyfunction!(check_schema, m)?)?;
    m.add_function(wrap_pyfunction!(select_layers_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_key_fields_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_coarse_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_score_threshold_json, m)?)?;
    m.add_function(wrap_pyfunction!(scored_decisions_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_key_concentrations_json, m)?)?;
    m.add("RECENT_RECORDS", gil::replay::RECENT_RECORDS)?;
    m.add(
        "POOLED_STANDARD_ERRORS",
        gil::replay::POOLED_STANDARD_ERRORS,
    )?;
    m.add("POOLED_RECENT_LOOKUPS", gil::replay::POOLED_RECENT_LOOKUPS)?;
    m.add("POOLED_RECENT_ANSWERS", gil::replay::POOLED_RECENT_ANSWERS)?;
    Ok(())
}
