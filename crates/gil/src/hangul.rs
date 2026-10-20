//! Korean typed a letter at a time: a syllable as the keystrokes that compose it, so text still being composed can be
//! matched against the values it is on the way to.

const FIRST: u32 = 0xAC00;
const LAST: u32 = 0xD7A3;

const INITIALS: [&str; 19] = [
    "ㄱ", "ㄲ", "ㄴ", "ㄷ", "ㄸ", "ㄹ", "ㅁ", "ㅂ", "ㅃ", "ㅅ", "ㅆ", "ㅇ", "ㅈ", "ㅉ", "ㅊ", "ㅋ",
    "ㅌ", "ㅍ", "ㅎ",
];

const VOWELS: [&str; 21] = [
    "ㅏ", "ㅐ", "ㅑ", "ㅒ", "ㅓ", "ㅔ", "ㅕ", "ㅖ", "ㅗ", "ㅗㅏ", "ㅗㅐ", "ㅗㅣ", "ㅛ", "ㅜ",
    "ㅜㅓ", "ㅜㅔ", "ㅜㅣ", "ㅠ", "ㅡ", "ㅡㅣ", "ㅣ",
];

const FINALS: [&str; 28] = [
    "", "ㄱ", "ㄲ", "ㄱㅅ", "ㄴ", "ㄴㅈ", "ㄴㅎ", "ㄷ", "ㄹ", "ㄹㄱ", "ㄹㅁ", "ㄹㅂ", "ㄹㅅ",
    "ㄹㅌ", "ㄹㅍ", "ㄹㅎ", "ㅁ", "ㅂ", "ㅂㅅ", "ㅅ", "ㅆ", "ㅇ", "ㅈ", "ㅊ", "ㅋ", "ㅌ", "ㅍ",
    "ㅎ",
];

/// A compound letter as the keystrokes that make it.
fn compound(c: char) -> Option<&'static str> {
    Some(match c {
        'ㄳ' => "ㄱㅅ",
        'ㄵ' => "ㄴㅈ",
        'ㄶ' => "ㄴㅎ",
        'ㄺ' => "ㄹㄱ",
        'ㄻ' => "ㄹㅁ",
        'ㄼ' => "ㄹㅂ",
        'ㄽ' => "ㄹㅅ",
        'ㄾ' => "ㄹㅌ",
        'ㄿ' => "ㄹㅍ",
        'ㅀ' => "ㄹㅎ",
        'ㅄ' => "ㅂㅅ",
        'ㅘ' => "ㅗㅏ",
        'ㅙ' => "ㅗㅐ",
        'ㅚ' => "ㅗㅣ",
        'ㅝ' => "ㅜㅓ",
        'ㅞ' => "ㅜㅔ",
        'ㅟ' => "ㅜㅣ",
        'ㅢ' => "ㅡㅣ",
        _ => return None,
    })
}

/// Whether `text` holds a Hangul syllable or letter.
pub fn has(text: &str) -> bool {
    text.chars()
        .any(|c| (FIRST..=LAST).contains(&(c as u32)) || ('ㄱ'..='ㅣ').contains(&c))
}

/// `text` with each Hangul syllable and compound letter spelled out as the keystrokes that make it.
pub fn keystrokes(text: &str) -> String {
    let mut keys = String::with_capacity(text.len() * 3);
    for c in text.chars() {
        let code = c as u32;
        if (FIRST..=LAST).contains(&code) {
            let index = (code - FIRST) as usize;
            keys.push_str(INITIALS[index / 588]);
            keys.push_str(VOWELS[index % 588 / 28]);
            keys.push_str(FINALS[index % 28]);
        } else if let Some(split) = compound(c) {
            keys.push_str(split);
        } else {
            keys.push(c);
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_syllable_is_its_keystrokes() {
        assert_eq!(keystrokes("박물관"), "ㅂㅏㄱㅁㅜㄹㄱㅗㅏㄴ");
        assert_eq!(keystrokes("ㄳ a"), "ㄱㅅ a");
        assert!(has("바") && has("ㅂ") && !has("bar"));
    }
}
