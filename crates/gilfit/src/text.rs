//! Text as the decision core compares it: normalized text, keys, n-grams, and the order strings are compared in —
//! `docs/text-normalization.md`, scalar by scalar.

use std::collections::BTreeMap;

use unicode_normalization::UnicodeNormalization;

/// NFKC, then each Unicode scalar value lower-cased by its simple mapping, every run of `White_Space` one space, and no
/// space at either end.
pub fn normalize(text: &str) -> String {
    let mut normal = String::with_capacity(text.len());
    let mut space = true;
    for c in text.nfkc() {
        if c.is_whitespace() {
            space = true;
            continue;
        }

        if space && !normal.is_empty() {
            normal.push(' ');
        }

        normal.push(lower(c));
        space = false;
    }

    normal
}

/// The simple lowercase mapping of one scalar (`UnicodeData.txt`, field 13). Rust's lowercase follows the full mapping,
/// which differs from the simple one only for U+0130 — taken to U+0069 here, as the simple mapping says.
fn lower(c: char) -> char {
    if c == '\u{130}' {
        return 'i';
    }

    let mut lowered = c.to_lowercase();
    match (lowered.next(), lowered.next()) {
        (Some(single), None) => single,
        _ => c, // no other scalar lowers to several; keep it as it is if one ever does
    }
}

/// How many Unicode scalar values `text` holds.
pub fn scalars(text: &str) -> usize {
    text.chars().count()
}

/// The key a field value makes: its normalized text, when non-empty and at most `max_key_length` scalars long.
pub fn key(text: &str, max_key_length: usize) -> Option<String> {
    let normal = normalize(text);
    (!normal.is_empty() && scalars(&normal) <= max_key_length).then_some(normal)
}

/// The n-grams of `text` with their counts, in scalar order: runs of `min_gram` to `max_gram` scalars of the normalized
/// text with a space at either end; text with fewer scalars than `min_gram` is one n-gram, and empty text has none.
pub fn grams(text: &str, min_gram: usize, max_gram: usize) -> Vec<(String, u32)> {
    let normal = normalize(text);
    if normal.is_empty() {
        return Vec::new();
    }

    let padded: Vec<char> = std::iter::once(' ')
        .chain(normal.chars())
        .chain(std::iter::once(' '))
        .collect();
    if padded.len() < min_gram {
        return vec![(padded.into_iter().collect(), 1)];
    }

    // A BTreeMap over String orders by UTF-8 bytes, which is Unicode scalar order.
    let mut counts = BTreeMap::<String, u32>::new();
    for n in min_gram..=max_gram.min(padded.len()) {
        for window in padded.windows(n) {
            *counts.entry(window.iter().collect()).or_insert(0) += 1;
        }
    }

    counts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_capital_i_lowers_to_plain_i() {
        assert_eq!(normalize("\u{130}stanbul"), "istanbul");
    }

    #[test]
    fn capital_sigma_is_never_final() {
        assert_eq!(normalize("\u{3A3}\u{391}\u{3A3}"), "\u{3C3}\u{3B1}\u{3C3}");
    }

    #[test]
    fn scalar_order_is_not_utf16_order() {
        // U+E000 sorts before U+1F600 by scalar value; in UTF-16 the surrogate D83D sorts first.
        assert!("\u{E000}" < "\u{1F600}");
    }
}
