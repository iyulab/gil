//! Korean typed text is compared by keystrokes, so a syllable still being composed is on the way to the value it begins.

use gil::key_memory::begins;

#[test]
fn typed_hangul_begins_a_value_its_keystrokes_begin() {
    for (value, typed) in [
        ("박물관", "ㅂ"),
        ("박물관", "바"),
        ("박물관", "박"),
        ("박물관", "박무"),
        ("박물관", "박물"),
        ("바가지", "박"),   // the final consonant may yet start the next syllable
        ("와이파이", "오"), // ㅘ is typed ㅗ then ㅏ
        ("닭갈비", "달"),   // ㄺ is typed ㄹ then ㄱ
        ("A동 101호", "a도"),
    ] {
        assert!(
            begins(value, Some(typed)),
            "{typed:?} should begin {value:?}"
        );
    }
}

#[test]
fn typed_hangul_does_not_begin_a_value_its_keystrokes_do_not_begin() {
    for (value, typed) in [
        ("까치", "ㄱ"), // ㄲ is one keystroke on the standard keyboard
        ("박물관", "버"),
        ("박물관", "김"),
        ("와이파이", "우"),
    ] {
        assert!(
            !begins(value, Some(typed)),
            "{typed:?} should not begin {value:?}"
        );
    }
}

#[test]
fn text_without_hangul_is_compared_as_before() {
    assert!(begins("network", Some("NET")));
}
