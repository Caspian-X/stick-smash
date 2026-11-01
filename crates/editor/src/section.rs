//! JSON paths shared by section markers and problem navigation.

use std::collections::BTreeSet;

use crate::inspect::under;

pub(crate) trait Section: Copy + 'static {
    const ALL: &'static [Self];
    fn keys(self) -> &'static [&'static str];
}

pub(crate) fn of_path<S: Section>(path: &str) -> Option<S> {
    S::ALL
        .iter()
        .flat_map(|s| s.keys().iter().map(move |k| (*s, *k)))
        .filter(|(_, k)| under(path, k))
        .max_by_key(|(_, k)| k.len())
        .map(|(s, _)| s)
}

pub(crate) fn dirty(paths: &BTreeSet<String>, prefix: &str) -> bool {
    paths.iter().any(|p| under(p, prefix))
}

pub(crate) fn index_of(path: &str, key: &str) -> Option<usize> {
    path.split('.').find_map(|s| s.strip_prefix(key)?.strip_prefix('[')?.split_once(']')?.0.parse().ok())
}
