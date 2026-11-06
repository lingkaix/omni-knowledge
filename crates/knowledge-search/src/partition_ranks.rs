//! Bounded file ordering over mature independently ranked partitions, not another scorer.
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    sync::Arc,
};

/// Retain at most `limit` distinct files for ONE method/keyword in ONE labelled library.
/// The caller prepays map/set/string/output peaks from its existing Core envelope. This pure
/// metadata container creates no source authority, resource promise or independent budget.
///
/// Each original contributes its BEST partition rank, not one extra vote per fragment/clip.
/// For files occurring in exactly one partition this preserves mature partition-RRF ordering.
/// Stable logical path and source ID break ties; a source's logical path must be stable across
/// this generation. Raw BM25 scores from independent partitions never enter this adapter.
/// Feed the resulting ordered list ONCE to the existing mature multi-method/keyword RRF.
///
/// Necessity: keeping every partition list is corpus-sized, while fusing every fragment list
/// directly multiplies long-file votes. Removal reintroduces either error. The worst retained
/// rank threshold only improves: an evicted file can matter later only through a better new
/// contribution, which is sufficient to re-enter. No corpus-sized seen/source map is needed.
pub struct FileRankWindow {
    limit: usize,
    files: BTreeMap<Arc<str>, (usize, Arc<str>)>,
    ordered: BTreeSet<(usize, Arc<str>, Arc<str>)>,
}
impl FileRankWindow {
    /// Create an empty data window; caller must prepay its complete bounded allocation peak.
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            files: BTreeMap::new(),
            ordered: BTreeSet::new(),
        }
    }
    /// Currently retained distinct originals, never a corpus-size estimate.
    pub fn len(&self) -> usize {
        self.files.len()
    }
    /// Whether this bounded window currently has any original contribution.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
    /// Whether this original currently occupies the bounded distinct-file window.
    pub fn contains(&self, source: &str) -> bool {
        self.files.contains_key(source)
    }
    /// This original's best one-based partition rank, never a summed fragment vote.
    pub fn rank(&self, source: &str) -> Option<usize> {
        self.files.get(source).map(|(rank, _)| *rank)
    }
    /// Borrow distinct originals in stable best-rank/path/ID order.
    pub fn ranked(&self) -> impl Iterator<Item = &str> {
        self.ordered.iter().map(|(_, _, source)| source.as_ref())
    }
    /// `rank` is the partition's already ordered DISTINCT-file rank, not a chunk rank or score.
    pub fn observe(&mut self, source: &str, path: &str, rank: NonZeroUsize) {
        let rank = rank.get();
        if self.limit == 0 {
            return;
        }
        if let Some((id, (old, logical))) = self.files.get_key_value(source) {
            if *old <= rank {
                return;
            }
            let (id, old, logical) = (id.clone(), *old, logical.clone());
            self.ordered.remove(&(old, logical.clone(), id.clone()));
            self.files.insert(id.clone(), (rank, logical.clone()));
            self.ordered.insert((rank, logical, id));
            return;
        }
        if self.files.len() == self.limit {
            let (last, last_path, last_id) = self.ordered.last().expect("nonempty bounded ranks");
            if (rank, path, source) >= (*last, last_path.as_ref(), last_id.as_ref()) {
                return;
            }
            let (_, _, id) = self.ordered.pop_last().expect("actual last rank");
            self.files.remove(id.as_ref());
        }
        let id: Arc<str> = Arc::from(source);
        let logical: Arc<str> = Arc::from(path);
        self.files.insert(id.clone(), (rank, logical.clone()));
        self.ordered.insert((rank, logical, id));
    }
    /// Consume into stable ordered original IDs; caller also funds these output copies.
    pub fn into_ranked(self) -> Vec<String> {
        drop(self.files);
        self.ordered
            .into_iter()
            .map(|(_, _, id)| id.to_string())
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rank(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }
    #[test]
    fn fragments_do_not_accumulate_file_votes_and_path_ties_are_stable() {
        let mut files = FileRankWindow::new(2);
        for _ in 0..1000 {
            files.observe("many-fragments", "z.md", rank(1));
        }
        files.observe("one-fragment", "a.md", rank(1));
        assert_eq!(files.into_ranked(), ["one-fragment", "many-fragments"]);
    }
    #[test]
    fn evicted_file_reenters_only_for_a_better_actual_contribution() {
        let mut files = FileRankWindow::new(2);
        files.observe("old", "c.md", rank(8));
        files.observe("a", "a.md", rank(1));
        files.observe("b", "b.md", rank(2));
        files.observe("old", "c.md", rank(9));
        assert_eq!(files.len(), 2);
        files.observe("old", "c.md", rank(1));
        assert_eq!(files.into_ranked(), ["a", "old"]);
    }
    #[test]
    fn arbitrarily_many_partitions_keep_only_requested_file_slots() {
        let mut files = FileRankWindow::new(5);
        for n in 0..10000 {
            let id = format!("{n:05}");
            files.observe(&id, &id, rank(n + 1));
            assert!(files.len() <= 5);
        }
        assert_eq!(
            files.into_ranked(),
            ["00000", "00001", "00002", "00003", "00004"]
        );
        let mut zero = FileRankWindow::new(0);
        zero.observe("x", "x", rank(1));
        assert!(zero.is_empty());
    }
    #[test]
    fn method_keyword_list_votes_once_through_the_same_mature_fuser() {
        let mut files = FileRankWindow::new(2);
        for _ in 0..100 {
            files.observe("a", "a", rank(1));
        }
        files.observe("b", "b", rank(1));
        let lists = vec![files.into_ranked(), vec!["b".into(), "a".into()]];
        let result = crate::fuse_ranks(&lists);
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "b");
        assert_eq!(result[0].1, result[1].1);
    }
}
