use std::fmt::Debug;

use crate::extensions::iter::IteratorExt;
use itertools::Itertools;

#[allow(missing_docs)]
#[allow(clippy::module_name_repetitions)]
pub trait StrFilter: Debug {
    /// returns true if `input` matches the `option`
    fn filter(&self, option: &str, input: &str) -> bool;
}
#[allow(missing_docs)]
pub trait StrMetric: Debug {
    /// the relative distance between two words between 0 and 1.
    /// 0 => the words are the same
    /// 1 => maximum distance
    fn distance(&self, option: &str, input: &str) -> f64;
}
impl<F: StrFilter> StrMetric for F {
    fn distance(&self, option: &str, input: &str) -> f64 {
        // default to 0 if the filter matches and 1 if not
        !self.filter(option, input) as u8 as f64
    }
}

/// use `filter` to sort the elements of `iter` in regards to `input`
pub fn sort_with<I, M, F>(
    filter: &M,
    iter: I,
    input: &str,
    mut get_str: F,
) -> impl Iterator<Item = I::Item>
where
    I: IntoIterator,
    F: FnMut(&I::Item) -> &str,
    M: StrMetric + ?Sized,
{
    iter.into_iter()
        .map(|it| {
            let distance = filter.distance(get_str(&it), input);
            (it, distance)
        })
        .sorted_by(|(_, d1), (_, d2)| {
            d1.partial_cmp(d2).unwrap_or_else(|| {
                log::warn!("encountered uncomparable values {d1:?} and {d2:?}");
                std::cmp::Ordering::Greater
            })
        }) // sort 0->1->NaN
        .map(|(it, _)| it)
}
#[derive(Debug, Clone, Copy)]
/// filters a string by checking if the search term is a prefix
pub struct StartsWithIgnoreCase;
impl StrFilter for StartsWithIgnoreCase {
    fn filter(&self, option: &str, input: &str) -> bool {
        option.to_lowercase().starts_with(&input.to_lowercase())
    }
}

#[derive(Debug, Clone, Copy)]
/// an implementation of Levenshteins Algorithm
pub struct Levenshtein {
    ignore_case: bool,
}
impl StrMetric for Levenshtein {
    fn distance(&self, option: &str, input: &str) -> f64 {
        let lev_distance = self.dynamic_distance(option.chars(), &input.chars().collect_vec());
        let max = option.len().max(input.len());
        lev_distance as f64 / max as f64
    }
}
impl Levenshtein {
    #[allow(missing_docs)]
    pub const fn new(ignore_case: bool) -> Self {
        Self { ignore_case }
    }
    #[allow(dead_code)]
    fn recursive_distance(self, a: &[char], b: &[char]) -> usize {
        if a.is_empty() {
            b.len()
        } else if b.is_empty() {
            a.len()
        } else if crate::str::compare_char(a[0], b[0], self.ignore_case) {
            self.recursive_distance(&a[1..], &b[1..])
        } else {
            let s1 = self.recursive_distance(&a[1..], b);
            let s2 = self.recursive_distance(a, &b[1..]);
            let s3 = self.recursive_distance(&a[1..], &b[1..]);
            1 + s1.min(s2).min(s3)
        }
    }
    fn dynamic_distance(self, s: impl IntoIterator<Item = char>, t: &[char]) -> usize {
        let n = t.len();

        // initialize v0 (the previous row of distances)
        // this row is A[0][i]: edit distance from an empty s to t;
        // that distance is the number of characters to append to  s to make t.
        let mut v0 = (0..=n).collect_vec();
        // v1 may as well be uninit
        let mut v1 = vec![0; n + 1];

        for (i, s_char) in s.into_iter().lzip(1..) {
            // calculate v1 (current row distances) from the previous row v0

            // first element of v1 is A[i][0]
            // edit distance is delete (i) chars from s to match empty t
            v1[0] = i;

            // use formula to fill in the rest of the row
            for (j, &t_char) in t.iter().enumerate() {
                // calculating costs for A[i][j + 1]
                let (substitution_cost, overflowing) = v0[j].overflowing_sub(
                    crate::str::compare_char(s_char, t_char, self.ignore_case) as usize,
                );
                v1[j + 1] = if overflowing {
                    0
                } else {
                    let deletion_cost = v0[j + 1];
                    let insertion_cost = v1[j];
                    substitution_cost.min(insertion_cost).min(deletion_cost) + 1
                };
            }
            // copy v1 (current row) to v0 (previous row) for next iteration
            // since data in v1 is always invalidated, a swap without copy could be more efficient
            std::mem::swap(&mut v0, &mut v1);
        }
        // after the last swap, the results of v1 are now in v0
        v0[n]
    }
}

#[derive(Debug, Clone, Copy)]
/// applies a multiplier realative to the maximal common prefix length
pub struct SameStartBoost<O> {
    /// should the case be ignored, when calculaten the maximal common prefix
    pub ignore_case: bool,
    /// the base boost to be applied
    pub same_start_bonus: f64,
    /// the original metric
    pub other: O,
}
impl<O: StrMetric> StrMetric for SameStartBoost<O> {
    fn distance(&self, option: &str, input: &str) -> f64 {
        let distance = self.other.distance(option, input);
        let max = option.len().max(input.len());
        let prefix_len = option
            .chars()
            .zip(input.chars())
            .take_while(|(a, b)| crate::str::compare_char(*a, *b, self.ignore_case))
            .count();
        let prefix_factor = prefix_len as f64 / max as f64;
        distance * (prefix_factor.mul_add(-self.same_start_bonus, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn __test_levenshtein(a: &str, b: &str, dist: usize, algo: Levenshtein) {
        let a = a.chars().collect_vec();
        let b = b.chars().collect_vec();
        assert_eq!(dist, algo.recursive_distance(&a, &b), "failed recursive");
        assert_eq!(
            dist,
            algo.recursive_distance(&b, &a),
            "failed recursive reversed"
        );
        assert_eq!(
            dist,
            algo.dynamic_distance(a.clone(), &b),
            "failed iterative"
        );
        assert_eq!(
            dist,
            algo.dynamic_distance(b, &a),
            "failed iterative reversed"
        );
    }
    #[test]
    fn test_levenshtein_same() {
        __test_levenshtein("Levenshtein", "Levenshtein", 0, Levenshtein::new(false));
        __test_levenshtein("levENSHTein", "LEVENshtein", 0, Levenshtein::new(true));
    }
    #[test]
    fn test_levenshtein_differend() {
        __test_levenshtein("kitten", "sitting", 3, Levenshtein::new(false));
        __test_levenshtein("levENSHTein", "LEVENshtein", 6, Levenshtein::new(false));
    }
}
