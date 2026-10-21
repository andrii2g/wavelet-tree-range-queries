# Wavelet Tree Range Queries

A dependency-free Rust library and CLI for statistical queries over a fixed `u64`
sequence. Includes recursive construction, packed partition bits, prefix counts,
query traces, SVG diagrams, differential tests, and a reproducible benchmark.
Requires Rust 1.88 or newer (edition 2024).

## Run

```sh
cargo run --release -- demo
cargo run --release -- rank 2 7 --data "7,2,9,3,2,8,5,2,6" --svg output/rank.svg
cargo run --release -- kth 2 8 3 --data "7,2,9,3,2,8,5,2,6" --svg output/kth.svg
cargo run --release -- count 2 8 5 --data "7,2,9,3,2,8,5,2,6" --svg output/count.svg
```

The answers are **2**, **3**, and **4**. Positions are zero-based, ranges are
half-open `[left, right)`, and `k` is one-based. `rank(value, end)` counts occurrences
in `[0, end)`. Every explicit CLI query is verified by scanning or sorting.
`--data` accepts comma-separated integers, optionally enclosed in brackets; use
`--data "[]"` for an empty sequence. This is a numeric list parser, not general JSON.
Invalid input exits with status 2. Run `--help` for usage.

The demo creates `output/partitions.svg`, `rank.svg`, `kth.svg`, and `count.svg`.
Open these in a browser. Use `demo --svg-dir some-folder` to change the destination.
Output paths are overwritten when reused. Generated output is ignored by Git.

## Library

```rust
use wavelet_tree::WaveletTree;

let tree = WaveletTree::new(&[7, 2, 9, 3, 2, 8, 5, 2, 6]);
assert_eq!(tree.rank(2, 7)?, 2);
assert_eq!(tree.kth_smallest(2, 8, 3)?, 3);
assert_eq!(tree.count_less_or_equal(2, 8, 5)?, 4);
let (answer, trace) = tree.trace_kth_smallest(2, 8, 3)?;
let svg = tree.to_svg("Third smallest in [2, 8)", &answer.to_string(), Some(&trace));
# Ok::<(), wavelet_tree::QueryError>(())
```

Queries return `QueryError` for reversed or out-of-bounds ranges. Counting an empty
range returns zero. An absent rank value returns zero. `k = 0` and `k` larger than
the range length are errors. Empty trees and the full `u64` domain are supported.
The tree owns its index and does not borrow or retain the original sequence.

## How it works

Construction sorts and deduplicates an alphabet, maps input values to integer
codes, then recursively halves the alphabet interval. Each internal node stably
partitions its subsequence. A packed bit is zero for the left partition and one
for the right. `prefix[i]` stores the number of ones before position `i`.

For a node-local range `[l, r)`, let `a = prefix[l]` and `b = prefix[r]`:

```text
left range  = [l - a, r - b)
right range = [a, b)
```

Rank follows the value's branch while translating one endpoint. Kth-smallest uses
the left range length to choose a branch and adjust `k`. Threshold counting
accepts or rejects complete value intervals and descends only along the threshold
boundary; sibling intervals terminate immediately. Queries do not scan bitvectors.

For `n` elements and `sigma` distinct values, alphabet preparation takes
`O(n log n)` and code mapping plus construction takes `O(n log sigma)` for
`sigma >= 2`. Queries take `O(1 + log sigma)`; empty and single-value trees have
constant query cost. Each distinct value has one leaf, giving `2*sigma - 1` nodes
for nonempty trees. Tree height is at most `ceil(log2(sigma))`.

This MVP deliberately stores full `usize` prefix arrays: memory is
`O(n log sigma)` machine words plus nodes and alphabet, **not succinct space**.
Partition bits are packed into `u64` words and retained for inspection. Replacing
full prefix counts with sampled rank metadata is a natural future experiment.

## Traces and diagrams

`trace_rank`, `trace_kth_smallest`, and `trace_count_less_or_equal` return the answer
and a `QueryTrace`. Normal queries use a separate compile-time instantiation with
tracing disabled and allocate no trace steps. Traces contain node IDs, input and
mapped ranges, branch decisions, incoming `k`, and accepted count contributions.
Leaf/terminal steps have empty mapped ranges because no child lookup occurs.
Absent-value rank queries have an empty trace.

SVGs show node value bounds, partition bits, highlighted local positions, mapped
edge ranges, and accepted/rejected nodes. A leaf uses `=` markers because it has
no partition bits. Pass a trace from the same tree to `to_svg`. Rendering caps
visible depth at five edges and each node at 18 positions, explicitly labelling
omissions. Traces themselves are complete. Diagrams are horizontally scrollable
for wide alphabets. SVG titles and answers are XML-escaped.

## Verification and metrics

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release
cargo bench --bench queries
```

Tests compare all valid ranges, order statistics, and sampled thresholds against
independent scanning/sorting implementations. Coverage includes exhaustive
sequences over a three-value alphabet through length five; seeded random and
duplicate-heavy data; sorted/reverse-sorted input; empty and all-equal sequences;
absent values; `0` and `u64::MAX`; invalid bounds; CLI failures; and SVG escaping.

The benchmark prints CSV for five distributions and two range widths (16 and
10,000). It checks every measured query against its naive oracle before timing,
warms up each query implementation, and uses `black_box`. Query timing excludes
construction, oracle validation, tracing, and SVG generation. Kth's naive baseline
includes the subarray copy and sort. Rank uses a prefix of the reported width.
Results are single-process samples, not statistical confidence intervals; repeat
runs before drawing conclusions. Small ranges may favor scanning.

Metrics have explicit meanings:

| Metric | Definition |
| --- | --- |
| Internal nodes / leaves | Actual allocated tree nodes |
| Estimated bytes | Inline tree plus boxed nodes and vector capacities |
| Compression time | Alphabet sorting/deduplication and code mapping |
| Construction time | Recursive construction, excluding final temporary-code destruction |
| Total build time | Complete constructor call |
| Query depth | Maximum edge depth reached, root at zero |
| Rank operations | Calls to the prefix-rank primitive |
| Prefix lookups | Actual prefix-array accesses; currently one per rank operation |
| Query runtime | Mean elapsed nanoseconds over 500 untraced queries |

Memory reporting excludes allocator metadata, temporary build buffers, the caller's
input, traces, and SVG strings. It is not peak process memory. Benchmark depth and
operation counters describe the first query in each batch, not the batch mean.
Build timings are single builds. `WaveletTree::new_with_timings` exposes the two
build phases; `stats()` exposes memory components and tree shape.

## Layout

- `src/tree.rs`: construction, queries, traces, and metrics.
- `src/svg.rs`: bounded SVG rendering.
- `src/main.rs`: CLI, naive verification, and demo.
- `tests/`: differential and CLI/SVG integration tests.
- `benches/queries.rs`: deterministic correctness-checked benchmark.

No GitHub Actions or workflows are included.
