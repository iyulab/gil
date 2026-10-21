//! The fixtures a binding's own tests read (`tests/fixtures/core/native/`): the schema path's golden schema and its
//! records in the core's `serde` form — what `gil_schema_new` and `gil_records_new` take — written from the same
//! sources the core's golden tests read. This test fails when they are out of date; `GIL_WRITE_FIXTURES=1` rewrites them.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use common::{fixture, lines, schema, to_record};

fn check_or_write(name: &str, expected: &str) {
    let path = format!(
        "{}/../../tests/fixtures/core/native/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    if std::env::var_os("GIL_WRITE_FIXTURES").is_some() {
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, expected).unwrap();
        return;
    }
    let actual = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{name} is missing; run with GIL_WRITE_FIXTURES=1"));
    assert!(
        actual == expected,
        "{name} is out of date; run with GIL_WRITE_FIXTURES=1"
    );
}

#[test]
fn the_native_fixtures_are_up_to_date() {
    let golden_schema = schema(&fixture("golden.jsonl"), |_| false);
    check_or_write(
        "schema.json",
        &(serde_json::to_string_pretty(&golden_schema).unwrap() + "\n"),
    );
    let records: String = lines(&fixture("records.jsonl"))
        .map(|r| serde_json::to_string(&to_record(&r)).unwrap() + "\n")
        .collect();
    check_or_write("records.jsonl", &records);
}
