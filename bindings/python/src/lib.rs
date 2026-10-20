//! Python bindings for the Gil decision core.
//!
//! The extension module (`gil._gil`) takes schemas and records as JSON and hands results back as JSON text; the
//! `gil` package around it reads that text into dictionaries. Records are gathered and returned whole rather than
//! passed to a Python callback, which would cost a call back into the interpreter per lookup.

use std::collections::BTreeMap;

use gil::decision_record::{DecisionRecord, KeyLook, SimilarLook};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, KeyFieldsChoice, KeyFieldsTrial, KeyState, LayerThresholds, ThresholdChoice,
    ThresholdReplay, TypedKeyThresholds, select_coarse_with, select_key_concentrations,
    select_key_fields, select_layers_with,
};
use gil::schema::{
    Attribute, AttributePolicy, AttributeRole, CoarseLevel, Record, Schema, SettlementKind,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::{Map, Value, json};

type Parsed<T> = Result<T, String>;

fn invalid(message: String) -> PyErr {
    PyValueError::new_err(message)
}

// ---- reading ----

fn object<'a>(value: &'a Value, what: &str) -> Parsed<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| format!("{what} must be an object"))
}

fn string(value: &Value, what: &str) -> Parsed<String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{what} must be a string"))
}

fn strings(value: &Value, what: &str) -> Parsed<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| format!("{what} must be a list"))?
        .iter()
        .map(|v| string(v, what))
        .collect()
}

fn number(value: &Value, what: &str) -> Parsed<f64> {
    value
        .as_f64()
        .ok_or_else(|| format!("{what} must be a number"))
}

fn optional_number(value: &Value, what: &str) -> Parsed<Option<f64>> {
    if value.is_null() {
        Ok(None)
    } else {
        number(value, what).map(Some)
    }
}

fn count(value: &Value, what: &str) -> Parsed<usize> {
    value
        .as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| format!("{what} must be a whole number"))
}

fn coarse(value: &Value) -> Parsed<CoarseLevel> {
    let mut level = CoarseLevel::by_prefix(0);
    for (name, v) in object(value, "coarse")? {
        match name.as_str() {
            "prefix" => level.prefix = count(v, "coarse.prefix")?,
            "classes" => {
                level.classes = Some(
                    object(v, "coarse.classes")?
                        .iter()
                        .map(|(value, class)| Ok((value.clone(), string(class, "a class")?)))
                        .collect::<Parsed<_>>()?,
                );
            }
            "key_threshold" => level.key_threshold = optional_number(v, "coarse.key_threshold")?,
            "memory_threshold" => {
                level.memory_threshold = optional_number(v, "coarse.memory_threshold")?;
            }
            "memory_floor" => {
                level.memory_floor = optional_number(v, "coarse.memory_similarity_floor")?;
            }
            other => return Err(format!("unknown coarse setting '{other}'")),
        }
    }
    Ok(level)
}

fn attribute(value: &Value) -> Parsed<Attribute> {
    let settings = object(value, "an attribute")?;
    let name = string(
        settings.get("name").ok_or("an attribute needs a name")?,
        "an attribute's name",
    )?;
    let role = match settings.get("role").and_then(Value::as_str) {
        Some("observed") => AttributeRole::Observed,
        Some("decided") => AttributeRole::Decided,
        _ => return Err(format!("field '{name}' needs a role: observed or decided")),
    };
    let mut f = Attribute::new(&name, role);
    for (setting, v) in settings {
        let what = format!("{name}.{setting}");
        match setting.as_str() {
            "name" | "role" => {}
            "multiple" => {
                f.multiple = v.as_bool().ok_or(format!("{what} must be true or false"))?
            }
            "use_as_evidence" => {
                f.use_as_evidence = v.as_bool().ok_or(format!("{what} must be true or false"))?;
            }
            "depends_on" => f.depends_on = (!v.is_null()).then(|| strings(v, &what)).transpose()?,
            "key_fields" => f.key_fields = (!v.is_null()).then(|| strings(v, &what)).transpose()?,
            "key_concentrations" => {
                f.key_concentrations = object(v, &what)?
                    .iter()
                    .map(|(k, c)| Ok((k.clone(), number(c, &what)?)))
                    .collect::<Parsed<_>>()?;
            }
            "key_threshold" => f.key_threshold = optional_number(v, &what)?,
            "similar_votes" => f.similar_votes = count(v, &what)?,
            "memory_threshold" => f.memory_threshold = optional_number(v, &what)?,
            "memory_floor" => f.memory_floor = optional_number(v, &what)?,
            "coarse" => f.coarse = (!v.is_null()).then(|| coarse(v)).transpose()?,
            "candidates" => f.candidates = (!v.is_null()).then(|| strings(v, &what)).transpose()?,
            "must_catch" => f.must_catch = (!v.is_null()).then(|| strings(v, &what)).transpose()?,
            "policy" => {
                f.policy = match v.as_str() {
                    Some("suggest") => AttributePolicy::Suggest,
                    Some("confirm_required") => AttributePolicy::ConfirmRequired,
                    Some("off") => AttributePolicy::Off,
                    _ => {
                        return Err(format!(
                            "{what}: a policy is \"suggest\", \"confirm_required\" or \"off\""
                        ));
                    }
                }
            }
            other => return Err(format!("unknown setting '{other}' on attribute '{name}'")),
        }
    }
    Ok(f)
}

fn schema(text: &str) -> Parsed<Schema> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("a schema is JSON: {e}"))?;
    let settings = object(&value, "a schema")?;
    let name = string(
        settings.get("name").ok_or("a schema needs a name")?,
        "a schema's name",
    )?;
    let attributes = settings
        .get("attributes")
        .and_then(Value::as_array)
        .ok_or("a schema needs a list of attributes")?
        .iter()
        .map(attribute)
        .collect::<Parsed<Vec<_>>>()?;
    if let Some(other) = settings.keys().find(|k| *k != "name" && *k != "attributes") {
        return Err(format!("unknown schema setting '{other}'"));
    }
    Ok(Schema { name, attributes })
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

fn record(value: &Value) -> Parsed<Record> {
    let record = object(value, "a record")?;
    let id = string(
        record.get("id").ok_or("a record needs an id")?,
        "a record's id",
    )?;
    let at = record
        .get("settled_at")
        .ok_or(format!("record '{id}' needs settled_at"))?;
    let settled_at = match at {
        Value::String(text) => ticks(text)?,
        Value::Number(n) => n
            .as_i64()
            .ok_or(format!("record '{id}': settled_at ticks must be whole"))?,
        _ => return Err(format!("record '{id}': settled_at is a time or ticks")),
    };
    let map = |name: &str| -> Parsed<Map<String, Value>> {
        match record.get(name) {
            None | Some(Value::Null) => Ok(Map::new()),
            Some(v) => object(v, name).cloned(),
        }
    };
    let values = map("values")?
        .iter()
        .map(|(f, v)| Ok((f.clone(), string(v, "a value")?)))
        .collect::<Parsed<BTreeMap<_, _>>>()?;
    let sets = map("sets")?
        .iter()
        .map(|(f, v)| Ok((f.clone(), strings(v, "a set")?)))
        .collect::<Parsed<BTreeMap<_, _>>>()?;
    let settlements = map("settlements")?
        .iter()
        .map(|(f, v)| Ok((f.clone(), settlement(&string(v, "a settlement kind")?)?)))
        .collect::<Parsed<BTreeMap<_, _>>>()?;
    let probabilities = map("probabilities")?
        .iter()
        .map(|(f, v)| Ok((f.clone(), number(v, "a probability")?)))
        .collect::<Parsed<BTreeMap<_, _>>>()?;
    let arrival = match record.get("arrival") {
        None | Some(Value::Null) => None,
        Some(v) => Some(strings(v, "arrival")?),
    };
    for key in record.keys() {
        if ![
            "id",
            "settled_at",
            "values",
            "sets",
            "settlements",
            "probabilities",
            "arrival",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("record '{id}': unknown entry '{key}'"));
        }
    }
    Ok(Record {
        id,
        values,
        sets,
        settled_at,
        settlements,
        probabilities,
        arrival,
    })
}

// ---- writing ----

fn choice_json(c: &ThresholdChoice) -> Value {
    json!({
        "threshold": c.threshold,
        "precision": c.precision,
        "answer_rate": c.answer_rate,
        "answered": c.answered,
        "lookups": c.lookups,
        "answered_weight": c.answered_weight,
        "effective_answered": c.effective_answered,
        "similarity_floor": c.similarity_floor,
        "promise": c.promise,
        "promise_answered": c.promise_answered,
    })
}

fn key_state_json(k: &KeyState) -> Value {
    json!({
        "key": k.key,
        "lookups": k.lookups,
        "answered": k.answered,
        "precision": k.precision,
        "answering": k.answering,
        "answered_weight": k.answered_weight,
        "effective_answered": k.effective_answered,
    })
}

fn replay_json(r: &ThresholdReplay) -> Value {
    json!({
        "chosen": r.chosen.as_ref().map(choice_json),
        "most_precise": r.most_precise.as_ref().map(choice_json),
        "lookups": r.lookups,
        "candidates": r.candidates,
        "keys": r.keys.as_ref().map(|keys| keys.iter().map(key_state_json).collect::<Vec<_>>()),
    })
}

fn layers_json(l: &LayerThresholds) -> Value {
    json!({ "key": replay_json(&l.key), "memory": replay_json(&l.memory) })
}

fn typed_json(t: &TypedKeyThresholds) -> Value {
    json!({ "by_length": t.by_length.iter().map(replay_json).collect::<Vec<_>>() })
}

fn trial_json(t: &KeyFieldsTrial) -> Value {
    json!({
        "key_fields": t.key_fields,
        "replay": replay_json(&t.replay),
        "typed": t.typed.as_ref().map(typed_json),
    })
}

fn key_fields_json(c: &KeyFieldsChoice) -> Value {
    json!({
        "key_fields": c.key_fields,
        "chosen": replay_json(&c.chosen),
        "all_fields": replay_json(&c.all_fields),
        "trials": c.trials.iter().map(trial_json).collect::<Vec<_>>(),
        "typed": c.typed.as_ref().map(typed_json),
    })
}

fn key_look_json(k: &KeyLook) -> Value {
    json!({
        "strength": k.strength,
        "value": k.value,
        "correct": k.correct,
        "key": k.key,
        "caught": k.caught,
    })
}

fn similar_look_json(s: &SimilarLook) -> Value {
    json!({
        "score": s.score,
        "value": s.value,
        "record": s.record,
        "correct": s.correct,
        "nearest": s.nearest,
        "voted": s.voted,
    })
}

fn record_json(r: &DecisionRecord) -> Value {
    json!({
        "record": r.record,
        "attribute": r.attribute,
        "order": r.order,
        "weight": r.weight,
        "key": r.key.as_ref().map(key_look_json),
        "similar": r.similar.as_ref().map(similar_look_json),
        "coarse": r.coarse.as_ref().map(|c| json!({
            "value_answered": c.value_answered,
            "keys": c.keys.as_ref().map(key_look_json),
            "vote": c.vote.as_ref().map(similar_look_json),
        })),
        "settled": r.settled,
        "settlement": r.settlement.map(|k| format!("{k:?}")),
        "probability": r.probability,
    })
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
#[pyo3(signature = (schema_json, attribute, records, target_precision, minimum_answered, counting=None, decisions=true))]
fn select_layers_json(
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
    let choice = Choice {
        target_precision,
        minimum_answered,
        counting: counting.as_deref(),
    };
    let mut lines = decisions.then(String::new);
    let layers = select_layers_with(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        attribute,
        &records.records,
        choice,
        &mut |r| push_record(&mut lines, r),
    );
    Ok((layers_json(&layers).to_string(), lines))
}

fn push_record(lines: &mut Option<String>, r: &DecisionRecord) {
    if let Some(out) = lines.as_mut() {
        out.push_str(&record_json(r).to_string());
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
    Ok((layers_json(&layers).to_string(), lines))
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
    Ok(key_fields_json(&chosen).to_string())
}

#[pymodule]
fn _gil(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Records>()?;
    m.add_function(wrap_pyfunction!(check_schema, m)?)?;
    m.add_function(wrap_pyfunction!(select_layers_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_key_fields_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_coarse_json, m)?)?;
    m.add_function(wrap_pyfunction!(select_key_concentrations_json, m)?)?;
    Ok(())
}
