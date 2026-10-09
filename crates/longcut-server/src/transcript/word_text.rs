pub(crate) fn canonical_word_separator(next: &str) -> &'static str {
    if next.chars().next().is_some_and(|character| {
        matches!(
            character,
            '.' | ',' | '!' | '?' | ':' | ';' | ')' | ']' | '}'
        )
    }) {
        ""
    } else {
        " "
    }
}

pub(crate) fn checked_canonical_word_len(
    current: usize,
    has_previous: bool,
    next: &str,
) -> Option<usize> {
    let separator = if has_previous {
        canonical_word_separator(next).len()
    } else {
        0
    };
    current
        .checked_add(separator)
        .and_then(|bytes| bytes.checked_add(next.len()))
}
