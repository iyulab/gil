//! What the golden tests share: reading the core fixtures, their logs as records, and the golden's schema.
#![allow(dead_code)] // each test binary uses its own part

use gil::replay::{ThresholdChoice, ThresholdReplay};
use gil::schema::{Attribute, AttributeRole, CoarseLevel, Record, Schema, SettlementKind};
use serde_json::{Number, Value, json};

/// How many logs the golden chooses its settings on; decisions are made on the rest.
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

/// A log as settled: its values, sets, settlements, draw probabilities and, where it has one, its arrival order.
pub fn to_record(log: &Value) -> Record {
    let object = |name: &str| log[name].as_object().expect(name);
    Record {
        id: log["id"].as_str().expect("id").to_owned(),
        values: object("values")
            .iter()
            .map(|(attribute, value)| {
                (
                    attribute.clone(),
                    value.as_str().expect("a value").to_owned(),
                )
            })
            .collect(),
        sets: object("sets")
            .iter()
            .map(|(attribute, values)| {
                let elements = values
                    .as_array()
                    .expect("a set")
                    .iter()
                    .map(|v| v.as_str().expect("an element").to_owned())
                    .collect();
                (attribute.clone(), elements)
            })
            .collect(),
        settled_at: ticks(log["settled_at"].as_str().expect("settled_at")),
        settlements: object("settlements")
            .iter()
            .map(|(attribute, kind)| {
                let name = kind.as_str().expect("a kind");
                let kind = SettlementKind::parse(name)
                    .unwrap_or_else(|| panic!("an unknown settlement kind '{name}'"));
                (attribute.clone(), kind)
            })
            .collect(),
        probabilities: object("probabilities")
            .iter()
            .map(|(attribute, p)| (attribute.clone(), p.as_f64().expect("a probability")))
            .collect(),
        arrival: log
            .get("arrival")
            .and_then(Value::as_array)
            .map(|attributes| {
                attributes
                    .iter()
                    .map(|f| f.as_str().expect("a field").to_owned())
                    .collect()
            }),
        batch: log.get("batch").and_then(Value::as_str).map(str::to_owned),
        ..Record::default()
    }
}

/// The golden's schema with the key layer settings its lines chose before the first line `stop` holds for (and before
/// the decisions).
pub fn schema(golden: &str, stop: impl Fn(&Value) -> bool) -> Schema {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    let mut code = Attribute::new("code", AttributeRole::Decided);
    code.coarse = Some(CoarseLevel::by_prefix(2));
    let mut tags = Attribute::new("tags", AttributeRole::Decided);
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
                target.key_fields = line["fields"].as_array().map(|attributes| {
                    attributes
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
                    .map(|(attribute, value)| {
                        (attribute.clone(), value.as_f64().expect("a number"))
                    })
                    .collect();
            }
            Some("layers") => {
                target.key_threshold = line["key"]["chosen"]["threshold"].as_f64();
                target.memory_threshold = line["memory"]["chosen"]["threshold"].as_f64();
                target.memory_floor = line["memory"]["chosen"]["similarity_floor"].as_f64();
            }
            Some("key") => {
                target.key_threshold = line["replay"]["chosen"]["threshold"].as_f64();
            }
            Some("coarse") => {
                let level = target.coarse.as_mut().expect("a coarse level");
                level.key_threshold = line["key"]["chosen"]["threshold"].as_f64();
                level.memory_threshold = line["memory"]["chosen"]["threshold"].as_f64();
                level.memory_floor = line["memory"]["chosen"]["similarity_floor"].as_f64();
            }
            _ => {}
        }
    }

    Schema::new(
        "ticket",
        vec![
            Attribute::new("channel", AttributeRole::Observed),
            Attribute::new("product", AttributeRole::Observed),
            Attribute::new("summary", AttributeRole::Observed),
            team,
            code,
            tags,
        ],
    )
    .expect("a valid schema")
}

/// The settlement kinds the golden counts.
pub const COUNTED: [SettlementKind; 6] = [
    SettlementKind::Enter,
    SettlementKind::Accept,
    SettlementKind::Correct,
    SettlementKind::Check,
    SettlementKind::AcceptOther,
    SettlementKind::CorrectOther,
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

/// Every number as the double it denotes, so a line compares by the values it holds rather than by how a writer spelled
/// them (`160` reads back as an integer, `160.0` as a double) — exactly.
pub fn doubles(value: Value) -> Value {
    match value {
        Value::Number(n) => {
            Value::Number(Number::from_f64(n.as_f64().expect("a number")).expect("a finite number"))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(doubles).collect()),
        Value::Object(attributes) => Value::Object(
            attributes
                .into_iter()
                .map(|(k, v)| (k, doubles(v)))
                .collect(),
        ),
        other => other,
    }
}

/// A golden output: checked line by line against its fixture or — with `GIL_WRITE_CORE_GOLDEN` set to a folder — written
/// afresh there, one JSON line per line, so that a change the golden turns on is reviewed as a diff of the fixture.
pub struct Golden {
    name: &'static str,
    /// For a part of a golden several tests write: the file it is written to, beside the others (`scripts/
    /// regen-goldens.sh` merges the parts in the fixture's order).
    part: Option<String>,
    expected: Option<Vec<Value>>,
    written: Vec<Value>,
}

impl Golden {
    /// The golden `name`, its fixture read unless it is being written.
    pub fn open(name: &'static str) -> Self {
        let expected = std::env::var_os("GIL_WRITE_CORE_GOLDEN")
            .is_none()
            .then(|| lines(&fixture(name)).collect());
        Self {
            name,
            part: None,
            expected,
            written: Vec::new(),
        }
    }

    /// The lines of the golden `name` whose kind is one of `kinds` — the part one test checks or writes, where other
    /// tests write the rest. Written to `<name>.<first kind>.part`.
    pub fn part(name: &'static str, kinds: &[&str]) -> Self {
        let expected = std::env::var_os("GIL_WRITE_CORE_GOLDEN")
            .is_none()
            .then(|| {
                lines(&fixture(name))
                    .filter(|l| kinds.iter().any(|k| l["kind"] == *k))
                    .collect()
            });
        Self {
            name,
            part: Some(format!("{name}.{}.part", kinds[0])),
            expected,
            written: Vec::new(),
        }
    }

    /// The next line: the same as the fixture's (numbers compared as doubles, exactly), or kept to write.
    pub fn check(&mut self, actual: Value, what: impl std::fmt::Display) {
        if let Some(expected) = &self.expected {
            let at = self.written.len();
            assert!(
                at < expected.len(),
                "{}: more lines than the golden ({what})",
                self.name
            );
            assert_eq!(
                doubles(actual.clone()),
                doubles(expected[at].clone()),
                "{} line {}: {what}",
                self.name,
                at + 1
            );
        }
        self.written.push(actual);
    }

    /// How many lines were checked or kept.
    pub fn len(&self) -> usize {
        self.written.len()
    }

    /// Checks that every line of the fixture came, or writes the file.
    pub fn finish(self) {
        match self.expected {
            Some(expected) => assert_eq!(
                self.written.len(),
                expected.len(),
                "{}: fewer lines than the golden",
                self.name
            ),
            None => {
                let folder =
                    std::env::var_os("GIL_WRITE_CORE_GOLDEN").expect("a folder to write to");
                std::fs::create_dir_all(&folder).expect("the folder");
                let text: String = self
                    .written
                    .iter()
                    .map(|l| format!("{}\n", line_safe(&l.to_string())))
                    .collect();
                let file = self.part.as_deref().unwrap_or(self.name);
                std::fs::write(std::path::Path::new(&folder).join(file), text)
                    .expect("the golden written");
            }
        }
    }
}

/// A JSON line with the characters a line reader may split at besides `\n` — NEL, LINE SEPARATOR, PARAGRAPH SEPARATOR
/// (`str.splitlines` among others) — written as escapes. They only occur inside strings, where an escape is the same text.
fn line_safe(line: &str) -> String {
    let mut safe = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            '\u{85}' | '\u{2028}' | '\u{2029}' => safe.push_str(&format!("\\u{:04X}", c as u32)),
            _ => safe.push(c),
        }
    }
    safe
}
