//! The text golden (`tests/fixtures/core/text.jsonl`): every line's normalized text, key and n-grams must come out the
//! same; with `GIL_WRITE_CORE_GOLDEN` set the lines are written afresh from the texts the fixture holds.

mod common;

use common::{Golden, fixture, lines};
use serde_json::json;

const MAX_KEY_LENGTH: usize = 60;
const MIN_GRAM: usize = 2;
const MAX_GRAM: usize = 3;

#[test]
fn the_text_normalization_reproduces_its_golden() {
    let texts: Vec<String> = lines(&fixture("text.jsonl"))
        .map(|l| l["text"].as_str().expect("text").to_owned())
        .collect();
    let mut golden = Golden::open("text.jsonl");
    for text in &texts {
        let grams: Vec<_> = gil::text::grams(text, MIN_GRAM, MAX_GRAM)
            .into_iter()
            .map(|(gram, count)| json!([gram, count]))
            .collect();
        let actual = json!({
            "text": text,
            "normal": gil::text::normalize(text),
            "key": gil::text::key(text, MAX_KEY_LENGTH),
            "grams": grams,
        });
        golden.check(actual, text);
    }

    assert_eq!(golden.len(), 23);
    golden.finish();
}
