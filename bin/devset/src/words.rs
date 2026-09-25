//! Phrasing the report and the help share: counts, lists, and near misses.

use core::fmt::Display;

/// `n` and `noun`, pluralised.
pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

/// `items`, comma-separated, with `and` before the last.
pub(crate) fn list<T: Display>(items: &[T]) -> String {
    let items: Vec<String> = items.iter().map(ToString::to_string).collect();
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.concat(),
    }
}

/// The candidate `needle` was most likely meant to be.
pub(crate) fn nearest<T: AsRef<str>>(needle: impl AsRef<str>, candidates: &[T]) -> Option<&str> {
    candidates
        .iter()
        .map(|candidate| {
            (strsim::jaro_winkler(needle.as_ref(), candidate.as_ref()), candidate.as_ref())
        })
        .filter(|&(score, _)| score > 0.8)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, candidate)| candidate)
}
