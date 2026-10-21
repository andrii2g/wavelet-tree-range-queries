use crate::{QueryTrace, TraceAction, WaveletTree, tree::Node};
use std::fmt::Write;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
impl WaveletTree {
    /// Export a bounded SVG. At most five edges of depth and 18 positions per node
    /// are shown. `trace` should come from a traced query on this tree.
    pub fn to_svg(&self, title: &str, answer: &str, trace: Option<&QueryTrace>) -> String {
        fn leaves(n: &Node, depth: usize) -> usize {
            if depth == 5 || n.left.is_none() {
                1
            } else {
                leaves(n.left.as_deref().unwrap(), depth + 1)
                    + leaves(n.right.as_deref().unwrap(), depth + 1)
            }
        }
        let count = self.root.as_deref().map_or(1, |n| leaves(n, 0));
        let width = count * 400;
        let height = 180 + (self.stats().max_depth.min(5) + 1) * 220;
        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="100%" height="100%" fill="#f8fafc"/><g font-family="monospace" font-size="13" fill="#0f172a"><text x="20" y="30" font-size="20">{}</text><text x="20" y="57">Answer: {}</text><text x="20" y="81">Blue: traverse | green: accept | red: reject</text><text x="20" y="103">Half-open ranges. Up to 18 positions and depth 5 shown.</text>"##,
            escape(title),
            escape(answer)
        );
        if let Some(root) = &self.root {
            draw(self, root, 0, 0, width, trace, &mut svg);
        } else {
            svg.push_str(r#"<text x="20" y="150">Empty sequence</text>"#);
        }
        svg.push_str("</g></svg>");
        svg
    }
}
fn visible_leaves(n: &Node, depth: usize) -> usize {
    if depth == 5 || n.left.is_none() {
        1
    } else {
        visible_leaves(n.left.as_deref().unwrap(), depth + 1)
            + visible_leaves(n.right.as_deref().unwrap(), depth + 1)
    }
}
fn draw(
    tree: &WaveletTree,
    n: &Node,
    depth: usize,
    offset: usize,
    width: usize,
    trace: Option<&QueryTrace>,
    svg: &mut String,
) {
    let x = offset + width / 2;
    let y = 140 + depth * 220;
    let step = trace.and_then(|t| t.steps.iter().find(|s| s.node_id == n.id));
    let stroke = match step.map(|s| s.action) {
        Some(TraceAction::Accept) => "#16a34a",
        Some(TraceAction::Reject) => "#dc2626",
        Some(_) => "#2563eb",
        None => "#94a3b8",
    };
    if depth < 5
        && let (Some(l), Some(r)) = (&n.left, &n.right)
    {
        let lw = width * visible_leaves(l, depth + 1) / visible_leaves(n, depth);
        for (child, off, w, left) in [
            (l.as_ref(), offset, lw, true),
            (r.as_ref(), offset + lw, width - lw, false),
        ] {
            let cx = off + w / 2;
            let child_step = trace.and_then(|t| t.steps.iter().find(|s| s.node_id == child.id));
            let color = if child_step.is_some() {
                "#2563eb"
            } else {
                "#cbd5e1"
            };
            write!(
                svg,
                r#"<path d="M{x} {} L{cx} {}" fill="none" stroke="{color}" stroke-width="2"/>"#,
                y + 150,
                y + 220
            )
            .unwrap();
            if let Some(s) =
                step.filter(|s| matches!(s.action, TraceAction::Left | TraceAction::Right))
            {
                let mapped = if left { &s.left_range } else { &s.right_range };
                write!(
                    svg,
                    r#"<text x="{}" y="{}" text-anchor="middle">{} [{}, {})</text>"#,
                    (x + cx) / 2,
                    y + 183,
                    if left { "L" } else { "R" },
                    mapped.start,
                    mapped.end
                )
                .unwrap();
            }
            draw(tree, child, depth + 1, off, w, trace, svg);
        }
    }
    write!(svg, r#"<rect x="{}" y="{y}" width="380" height="150" rx="10" fill="white" stroke="{stroke}" stroke-width="3"/><text x="{}" y="{}">node {} | values {}..={}</text><text x="{}" y="{}">length {}</text>"#, x-190, x-180, y+24, n.id, tree.alphabet[n.lo], tree.alphabet[n.hi-1], x-180, y+45, n.len).unwrap();
    if let Some(s) = step {
        write!(
            svg,
            r#"<text x="{}" y="{}">[{}, {}) {:?}{} +{}</text>"#,
            x - 180,
            y + 65,
            s.input_range.start,
            s.input_range.end,
            s.action,
            s.k.map_or(String::new(), |k| format!(" k={k}")),
            s.contribution
        )
        .unwrap();
    }
    for i in 0..n.len.min(18) {
        let px = x - 180 + i * 19;
        let active = step.is_some_and(|s| s.input_range.contains(&i));
        let fill = if active { "#bfdbfe" } else { "#f1f5f9" };
        let symbol = if n.left.is_none() {
            "="
        } else if n.bit(i) {
            "1"
        } else {
            "0"
        };
        write!(svg, r#"<rect x="{px}" y="{}" width="17" height="22" fill="{fill}"/><text x="{}" y="{}">{symbol}</text>"#, y+77, px+4, y+93).unwrap();
    }
    let note = if depth == 5 && n.left.is_some() {
        "Subtree collapsed at depth limit"
    } else if n.len > 18 {
        "Remaining positions omitted"
    } else if n.left.is_none() {
        "Leaf: all positions have this value"
    } else {
        "0 = left, 1 = right (stable partition)"
    };
    write!(
        svg,
        r#"<text x="{}" y="{}">{note}</text>"#,
        x - 180,
        y + 127
    )
    .unwrap();
}
