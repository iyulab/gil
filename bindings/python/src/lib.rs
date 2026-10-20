//! Python bindings for the Gil decision core.
//!
//! The extension module (`gil._gil`) takes schemas and records as JSON and hands results back as JSON text; the
//! `gil` package around it reads that text into dictionaries. Records are gathered and returned whole rather than
//! passed to a Python callback, which would cost a call back into the interpreter per lookup.

use std::panic::{AssertUnwindSafe, catch_unwind};

use gil::decider::{Answer, ExternalCandidate};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record;
use gil::replay::{
    Choice, LayerThresholds, ThresholdRule, select_coarse_with, select_key_concentrations,
    select_key_fields, select_layers_by_rule_with,
};
use gil::schema::{AttributeRole, Record, Schema, Settlement, SettlementKind};
use gil::tuning::Tuning;
use gil::usage::{Decision, usage};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::{Value, json};

type Parsed<T> = Result<T, String>;

fn invalid(message: String) -> PyErr {
    PyValueError::new_err(message)
}

// ---- reading ----

/// A schema as the core's `serde` form reads it — the settings an attribute leaves out take `Attribute::new`'s defaults —
/// checked as the reference implementation checks one.
fn schema(text: &str) -> Parsed<Schema> {
    let schema: Schema =
        serde_json::from_str(text).map_err(|e| format!("a schema does not read: {e}"))?;
    schema.check()?;
    Ok(schema)
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

/// The schema, checked to have `attribute` as a decided attribute.
fn deciding(schema_json: &str, attribute: &str) -> PyResult<Schema> {
    let schema = schema(schema_json).map_err(invalid)?;
    match schema.attributes.iter().find(|f| f.name == attribute) {
        Some(f) if f.role == AttributeRole::Decided => Ok(schema),
        Some(_) => Err(invalid(format!("attribute '{attribute}' is not decided"))),
        None => Err(invalid(format!(
            "the schema has no attribute '{attribute}'"
        ))),
    }
}

/// The schema, checked to judge `attribute`, and the counted settlement kinds, from the arguments every selection takes.
fn selection_input(
    schema_json: &str,
    attribute: &str,
    target_precision: f64,
    minimum_answered: usize,
    counting: Option<Vec<String>>,
) -> PyResult<(Schema, Option<Vec<SettlementKind>>)> {
    let schema = deciding(schema_json, attribute)?;
    if !(target_precision > 0.0 && target_precision <= 1.0) || minimum_answered == 0 {
        return Err(invalid(
            "target_precision is in (0, 1] and minimum_answered at least 1".to_owned(),
        ));
    }
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
    let (schema, counting) = selection_input(
        schema_json,
        attribute,
        target_precision,
        minimum_answered,
        counting,
    )?;
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = select_layers_by_rule_with(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        attribute,
        &records.records,
        choice,
        rule,
        &mut |r| push_record(&mut lines, r),
    );
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
    let (schema, counting) = selection_input(
        schema_json,
        attribute,
        target_precision,
        minimum_answered,
        counting,
    )?;
    let definition = schema.attribute(attribute);
    if definition.multiple {
        return Err(invalid(format!(
            "'{attribute}' takes several values; it has no coarse level"
        )));
    }
    if definition.coarse.is_none() {
        return Err(invalid(format!("'{attribute}' has no coarse level")));
    }
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = select_coarse_with(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        attribute,
        &records.records,
        choice,
        &mut |r| push_record(&mut lines, r),
    );
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
    let schema = deciding(schema_json, attribute)?;
    let found = select_key_concentrations(
        &mut KeyMemory::default(),
        &schema,
        attribute,
        &records.records,
    );
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
    let (schema, counting) = selection_input(
        schema_json,
        attribute,
        target_precision,
        minimum_answered,
        counting,
    )?;
    if typed_longest > 0 && schema.attribute(attribute).multiple {
        return Err(invalid(format!(
            "'{attribute}' takes several values; typing is not counted for it"
        )));
    }
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let chosen = select_key_fields(&schema, attribute, &records.records, choice, typed_longest);
    Ok(to_json(&chosen))
}

/// Runs a core call, turning a broken contract (the core panics on one) into a `ValueError` with the core's message.
fn guarded<T>(call: impl FnOnce() -> T) -> PyResult<T> {
    catch_unwind(AssertUnwindSafe(call)).map_err(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "the core refused the call".to_owned());
        invalid(message)
    })
}

/// The memories that answer: the settled key memory and, unless left out, the lexical similar record memory.
#[pyclass(module = "gil._gil", unsendable)]
struct Decider {
    inner: gil::decider::Decider,
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
            inner.rebuild(&schema, &records.records);
        }
        Ok(Self { inner })
    }
}

fn settlement_of(
    kind: &str,
    value: Option<&str>,
    values: Option<Vec<String>>,
    probability: Option<f64>,
) -> Parsed<Settlement> {
    let kind = settlement(kind)?;
    let one = || value.ok_or_else(|| format!("a {kind:?} settlement needs a value"));
    let many = || {
        values
            .clone()
            .ok_or_else(|| format!("a {kind:?} settlement needs values"))
    };
    Ok(match kind {
        SettlementKind::Accept => Settlement::accept(one()?),
        SettlementKind::Correct => Settlement::correct(one()?),
        SettlementKind::Enter => Settlement::enter(one()?),
        SettlementKind::Fill => Settlement::fill(one()?),
        SettlementKind::AcceptOther => Settlement::accept_other(one()?),
        SettlementKind::CorrectOther => Settlement::correct_other(one()?),
        SettlementKind::Restore if values.is_some() => {
            Settlement::restore_set(many()?.iter().map(String::as_str))
        }
        SettlementKind::Restore => Settlement::restore(one()?),
        SettlementKind::Set => Settlement::set(many()?.iter().map(String::as_str)),
        SettlementKind::Check => {
            let p =
                probability.ok_or("a Check settlement needs the probability it was drawn with")?;
            if !(p > 0.0 && p <= 1.0) {
                return Err("a check is drawn with a probability above 0 and at most 1".to_owned());
            }
            Settlement::check(one()?, p)
        }
        SettlementKind::Reject => Settlement::reject(),
        SettlementKind::Revert => Settlement::revert(),
    })
}

/// One record of a schema while its values arrive. Each change puts it into the decider's memory again and returns the
/// fresh answers it causes, as JSON.
#[pyclass(module = "gil._gil", unsendable)]
struct OpenRecord {
    inner: open_record::OpenRecord,
}

#[pymethods]
impl OpenRecord {
    /// A new record `id`, starting at `settled_at` (100-nanosecond ticks since the Unix epoch, UTC).
    #[new]
    fn new(schema_json: &str, id: &str, settled_at: i64) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        guarded(|| open_record::OpenRecord::new(schema, id, settled_at)).map(|inner| Self { inner })
    }

    /// A saved record (JSON, as a snapshot gives it) reopened as it was left.
    #[staticmethod]
    fn reopen(schema_json: &str, record_json: &str) -> PyResult<Self> {
        let schema = schema(schema_json).map_err(invalid)?;
        let value: Value = serde_json::from_str(record_json).map_err(|e| invalid(e.to_string()))?;
        let saved = record(&value).map_err(invalid)?;
        guarded(|| open_record::OpenRecord::reopen(schema, &saved)).map(|inner| Self { inner })
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
        let decider = &mut decider.inner;
        guarded(|| self.inner.observe(decider, attribute, value)).map(|a| answers_json(&a))
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
        let settlement = settlement_of(kind, value, values, probability).map_err(invalid)?;
        let decider = &mut decider.inner;
        guarded(|| self.inner.settle(decider, attribute, &settlement, now))
            .map(|a| answers_json(&a))
    }

    /// Answers for every open decided attribute, as JSON.
    fn answer(&self, decider: PyRef<'_, Decider>) -> PyResult<String> {
        guarded(|| self.inner.answer(&decider.inner)).map(|a| answers_json(&a))
    }

    /// The answer for one open decided attribute alone — with `partial`, the text typed into it so far — as JSON.
    #[pyo3(signature = (decider, attribute, partial=None))]
    fn answer_partial(
        &self,
        decider: PyRef<'_, Decider>,
        attribute: &str,
        partial: Option<&str>,
    ) -> PyResult<String> {
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
    fn snapshot(&self) -> String {
        serde_json::to_string(&self.inner.snapshot()).expect("a record serializes")
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
    let mut schema = schema(schema_json).map_err(invalid)?;
    tuning(tuning_json)?.apply(&mut schema);
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
    Ok(())
}
