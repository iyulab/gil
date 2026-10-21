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
    Choice, LayerThresholds, ThresholdRule, select_coarse_with, select_key_concentrations,
    select_key_fields, select_layers_by_rule_with,
};
use gil::schema::{Record, Schema, Settlement, SettlementKind};
use gil::tuning::Tuning;
use gil::usage::{Decision, usage};
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

/// `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` → 100-nanosecond ticks since the Unix epoch, UTC.
fn ticks(text: &str) -> Parsed<i64> {
    let bad = || format!("settled_at '{text}' is not yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM");
    if text.len() != 33 || !text.is_ascii() {
        return Err(bad());
    }
    let number =
        |range: std::ops::Range<usize>| -> Parsed<i64> { text[range].parse().map_err(|_| bad()) };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second, fraction) = (
        number(11..13)?,
        number(14..16)?,
        number(17..19)?,
        number(20..27)?,
    );
    let sign = match &text[27..28] {
        "+" => 1,
        "-" => -1,
        _ => return Err(bad()),
    };
    let offset = sign * (number(28..30)? * 60 + number(31..33)?);
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + (minute - offset) * 60 + second;
    Ok(seconds * 10_000_000 + fraction)
}

fn settlement(name: &str) -> Parsed<SettlementKind> {
    SettlementKind::parse(name).ok_or_else(|| format!("unknown settlement kind '{name}'"))
}

/// A record as the core's `serde` form reads it, with `settled_at` given as ticks or as a time and an entry given as null
/// left out.
fn record(value: &Value) -> Parsed<Record> {
    let mut value = value.clone();
    // An entry given as null is one left out, as Python's None reads.
    if let Value::Object(entries) = &mut value {
        entries.retain(|_, v| !v.is_null());
    }
    if let Some(Value::String(text)) = value.get("settled_at") {
        let ticks = ticks(text)?;
        value["settled_at"] = json!(ticks);
    }
    serde_json::from_value(value).map_err(|e| format!("a record does not read: {e}"))
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
            .map(|(i, line)| {
                let value: Value =
                    serde_json::from_str(line).map_err(|e| format!("line {}: {e}", i + 1))?;
                record(&value).map_err(|e| format!("line {}: {e}", i + 1))
            })
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

/// Replays the records in order and chooses the attribute's key and memory thresholds, as `select_layers` does. Returns
/// the thresholds as JSON and, when `decisions` is true, the decision records as JSON lines.
#[pyfunction]
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, decisions=true, rule="every_band"))]
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
) -> PyResult<(String, Option<String>)> {
    let rule = match rule {
        "every_band" => ThresholdRule::EveryBand,
        "pooled" => ThresholdRule::Pooled,
        other => {
            return Err(invalid(format!(
                "unknown threshold rule '{other}' (every_band or pooled)"
            )));
        }
    };
    let (schema, counting) = selection_input(schema_json, counting)?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = guarded(|| {
        select_layers_by_rule_with(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &records.records,
            choice,
            rule,
            &mut |r| push_record(&mut lines, r),
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
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, decisions=true))]
fn select_coarse_json(
    schema_json: &str,
    attribute: &str,
    records: &Records,
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<Vec<String>>,
    decisions: bool,
) -> PyResult<(String, Option<String>)> {
    let (schema, counting) = selection_input(schema_json, counting)?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = guarded(|| {
        select_coarse_with(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &records.records,
            choice,
            &mut |r| push_record(&mut lines, r),
        )
    })?;
    Ok((to_json(&layers), lines))
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
    /// record memory unless `similar_records` is false.
    #[new]
    #[pyo3(signature = (schema_json, records=None, similar_records=true))]
    fn new(schema_json: &str, records: Option<&Records>, similar_records: bool) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let mut inner = gil::decider::Decider::new(
            KeyMemory::default(),
            similar_records.then(LexicalMemory::default),
        );
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
        let value: Value = serde_json::from_str(record_json).map_err(|e| invalid(e.to_string()))?;
        let settled = record(&value).map_err(invalid)?;
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
    #[new]
    fn new(schema_json: &str, id: &str, settled_at: i64) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        guarded(|| open_record::OpenRecord::new(schema, id, settled_at)).map(|inner| Self {
            inner,
            broken: false,
        })
    }

    /// A saved record (JSON, as a snapshot gives it) reopened as it was left.
    #[staticmethod]
    fn reopen(schema_json: &str, record_json: &str) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let value: Value = serde_json::from_str(record_json).map_err(|e| invalid(e.to_string()))?;
        let saved = record(&value).map_err(invalid)?;
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

    /// Settles a decided attribute at `now` (ticks) — `kind` by name, with the value, values or probability it needs;
    /// the answers it changes, as JSON.
    #[pyo3(signature = (decider, attribute, kind, now, value=None, values=None, probability=None))]
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
    ) -> PyResult<String> {
        usable(self.broken, "record")?;
        usable(decider.broken, "decider")?;
        let settlement = settlement_of(kind, value, values, probability).map_err(invalid)?;
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

/// The tuning with what `attribute`'s layer replay (JSON, as `select_layers_json` gives it) promised, as JSON.
#[pyfunction]
fn tuning_promise_json(tuning_json: &str, attribute: &str, layers_json: &str) -> PyResult<String> {
    let mut tuned = tuning(tuning_json)?;
    let layers: LayerThresholds = serde_json::from_str(layers_json)
        .map_err(|e| invalid(format!("a layer replay does not read: {e}")))?;
    guarded(|| tuned.promise(attribute, &layers))?;
    Ok(to_json(&tuned))
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

/// The usage counts of the decisions (a JSON list of `{"attribute", "answer", "settled", "settlement", "probability",
/// "promise"}`), by attribute, as JSON.
#[pyfunction]
fn usage_json(decisions_json: &str) -> PyResult<String> {
    let items: Value = serde_json::from_str(decisions_json).map_err(|e| invalid(e.to_string()))?;
    let items = items
        .as_array()
        .ok_or_else(|| invalid("decisions are a list".to_owned()))?;
    let text = |v: &Value, what: &str| -> Parsed<Option<String>> {
        match v {
            Value::Null => Ok(None),
            Value::String(s) => Ok(Some(s.clone())),
            _ => Err(format!("a decision's {what} is a string or None")),
        }
    };
    struct Owned {
        attribute: String,
        answer: Option<String>,
        settled: Option<String>,
        settlement: Option<SettlementKind>,
        probability: f64,
        promise: Option<f64>,
    }
    let owned = items
        .iter()
        .map(|d| {
            Ok(Owned {
                attribute: text(&d["attribute"], "attribute")?
                    .ok_or("a decision needs its attribute")?,
                answer: text(&d["answer"], "answer")?,
                settled: text(&d["settled"], "settled")?,
                settlement: text(&d["settlement"], "settlement")?
                    .map(|k| settlement(&k))
                    .transpose()?,
                probability: d.get("probability").and_then(Value::as_f64).unwrap_or(1.0),
                promise: d.get("promise").and_then(Value::as_f64),
            })
        })
        .collect::<Parsed<Vec<_>>>()
        .map_err(invalid)?;
    let counts = usage(owned.iter().map(|d| Decision {
        attribute: &d.attribute,
        answer: d.answer.as_deref(),
        settled: d.settled.as_deref(),
        settlement: d.settlement,
        probability: d.probability,
        promise: d.promise,
    }));
    Ok(serde_json::to_string(&counts).expect("counts serialize"))
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
    m.add_function(wrap_pyfunction!(tuning_capture_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_apply_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_promise_json, m)?)?;
    m.add_function(wrap_pyfunction!(tuning_stale_json, m)?)?;
    m.add_function(wrap_pyfunction!(usage_json, m)?)?;
    m.add_function(wrap_pyfunction!(check_schema, m)?)?;
    m.add_function(wrap_pyfunction!(select_layers_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_key_fields_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_coarse_json, m)?)?;
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
