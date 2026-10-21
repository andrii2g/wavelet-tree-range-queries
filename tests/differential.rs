use wavelet_tree::WaveletTree;

fn verify(values: &[u64]) {
    let tree = WaveletTree::new(values);
    let mut probes = values.to_vec();
    probes.extend([0, 1, 2, 5, 10, u64::MAX]);
    for end in 0..=values.len() {
        for &v in &probes {
            let expected = values[..end].iter().filter(|&&x| x == v).count();
            assert_eq!(tree.rank(v, end).unwrap(), expected);
            assert_eq!(tree.trace_rank(v, end).unwrap().0, expected);
        }
    }
    for l in 0..=values.len() {
        for r in l..=values.len() {
            let mut sorted = values[l..r].to_vec();
            sorted.sort_unstable();
            for (i, &v) in sorted.iter().enumerate() {
                assert_eq!(tree.kth_smallest(l, r, i + 1).unwrap(), v);
                assert_eq!(tree.trace_kth_smallest(l, r, i + 1).unwrap().0, v);
            }
            for &v in &probes {
                let expected = values[l..r].iter().filter(|&&x| x <= v).count();
                assert_eq!(tree.count_less_or_equal(l, r, v).unwrap(), expected);
                assert_eq!(tree.trace_count_less_or_equal(l, r, v).unwrap().0, expected);
            }
            assert!(tree.kth_smallest(l, r, 0).is_err());
            assert!(tree.kth_smallest(l, r, r - l + 1).is_err());
        }
    }
    assert!(tree.rank(0, values.len() + 1).is_err());
    assert!(tree.kth_smallest(1, 0, 1).is_err());
    assert!(tree.count_less_or_equal(0, values.len() + 1, 0).is_err());
    assert!(tree.count_less_or_equal(1, 0, 0).is_err());
    let stats = tree.stats();
    assert_eq!(stats.leaves, tree.alphabet().len());
    assert_eq!(stats.internal_nodes, stats.leaves.saturating_sub(1));
}
#[test]
fn edge_cases() {
    for values in [
        vec![],
        vec![9],
        vec![2; 12],
        vec![0, u64::MAX, 0, 5, u64::MAX],
        vec![7, 2, 9, 3, 2, 8, 5, 2, 6],
        (0..16).collect(),
        (0..16).rev().collect(),
    ] {
        verify(&values);
    }
}
#[test]
fn exhaustive_small_alphabet() {
    for len in 0..=5 {
        for mut code in 0..3usize.pow(len) {
            let values: Vec<_> = (0..len)
                .map(|_| {
                    let v = (code % 3) as u64;
                    code /= 3;
                    v
                })
                .collect();
            verify(&values);
        }
    }
}
#[test]
fn seeded_distributions() {
    let mut seed = 0xdeadbeef_u64;
    for case in 0..30 {
        let values: Vec<_> = (0..20)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                if case % 2 == 0 { seed % 7 } else { seed }
            })
            .collect();
        verify(&values);
    }
}
#[test]
fn trace_mapping_and_metrics() {
    let tree = WaveletTree::new(&[7, 2, 9, 3, 2, 8, 5, 2, 6]);
    let (answer, trace) = tree.trace_kth_smallest(2, 8, 3).unwrap();
    assert_eq!(answer, 3);
    assert_eq!(trace.steps[0].left_range, 1..5);
    assert_eq!(trace.steps[0].right_range, 1..3);
    assert_eq!(trace.rank_operations, 2 * trace.max_depth);
    assert_eq!(trace.prefix_lookups, trace.rank_operations);
    assert_eq!(tree.trace_rank(2, 7).unwrap().1.rank_operations, 2);
}
