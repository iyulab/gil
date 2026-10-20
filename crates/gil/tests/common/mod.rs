//! What the golden tests share: reading the core fixtures, their records as documents, and the golden's form.
#![allow(dead_code)] // each test binary uses its own part

use gil::form::{CoarseLevel, Document, Field, Form, Role, SettlementKind};
use gil::threshold_selection::{ThresholdChoice, ThresholdReplay};
use serde_json::{Number, Value, json};

/// How many records the golden chooses its settings on; decisions are made on the rest.
pub const CHOSEN: usize = 200;

/// A core fixture's text.
pub fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../tests/fixtures/core/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).expect("a core fixture")
}

/// A fixture's JSON lines.
pub fn lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("a JSON line"))
}

/// `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` → 100-nanosecond ticks since the Unix epoch, UTC.
pub fn ticks(text: &str) -> i64 {
    let number = |range: std::ops::Range<usize>| -> i64 { text[range].parse().expect("a number") };
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let (hour, minute, second, fraction) = (
        number(11..13),
        number(14..16),
        number(17..19),
        number(20..27),
    );
    let sign = if &text[27..28] == "-" { -1 } else { 1 };
    let offset = sign * (number(28..30) * 60 + number(31..33));
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + (minute - offset) * 60 + second;
    seconds * 10_000_000 + fraction
}

/// A record as settled: its values, sets, settlements, draw probabilities and, where it has one, its arrival order.
pub fn document(record: &Value) -> Document {
    let object = |name: &str| record[name].as_object().expect(name);
    Document {
        id: record["id"].as_str().expect("id").to_owned(),
        values: object("values")
            .iter()
            .map(|(field, value)| (field.clone(), value.as_str().expect("a value").to_owned()))
            .collect(),
        sets: object("sets")
            .iter()
            .map(|(field, values)| {
                let elements = values
                    .as_array()
                    .expect("a set")
                    .iter()
                    .map(|v| v.as_str().expect("an element").to_owned())
                    .collect();
                (field.clone(), elements)
            })
            .collect(),
        settled_at: ticks(record["settled_at"].as_str().expect("settled_at")),
        settlements: object("settlements")
            .iter()
            .map(|(field, kind)| {
                let name = kind.as_str().expect("a kind");
                let kind = SettlementKind::parse(name)
                    .unwrap_or_else(|| panic!("an unknown settlement kind '{name}'"));
                (field.clone(), kind)
            })
            .collect(),
        probabilities: object("probabilities")
            .iter()
            .map(|(field, p)| (field.clone(), p.as_f64().expect("a probability")))
            .collect(),
        arrival: record
            .get("arrival")
            .and_then(Value::as_array)
            .map(|fields| {
                fields
                    .iter()
                    .map(|f| f.as_str().expect("a field").to_owned())
                    .collect()
            }),
    }
}

/// The golden's form with the key layer settings its lines chose before the first line `stop` holds for (and before
/// the decisions).
pub fn form(golden: &str, stop: impl Fn(&Value) -> bool) -> Form {
    let mut team = Field::new("team", Role::Judged);
    let mut code = Field::new("code", Role::Judged);
    code.coarse = Some(CoarseLevel::by_prefix(2));
    let mut tags = Field::new("tags", Role::Judged);
    tags.multiple = true;
    for line in lines(golden).take_while(|l| l["kind"] != "decision" && !stop(l)) {
        let target = match line["field"].as_str() {
            Some("team") => &mut team,
            Some("code") => &mut code,
            Some("tags") => &mut tags,
            _ => continue,
        };
        match line["kind"].as_str() {
            Some("key_fields") => {
                target.key_fields = line["fields"].as_array().map(|fields| {
                    fields
                        .iter()
                        .map(|f| f.as_str().expect("a field").to_owned())
                        .collect()
                });
            }
            Some("key_concentrations") => {
                target.key_concentrations = line["concentrations"]
                    .as_object()
                    .expect("concentrations")
                    .iter()
                    .map(|(field, value)| (field.clone(), value.as_f64().expect("a number")))
                    .collect();
            }
            Some("layers") => {
                target.key_threshold = line["key"]["chosen"]["threshold"].as_f64();
                target.memory_threshold = line["memory"]["chosen"]["threshold"].as_f64();
                target.memory_similarity_floor =
                    line["memory"]["chosen"]["similarity_floor"].as_f64();
            }
            Some("key") => {
                target.key_threshold = line["replay"]["chosen"]["threshold"].as_f64();
            }
            Some("coarse") => {
                let level = target.coarse.as_mut().expect("a coarse level");
                level.key_threshold = line["key"]["chosen"]["threshold"].as_f64();
                level.memory_threshold = line["memory"]["chosen"]["threshold"].as_f64();
                level.memory_similarity_floor =
                    line["memory"]["chosen"]["similarity_floor"].as_f64();
            }
            _ => {}
        }
    }

    Form {
        name: "ticket".to_owned(),
        fields: vec![
            Field::new("channel", Role::Observed),
            Field::new("product", Role::Observed),
            Field::new("summary", Role::Observed),
            team,
            code,
            tags,
        ],
    }
}

/// The settlement kinds the golden counts.
pub const COUNTED: [SettlementKind; 4] = [
    SettlementKind::Enter,
    SettlementKind::Accept,
    SettlementKind::Correct,
    SettlementKind::Check,
];

/// The replay in the golden's shape.
pub fn replay(replay: &ThresholdReplay) -> Value {
    json!({
        "lookups": replay.lookups,
        "candidates": replay.candidates,
        "chosen": replay.chosen.as_ref().map(choice),
        "most_precise": replay.most_precise.as_ref().map(choice),
        "keys": replay.keys.as_ref().map(|keys| keys
            .iter()
            .map(|k| json!([k.key, k.lookups, k.answered, k.precision, k.answering]))
            .collect::<Vec<_>>()),
    })
}

pub fn choice(choice: &ThresholdChoice) -> Value {
    json!({
        "threshold": choice.threshold,
        "precision": choice.precision,
        "answered": choice.answered,
        "lookups": choice.lookups,
        "answered_weight": choice.answered_weight,
        "effective_answered": choice.effective_answered,
        "similarity_floor": choice.similarity_floor,
        "promise": choice.promise,
        "promise_answered": choice.promise_answered,
    })
}

/// Every number as the double it denotes: the reference writes a whole double without a fraction (`160`), which reads
/// back as an integer, so both sides compare as doubles — exactly.
pub fn doubles(value: Value) -> Value {
    match value {
        Value::Number(n) => {
            Value::Number(Number::from_f64(n.as_f64().expect("a number")).expect("a finite number"))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(doubles).collect()),
        Value::Object(fields) => {
            Value::Object(fields.into_iter().map(|(k, v)| (k, doubles(v))).collect())
        }
        other => other,
    }
}
