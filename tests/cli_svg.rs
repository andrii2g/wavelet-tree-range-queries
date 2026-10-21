use std::process::Command;
use wavelet_tree::WaveletTree;
#[test]
fn cli_queries_and_errors() {
    for (args, expected) in [
        (
            vec!["rank", "2", "7", "--data", "7,2,9,3,2,8,5,2,6"],
            "answer: 2",
        ),
        (
            vec!["kth", "2", "8", "3", "--data", "[7,2,9,3,2,8,5,2,6]"],
            "answer: 3",
        ),
        (vec!["count", "0", "0", "0", "--data", "[]"], "answer: 0"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wavelet-tree"))
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains(expected));
    }
    for args in [
        vec!["kth", "0", "1", "0", "--data", "5"],
        vec!["rank", "2", "99", "--data", "2"],
        vec!["rank", "2", "0", "--data", "bad"],
        vec!["demo", "--unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wavelet-tree"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
#[test]
fn svg_handles_empty_large_and_escaped_labels() {
    let empty = WaveletTree::new(&[]).to_svg("<tag>&\"", "0", None);
    assert!(empty.contains("&lt;tag&gt;&amp;&quot;"));
    assert!(empty.contains("Empty sequence"));
    let tree = WaveletTree::new(&(0..200).collect::<Vec<_>>());
    let (answer, trace) = tree.trace_kth_smallest(10, 100, 4).unwrap();
    let svg = tree.to_svg("kth", &answer.to_string(), Some(&trace));
    assert!(svg.contains("Subtree collapsed"));
    assert!(svg.contains("L ["));
    assert!(svg.contains("#bfdbfe"));
    assert!(svg.ends_with("</g></svg>"));
}
