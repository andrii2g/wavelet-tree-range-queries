use std::time::{Duration, Instant};
use std::{fmt, mem::size_of, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct BuildTimings {
    pub compression: Duration,
    pub construction: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    InvalidRange {
        left: usize,
        right: usize,
        len: usize,
    },
    InvalidK {
        k: usize,
        len: usize,
    },
}
impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { left, right, len } => {
                write!(f, "invalid range [{left}, {right}) for length {len}")
            }
            Self::InvalidK { k, len } => write!(f, "k must be in 1..={len}, got {k}"),
        }
    }
}
impl std::error::Error for QueryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceAction {
    Left,
    Right,
    Accept,
    Reject,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    pub node_id: usize,
    pub input_range: Range<usize>,
    pub left_range: Range<usize>,
    pub right_range: Range<usize>,
    pub action: TraceAction,
    /// One-based order statistic on entry, for kth queries.
    pub k: Option<usize>,
    /// Number of qualifying values accepted at this step.
    pub contribution: usize,
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QueryTrace {
    pub steps: Vec<TraceStep>,
    pub max_depth: usize,
    pub rank_operations: usize,
    pub prefix_lookups: usize,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeStats {
    pub internal_nodes: usize,
    pub leaves: usize,
    pub max_depth: usize,
    pub structural_bytes: usize,
    pub bitvector_bytes: usize,
    pub prefix_bytes: usize,
    pub alphabet_bytes: usize,
}
impl TreeStats {
    /// Inline tree storage plus owned allocation capacities; excludes allocator overhead.
    pub fn estimated_bytes(&self) -> usize {
        self.structural_bytes + self.bitvector_bytes + self.prefix_bytes + self.alphabet_bytes
    }
}

#[derive(Debug)]
pub(crate) struct Node {
    pub id: usize,
    pub lo: usize,
    pub hi: usize,
    pub len: usize,
    pub bits: Vec<u64>,
    pub prefix: Vec<usize>,
    pub left: Option<Box<Node>>,
    pub right: Option<Box<Node>>,
}
impl Node {
    fn build(codes: &[usize], lo: usize, hi: usize, next: &mut usize) -> Box<Self> {
        let id = *next;
        *next += 1;
        let mut node = Box::new(Self {
            id,
            lo,
            hi,
            len: codes.len(),
            bits: vec![],
            prefix: vec![],
            left: None,
            right: None,
        });
        if hi - lo == 1 {
            return node;
        }
        let mid = lo + (hi - lo) / 2;
        node.bits = vec![0; codes.len().div_ceil(64)];
        node.prefix = Vec::with_capacity(codes.len() + 1);
        node.prefix.push(0);
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for (i, &code) in codes.iter().enumerate() {
            let one = code >= mid;
            if one {
                node.bits[i / 64] |= 1 << (i % 64);
                right.push(code);
            } else {
                left.push(code);
            }
            node.prefix.push(node.prefix[i] + usize::from(one));
        }
        node.left = Some(Self::build(&left, lo, mid, next));
        node.right = Some(Self::build(&right, mid, hi, next));
        node
    }
    pub(crate) fn bit(&self, i: usize) -> bool {
        self.bits[i / 64] & (1 << (i % 64)) != 0
    }
    fn map(&self, range: &Range<usize>) -> (Range<usize>, Range<usize>) {
        let a = self.prefix[range.start];
        let b = self.prefix[range.end];
        (range.start - a..range.end - b, a..b)
    }
}

#[derive(Debug)]
pub struct WaveletTree {
    pub(crate) alphabet: Vec<u64>,
    pub(crate) root: Option<Box<Node>>,
    len: usize,
}
impl WaveletTree {
    pub fn new(values: &[u64]) -> Self {
        Self::new_with_timings(values).0
    }
    /// Compression includes alphabet sorting and mapping values to codes.
    /// Construction excludes destruction of the temporary codes buffer.
    pub fn new_with_timings(values: &[u64]) -> (Self, BuildTimings) {
        let start = Instant::now();
        let mut alphabet = values.to_vec();
        alphabet.sort_unstable();
        alphabet.dedup();
        alphabet.shrink_to_fit();
        let codes: Vec<_> = values
            .iter()
            .map(|v| alphabet.binary_search(v).unwrap())
            .collect();
        let compression = start.elapsed();
        let start = Instant::now();
        let root = if alphabet.is_empty() {
            None
        } else {
            Some(Node::build(&codes, 0, alphabet.len(), &mut 0))
        };
        let construction = start.elapsed();
        (
            Self {
                alphabet,
                root,
                len: values.len(),
            },
            BuildTimings {
                compression,
                construction,
            },
        )
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn alphabet(&self) -> &[u64] {
        &self.alphabet
    }
    fn validate(&self, l: usize, r: usize) -> Result<(), QueryError> {
        if l > r || r > self.len {
            Err(QueryError::InvalidRange {
                left: l,
                right: r,
                len: self.len,
            })
        } else {
            Ok(())
        }
    }
    pub fn rank(&self, value: u64, end: usize) -> Result<usize, QueryError> {
        self.rank_impl::<false>(value, end).map(|x| x.0)
    }
    pub fn trace_rank(&self, value: u64, end: usize) -> Result<(usize, QueryTrace), QueryError> {
        self.rank_impl::<true>(value, end)
    }
    fn rank_impl<const TRACE: bool>(
        &self,
        value: u64,
        end: usize,
    ) -> Result<(usize, QueryTrace), QueryError> {
        self.validate(0, end)?;
        let mut trace = QueryTrace::default();
        let Ok(code) = self.alphabet.binary_search(&value) else {
            return Ok((0, trace));
        };
        let mut node = self.root.as_deref().unwrap();
        let mut end = end;
        let mut depth = 0;
        while node.hi - node.lo > 1 {
            let ones = node.prefix[end];
            let left = 0..end - ones;
            let right = 0..ones;
            let go_left = code < node.lo + (node.hi - node.lo) / 2;
            if TRACE {
                trace.rank_operations += 1;
                trace.prefix_lookups += 1;
                record(
                    &mut trace,
                    node,
                    0..end,
                    (left.clone(), right.clone()),
                    if go_left {
                        TraceAction::Left
                    } else {
                        TraceAction::Right
                    },
                    None,
                    0,
                    depth,
                );
            }
            end = if go_left { left.end } else { right.end };
            node = if go_left {
                node.left.as_deref().unwrap()
            } else {
                node.right.as_deref().unwrap()
            };
            depth += 1;
        }
        if TRACE {
            record(
                &mut trace,
                node,
                0..end,
                (0..0, 0..0),
                TraceAction::Accept,
                None,
                end,
                depth,
            );
        }
        Ok((end, trace))
    }
    pub fn kth_smallest(&self, left: usize, right: usize, k: usize) -> Result<u64, QueryError> {
        self.kth_impl::<false>(left, right, k).map(|x| x.0)
    }
    pub fn trace_kth_smallest(
        &self,
        left: usize,
        right: usize,
        k: usize,
    ) -> Result<(u64, QueryTrace), QueryError> {
        self.kth_impl::<true>(left, right, k)
    }
    fn kth_impl<const TRACE: bool>(
        &self,
        left: usize,
        right: usize,
        mut k: usize,
    ) -> Result<(u64, QueryTrace), QueryError> {
        self.validate(left, right)?;
        if k == 0 || k > right - left {
            return Err(QueryError::InvalidK {
                k,
                len: right - left,
            });
        }
        let mut trace = QueryTrace::default();
        let mut node = self.root.as_deref().unwrap();
        let mut range = left..right;
        let mut depth = 0;
        while node.hi - node.lo > 1 {
            let (l, r) = node.map(&range);
            let go_left = k <= l.len();
            if TRACE {
                trace.rank_operations += 2;
                trace.prefix_lookups += 2;
                record(
                    &mut trace,
                    node,
                    range.clone(),
                    (l.clone(), r.clone()),
                    if go_left {
                        TraceAction::Left
                    } else {
                        TraceAction::Right
                    },
                    Some(k),
                    0,
                    depth,
                );
            }
            if go_left {
                range = l;
                node = node.left.as_deref().unwrap();
            } else {
                k -= l.len();
                range = r;
                node = node.right.as_deref().unwrap();
            }
            depth += 1;
        }
        if TRACE {
            record(
                &mut trace,
                node,
                range,
                (0..0, 0..0),
                TraceAction::Accept,
                Some(k),
                0,
                depth,
            );
        }
        Ok((self.alphabet[node.lo], trace))
    }
    pub fn count_less_or_equal(
        &self,
        left: usize,
        right: usize,
        value: u64,
    ) -> Result<usize, QueryError> {
        self.count_impl::<false>(left, right, value).map(|x| x.0)
    }
    pub fn trace_count_less_or_equal(
        &self,
        left: usize,
        right: usize,
        value: u64,
    ) -> Result<(usize, QueryTrace), QueryError> {
        self.count_impl::<true>(left, right, value)
    }
    fn count_impl<const TRACE: bool>(
        &self,
        left: usize,
        right: usize,
        value: u64,
    ) -> Result<(usize, QueryTrace), QueryError> {
        self.validate(left, right)?;
        let mut trace = QueryTrace::default();
        let limit = self.alphabet.partition_point(|&v| v <= value);
        let result = self
            .root
            .as_deref()
            .map_or(0, |n| count::<TRACE>(n, left..right, limit, 0, &mut trace));
        Ok((result, trace))
    }
    pub fn stats(&self) -> TreeStats {
        fn visit(n: &Node, depth: usize, stats: &mut TreeStats) {
            stats.structural_bytes += size_of::<Node>();
            stats.bitvector_bytes += n.bits.capacity() * size_of::<u64>();
            stats.prefix_bytes += n.prefix.capacity() * size_of::<usize>();
            stats.max_depth = stats.max_depth.max(depth);
            if n.left.is_none() {
                stats.leaves += 1;
            } else {
                stats.internal_nodes += 1;
            }
            if let Some(l) = &n.left {
                visit(l, depth + 1, stats);
            }
            if let Some(r) = &n.right {
                visit(r, depth + 1, stats);
            }
        }
        let mut stats = TreeStats {
            structural_bytes: size_of::<Self>(),
            alphabet_bytes: self.alphabet.capacity() * size_of::<u64>(),
            ..TreeStats::default()
        };
        if let Some(root) = &self.root {
            visit(root, 0, &mut stats);
        }
        stats
    }
}
#[allow(clippy::too_many_arguments)]
fn record(
    trace: &mut QueryTrace,
    node: &Node,
    range: Range<usize>,
    mapped: (Range<usize>, Range<usize>),
    action: TraceAction,
    k: Option<usize>,
    contribution: usize,
    depth: usize,
) {
    trace.max_depth = trace.max_depth.max(depth);
    trace.steps.push(TraceStep {
        node_id: node.id,
        input_range: range,
        left_range: mapped.0,
        right_range: mapped.1,
        action,
        k,
        contribution,
    });
}
fn count<const TRACE: bool>(
    node: &Node,
    range: Range<usize>,
    limit: usize,
    depth: usize,
    trace: &mut QueryTrace,
) -> usize {
    if range.is_empty() || limit <= node.lo || limit >= node.hi {
        let accepted = !range.is_empty() && limit >= node.hi;
        let result = if accepted { range.len() } else { 0 };
        if TRACE {
            record(
                trace,
                node,
                range,
                (0..0, 0..0),
                if accepted {
                    TraceAction::Accept
                } else {
                    TraceAction::Reject
                },
                None,
                result,
                depth,
            );
        }
        return result;
    }
    let (l, r) = node.map(&range);
    if TRACE {
        trace.rank_operations += 2;
        trace.prefix_lookups += 2;
        record(
            trace,
            node,
            range,
            (l.clone(), r.clone()),
            if limit <= node.lo + (node.hi - node.lo) / 2 {
                TraceAction::Left
            } else {
                TraceAction::Right
            },
            None,
            0,
            depth,
        );
    }
    count::<TRACE>(node.left.as_deref().unwrap(), l, limit, depth + 1, trace)
        + count::<TRACE>(node.right.as_deref().unwrap(), r, limit, depth + 1, trace)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partition_bits_and_prefixes_across_word_boundaries() {
        fn check(node: &Node, codes: &[usize]) {
            assert_eq!(node.len, codes.len());
            if node.hi - node.lo == 1 {
                assert!(codes.iter().all(|&code| code == node.lo));
                return;
            }
            let mid = node.lo + (node.hi - node.lo) / 2;
            let mut ones = 0;
            let (mut l, mut r) = (vec![], vec![]);
            for (i, &code) in codes.iter().enumerate() {
                assert_eq!(node.prefix[i], ones);
                assert_eq!(node.bit(i), code >= mid);
                if code >= mid {
                    ones += 1;
                    r.push(code);
                } else {
                    l.push(code);
                }
            }
            assert_eq!(node.prefix[codes.len()], ones);
            check(node.left.as_deref().unwrap(), &l);
            check(node.right.as_deref().unwrap(), &r);
        }
        let values: Vec<_> = (0..193).map(|i| (i * 37 % 19) as u64).collect();
        let tree = WaveletTree::new(&values);
        let codes: Vec<_> = values
            .iter()
            .map(|v| tree.alphabet.binary_search(v).unwrap())
            .collect();
        check(tree.root.as_deref().unwrap(), &codes);
        for end in [0, 63, 64, 65, 127, 128, 129, 193] {
            for v in 0..20 {
                assert_eq!(
                    tree.rank(v, end).unwrap(),
                    values[..end].iter().filter(|&&x| x == v).count()
                );
            }
        }
    }
}
