//! The key layer golden (`tests/fixtures/core/keys.jsonl`), written by the .NET implementation: the settled field
//! memory's ranking for each single-valued judged field of every record after the first 200, with the settings the
//! golden chose (`golden.jsonl`), must come out the same here — values, scores to the bit, evidence and trust.

use std::collections::BTreeMap;

use gilfit::field_memory::FieldMemory;
use gilfit::form::{Document, Field, Form, Role};
use serde_json::{Value, json};

const CHOSEN: usize = 200;

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../tests/fixtures/core/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).expect("a core fixture")
}

fn lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("a JSON line"))
}

/// `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` → 100-nanosecond ticks since the Unix epoch, UTC.
fn ticks(text: &str) -> i64 {
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

/// A record as the field memory remembers it: values a program filled are left out.
fn remembered(record: &Value) -> Document {
    let filled: Vec<&str> = record["settlements"]
        .as_object()
        .expect("settlements")
        .iter()
        .filter(|(_, kind)| kind.as_str() == Some("Fill"))
        .map(|(field, _)| field.as_str())
        .collect();
    let values = record["values"]
        .as_object()
        .expect("values")
        .iter()
        .filter(|(field, _)| !filled.contains(&field.as_str()))
        .map(|(field, value)| (field.clone(), value.as_str().expect("a value").to_owned()))
        .collect();
    let sets = record["sets"]
        .as_object()
        .expect("sets")
        .iter()
        .filter(|(field, _)| !filled.contains(&field.as_str()))
        .map(|(field, values)| {
            let elements = values
                .as_array()
                .expect("a set")
                .iter()
                .map(|v| v.as_str().expect("an element").to_owned())
                .collect();
            (field.clone(), elements)
        })
        .collect();
    Document {
        id: record["id"].as_str().expect("id").to_owned(),
        values,
        sets,
        settled_at: ticks(record["settled_at"].as_str().expect("settled_at")),
    }
}

/// The golden's form with the settings its first lines chose for the key layer.
fn form(golden: &str) -> Form {
    let mut team = Field::new("team", Role::Judged);
    let mut code = Field::new("code", Role::Judged);
    let mut tags = Field::new("tags", Role::Judged);
    tags.multiple = true;
    for line in lines(golden).take_while(|l| l["kind"] != "decision") {
        let target = match line["field"].as_str() {
            Some("team") => &mut team,
            Some("code") => &mut code,
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

#[test]
fn the_key_layer_reproduces_its_golden() {
    let records: Vec<Value> = lines(&fixture("records.jsonl")).collect();
    let form = form(&fixture("golden.jsonl"));
    let expected: Vec<Value> = lines(&fixture("keys.jsonl")).collect();

    let mut memory = FieldMemory::default();
    for record in &records[..CHOSEN] {
        memory.put(&form, &remembered(record));
    }

    let mut at = 0;
    for record in &records[CHOSEN..] {
        let observed: BTreeMap<String, String> = record["values"]
            .as_object()
            .expect("values")
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.as_str().expect("a value").to_owned()))
            .collect();
        for field in ["team", "code"] {
            let candidates: Vec<Value> = memory
                .rank(&form, field, &observed, 3)
                .into_iter()
                .map(|c| json!([c.value, c.score, c.evidence, c.trusted]))
                .collect();
            let actual = json!({
                "kind": "key_rank",
                "doc": record["id"],
                "field": field,
                "candidates": candidates,
            });
            assert_eq!(actual, expected[at], "line {}", at + 1);
            at += 1;
        }

        memory.put(&form, &remembered(record));
    }

    assert_eq!(at, expected.len());
}

#[test]
fn ticks_count_from_the_unix_epoch_in_utc() {
    assert_eq!(ticks("1970-01-01T00:00:00.0000000+00:00"), 0);
    assert_eq!(ticks("1970-01-01T09:00:00.0000000+09:00"), 0);
    assert_eq!(
        ticks("2026-01-01T00:07:00.0000001+00:00"),
        17_672_260_200_000_001
    );
}
