# Text normalization and ordering

This page specifies, scalar by scalar, how the decision core turns text into the keys and n-grams it compares, and
the orders its sums and ties follow. The reference implementation is the Rust crate (`crates/gil`, `text.rs`);
another implementation that follows this page reaches the same keys, the same n-grams and the same sums to the bit.

## Normalized text

Every comparison of text by its characters — the keys of values settled together, the n-grams of similar records —
starts from the same normalized text:

1. **NFKC** (Unicode Normalization Form KC). By the Unicode normalization stability policy, the result for a character
   assigned in a given Unicode version never changes in a later one; implementations built on different Unicode
   versions agree on every character assigned in both. The reference takes NFKC from the `unicode-normalization`
   crate.
2. **Lower case, one scalar value at a time**, by the simple lowercase mapping (`UnicodeData.txt`, field 13) — no
   context-dependent rule and no mapping to several characters. U+0130 becomes U+0069, and U+03A3 is always U+03C3,
   never the final sigma. The reference takes Rust's `char::to_lowercase`, which follows the full mapping and differs
   from the simple one only at U+0130; that one is taken to U+0069, as the simple mapping says.
3. **Whitespace**: every run of scalar values with the Unicode `White_Space` property is one U+0020 SPACE, and there
   is none at either end. That property holds 25 code points: U+0009–U+000D, U+0020, U+0085, U+00A0, U+1680,
   U+2000–U+200A, U+2028, U+2029, U+202F, U+205F, U+3000. The reference is Rust's `char::is_whitespace`.

A key is the normalized text of one attribute value, when it is non-empty and no longer than `maxKeyLength` scalar
values.

## N-grams

The similar-record memory adds one space at either end of the normalized text, then counts every run of `minGram` to `maxGram`
consecutive **Unicode scalar values** — a character outside the Basic Multilingual Plane is one scalar, not two
UTF-16 code units. Text with fewer scalars than `minGram` is one n-gram, itself.

## Orders

- **Strings are ordered by Unicode scalar value**, which is the order their UTF-8 or UTF-32 encodings sort in. It
  differs from ordinal UTF-16 order only where a character beyond U+FFFF meets one from U+E000 to U+FFFF. Ties between
  values, records settled at the same moment, and the values of a set all follow it.
- **The n-grams of a text are listed in that order.** An n-gram seen for the first time takes the next id in that
  order, texts taking their ids in the order they are remembered. The ids decide the order of every sum over n-grams:
  a row's norm, a lookup's dot products, the norm of a lookup's n-grams no row holds (summed in the lookup's own order).
- **Votes are tallied in the order of the voters**, nearest first, and a tie between tallies keeps the earlier.
- **A hash that orders the values of a set** is 64-bit FNV-1a over the UTF-8 bytes of the record id, U+001F, and
  the value.

## Typed text and prefixes

Text typed into an attribute and the prefixes cut from values are counted in **Unicode scalar values**, as n-grams are:

- **How much is typed** — the length that picks an entry of `Attribute.TypedKeyThresholds` — is the number of
  scalars typed. A character beyond U+FFFF is one.
- **A prefix of a value** — the first *n* characters, as a coarse level's `Prefix` names a class and as replaying
  typed text cuts the settled value — is its first *n* scalars, or all of it when shorter. A prefix never ends inside
  a character.
- **A value begins with typed text, ignoring case,** when the typed text's scalars match the value's first scalars one
  for one, each compared by the simple lowercase mapping of [Normalized text](#normalized-text) step 2 (U+0130 to
  U+0069 included). Nothing else of normalization applies: no NFKC, and spaces count as typed.

## Hangul

A prefix typed partway through a Hangul syllable is matched by its keystrokes: each precomposed syllable (U+AC00 to
U+D7A3) splits into its initial, vowel and final letters, a compound vowel or final into the two keys that type it on
the standard keyboard, and a compatibility letter typed alone (ㄳ, ㅘ, …) splits the same way.
