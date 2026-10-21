use std::{hint::black_box, time::Instant};
use wavelet_tree::WaveletTree;
fn random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}
fn timed<T>(iterations: usize, mut f: impl FnMut(usize) -> T) -> f64 {
    for i in 0..iterations.min(100) {
        black_box(f(i));
    }
    let start = Instant::now();
    for i in 0..iterations {
        black_box(f(i));
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}
fn main() {
    let n = 10_000;
    let iterations = 500;
    println!(
        "dataset,n,sigma,compression_us,construction_us,total_build_us,nodes,estimated_bytes,query,width,tree_ns,naive_ns,depth,rank_ops,prefix_lookups"
    );
    for dataset in ["uniform", "duplicates", "sorted", "reverse", "sparse"] {
        let mut seed = 0xdeadbeef;
        let values: Vec<u64> = (0..n)
            .map(|i| match dataset {
                "uniform" => random(&mut seed) % 1024,
                "duplicates" => random(&mut seed) % 8,
                "sorted" => i as u64,
                "reverse" => (n - i) as u64,
                _ => random(&mut seed),
            })
            .collect();
        let build_start = Instant::now();
        let (tree, build) = WaveletTree::new_with_timings(&values);
        let total = build_start.elapsed();
        let stats = tree.stats();
        for width in [16, n] {
            let queries: Vec<_> = (0..iterations)
                .map(|_| {
                    let l = random(&mut seed) as usize % (n - width + 1);
                    let v = values[random(&mut seed) as usize % n];
                    let k = random(&mut seed) as usize % width + 1;
                    (l, l + width, v, k)
                })
                .collect();
            for kind in ["rank", "kth", "count"] {
                // Rank uses a prefix of the selected width; other queries use [l,r).
                let indexed = |i: usize| {
                    let (l, r, v, k) = queries[i];
                    match kind {
                        "rank" => tree.rank(v, width).unwrap() as u64,
                        "kth" => tree.kth_smallest(l, r, k).unwrap(),
                        _ => tree.count_less_or_equal(l, r, v).unwrap() as u64,
                    }
                };
                let naive = |i: usize| {
                    let (l, r, v, k) = queries[i];
                    match kind {
                        "rank" => values[..width].iter().filter(|&&x| x == v).count() as u64,
                        "kth" => {
                            let mut copy = values[l..r].to_vec();
                            copy.sort_unstable();
                            copy[k - 1]
                        }
                        _ => values[l..r].iter().filter(|&&x| x <= v).count() as u64,
                    }
                };
                for i in 0..iterations {
                    assert_eq!(indexed(i), naive(i));
                }
                let tree_ns = timed(iterations, indexed);
                let naive_ns = timed(iterations, naive);
                let (l, r, v, k) = queries[0];
                let trace = match kind {
                    "rank" => tree.trace_rank(v, width).unwrap().1,
                    "kth" => tree.trace_kth_smallest(l, r, k).unwrap().1,
                    _ => tree.trace_count_less_or_equal(l, r, v).unwrap().1,
                };
                println!(
                    "{dataset},{n},{},{},{},{},{},{},{kind},{width},{tree_ns:.2},{naive_ns:.2},{},{},{}",
                    tree.alphabet().len(),
                    build.compression.as_micros(),
                    build.construction.as_micros(),
                    total.as_micros(),
                    stats.internal_nodes + stats.leaves,
                    stats.estimated_bytes(),
                    trace.max_depth,
                    trace.rank_operations,
                    trace.prefix_lookups
                );
            }
        }
    }
}
