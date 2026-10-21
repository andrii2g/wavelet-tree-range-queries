use std::{error::Error, fs, path::Path, time::Instant};
use wavelet_tree::WaveletTree;
const HELP: &str = "wavelet-tree demo [--svg-dir output]\nwavelet-tree rank <value> <end> --data <comma-separated-u64s> [--svg file.svg]\nwavelet-tree kth <left> <right> <k> --data <comma-separated-u64s> [--svg file.svg]\nwavelet-tree count <left> <right> <value> --data <comma-separated-u64s> [--svg file.svg]\nPositions are zero-based, ranges half-open, k one-based. Use --data [] for empty input.";
fn save(path: &str, svg: String) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, svg)?;
    println!("SVG: {path}");
    Ok(())
}
fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}\n\n{HELP}");
        std::process::exit(2);
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        println!("{HELP}");
        return Ok(());
    }
    let command = args.remove(0);
    let mut data = None;
    let mut svg = None;
    let mut dir = None;
    let mut positional = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--data" | "--svg" | "--svg-dir" => {
                let value = iter.next().ok_or("missing option value")?;
                let slot = match arg.as_str() {
                    "--data" => &mut data,
                    "--svg" => &mut svg,
                    _ => &mut dir,
                };
                if slot.replace(value).is_some() {
                    return Err("duplicate option".into());
                }
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}").into()),
            _ => positional.push(arg),
        }
    }
    if command == "demo" {
        if data.is_some() || svg.is_some() || !positional.is_empty() {
            return Err("demo accepts only --svg-dir".into());
        }
        let values = [7, 2, 9, 3, 2, 8, 5, 2, 6];
        let tree = WaveletTree::new(&values);
        let dir = dir.unwrap_or_else(|| "output".into());
        save(
            &format!("{dir}/partitions.svg"),
            tree.to_svg("Stable value partitions", "full tree", None),
        )?;
        for (name, title, answer, trace) in [
            {
                let (a, t) = tree.trace_rank(2, 7)?;
                ("rank", "rank(2, 7)", a.to_string(), t)
            },
            {
                let (a, t) = tree.trace_kth_smallest(2, 8, 3)?;
                ("kth", "kth_smallest(2, 8, 3)", a.to_string(), t)
            },
            {
                let (a, t) = tree.trace_count_less_or_equal(2, 8, 5)?;
                ("count", "count_less_or_equal(2, 8, 5)", a.to_string(), t)
            },
        ] {
            println!("{title} = {answer}");
            save(
                &format!("{dir}/{name}.svg"),
                tree.to_svg(title, &answer, Some(&trace)),
            )?;
        }
        return Ok(());
    }
    if dir.is_some() {
        return Err("--svg-dir is only valid for demo".into());
    }
    let input = data.ok_or("--data is required")?;
    let input = input.trim();
    let input = if input.starts_with('[') && input.ends_with(']') {
        &input[1..input.len() - 1]
    } else {
        input
    };
    let values: Vec<u64> = if input.trim().is_empty() {
        vec![]
    } else {
        input
            .split(',')
            .map(|s| s.trim().parse())
            .collect::<Result<_, _>>()?
    };
    let start = Instant::now();
    let tree = WaveletTree::new(&values);
    let build = start.elapsed();
    let (answer, expected, trace) = match command.as_str() {
        "rank" if positional.len() == 2 => {
            let value = positional[0].parse()?;
            let end = positional[1].parse()?;
            let (answer, trace) = tree.trace_rank(value, end)?;
            (
                answer.to_string(),
                values[..end]
                    .iter()
                    .filter(|&&v| v == value)
                    .count()
                    .to_string(),
                trace,
            )
        }
        "kth" if positional.len() == 3 => {
            let l = positional[0].parse()?;
            let r = positional[1].parse()?;
            let k = positional[2].parse()?;
            let (answer, trace) = tree.trace_kth_smallest(l, r, k)?;
            let mut sorted = values[l..r].to_vec();
            sorted.sort_unstable();
            (answer.to_string(), sorted[k - 1].to_string(), trace)
        }
        "count" if positional.len() == 3 => {
            let l = positional[0].parse()?;
            let r = positional[1].parse()?;
            let v = positional[2].parse()?;
            let (answer, trace) = tree.trace_count_less_or_equal(l, r, v)?;
            (
                answer.to_string(),
                values[l..r].iter().filter(|&&x| x <= v).count().to_string(),
                trace,
            )
        }
        _ => return Err("unknown command or wrong number of arguments".into()),
    };
    if answer != expected {
        return Err("naive verification failed".into());
    }
    println!(
        "answer: {answer} (verified against naive implementation)\nbuild: {build:?}\nstats: {:?}\nestimated bytes: {}\nquery depth: {}, rank operations: {}, prefix lookups: {}",
        tree.stats(),
        tree.stats().estimated_bytes(),
        trace.max_depth,
        trace.rank_operations,
        trace.prefix_lookups
    );
    if let Some(path) = svg {
        save(
            &path,
            tree.to_svg(
                &format!("{command} {}", positional.join(" ")),
                &answer,
                Some(&trace),
            ),
        )?;
    }
    Ok(())
}
