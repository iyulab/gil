//! The text golden (`tests/fixtures/core/text.jsonl`), written by the .NET implementation: every line's normalized
//! text, key and n-grams must come out the same here.

use serde_json::{Value, json};

const MAX_KEY_LENGTH: usize = 60;
const MIN_GRAM: usize = 2;
const MAX_GRAM: usize = 3;

#[test]
fn the_text_normalization_reproduces_its_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/core/text.jsonl"
    );
    let golden = std::fs::read_to_string(path).expect("the text golden");
    let mut lines = 0;
    for line in golden.lines().filter(|l| !l.is_empty()) {
        let expected: Value = serde_json::from_str(line).expect("a JSON line");
        let text = expected["text"].as_str().expect("text");
        let grams: Vec<Value> = gil::text::grams(text, MIN_GRAM, MAX_GRAM)
            .into_iter()
            .map(|(gram, count)| json!([gram, count]))
            .collect();
        let actual = json!({
            "text": text,
            "normal": gil::text::normalize(text),
            "key": gil::text::key(text, MAX_KEY_LENGTH),
            "grams": grams,
        });
        assert_eq!(actual, expected, "line {}", lines + 1);
        lines += 1;
    }

    assert_eq!(lines, 23);
}
