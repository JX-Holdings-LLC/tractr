//! `farm-bench`: times every scenario a fixed number of times and prints the medians.
//!
//! ```text
//! cargo run --release -p farm-bench -- [--budget] [--factor F] [--scale S] [FILTER]...
//! ```
//!
//! - `--budget`: exit with status 1 when a median is over its limit, the budget × the factor
//!   (CI). A budget the code is known to miss has a ceiling at today's time instead, so CI
//!   still catches regressions; the table reports it as over budget.
//! - `--factor F`: the budget multiplier (default 3: shared CI runners are slower and noisier
//!   than the mid-range laptop the budgets are written for).
//! - `--scale S`: multiplies the run counts (default 1).
//! - `FILTER`: run only the scenarios whose name contains one of these substrings.
//!
//! For statistics (confidence intervals, comparison with the last run) use criterion:
//! `cargo bench -p farm-bench`.
#![forbid(unsafe_code)]

use farm_bench::{scenarios, Measurement};
use std::process::ExitCode;

const USAGE: &str = "usage: farm-bench [--budget] [--factor F] [--scale S] [FILTER]...
       farm-bench lab --cart game.cart [--strategy S]... [--season | --year | --days N] [--seeds N] [--csv DIR]
         strategies: crop:<id>, best-crop, process, raw (default: every crop:<id>, best-crop, process, raw)";

/// `farm-bench lab`: the balancing lab (#230).
fn lab(args: &[String]) -> Result<(), String> {
    use farm_bench::lab::{period_days, run_lab, Strategy};
    let mut cart = None;
    let mut strategies = Vec::new();
    let mut days = None;
    let mut year = false;
    let mut seeds = 5_u32;
    let mut csv = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |flag: &str| iter.next().cloned().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "--cart" => cart = Some(value("--cart")?),
            "--strategy" => strategies.push(Strategy::parse(&value("--strategy")?)?),
            "--season" => year = false,
            "--year" => year = true,
            "--days" => days = Some(value("--days")?.parse::<u32>().map_err(|_| "--days needs a number")?),
            "--seeds" => seeds = value("--seeds")?.parse::<u32>().map_err(|_| "--seeds needs a number")?.max(1),
            "--csv" => csv = Some(std::path::PathBuf::from(value("--csv")?)),
            other => return Err(format!("unknown lab option {other}")),
        }
    }
    let path = cart.ok_or("lab needs --cart game.cart")?;
    let bytes = std::fs::read(&path).map_err(|error| format!("read {path}: {error}"))?;
    let cart = farm_cart::load_cartridge(&bytes)?;
    if strategies.is_empty() {
        let mut crops: Vec<&String> = cart
            .content
            .crops
            .keys()
            .filter(|id| {
                cart.content
                    .items
                    .iter()
                    .any(|item| item.r#type == "seed" && item.crop_type.as_deref() == Some(id.as_str()))
            })
            .collect();
        crops.sort();
        strategies.extend(crops.into_iter().map(|id| Strategy::Crop(id.clone())));
        strategies.extend([Strategy::BestCrop, Strategy::Process, Strategy::Raw]);
    }
    let days = days.unwrap_or_else(|| period_days(&cart, year));
    let seeds: Vec<String> = (1..=seeds).map(|n| format!("lab-{n}")).collect();
    let report = run_lab(&cart, &strategies, &seeds, days);
    print!("{}", report.summary());
    if let Some(dir) = csv {
        std::fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
        for (name, text) in report.csv() {
            let file = dir.join(name);
            std::fs::write(&file, text).map_err(|error| format!("write {}: {error}", file.display()))?;
        }
        println!("wrote {}", dir.display());
    }
    Ok(())
}

#[derive(Debug)]
struct Args {
    budget: bool,
    factor: f64,
    scale: f64,
    filters: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { budget: false, factor: 3.0, scale: 1.0, filters: Vec::new() };
    let mut iter = std::env::args().skip(1);
    let number = |flag: &str, value: Option<String>| -> Result<f64, String> {
        value
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| format!("{flag} needs a positive number"))
    };
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--budget" => args.budget = true,
            "--factor" => args.factor = number("--factor", iter.next())?,
            "--scale" => args.scale = number("--scale", iter.next())?,
            "-h" | "--help" => return Err(String::new()),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            filter => args.filters.push(filter.to_owned()),
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().map(String::as_str) == Some("lab") {
        return match lab(&all[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}\n{USAGE}");
                ExitCode::from(2)
            }
        };
    }
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            eprintln!("{USAGE}");
            return if message.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) };
        }
    };

    let selected: Vec<_> = scenarios(args.scale)
        .into_iter()
        .filter(|s| args.filters.is_empty() || args.filters.iter().any(|f| s.name.contains(f.as_str())))
        .collect();
    if selected.is_empty() {
        eprintln!("no scenario matches {:?}", args.filters);
        return ExitCode::from(2);
    }
    if cfg!(debug_assertions) {
        eprintln!("warning: a debug build; budgets are meant for --release");
    }

    println!(
        "{:<46} {:>5} {:>10} {:>10} {:>10} {:>10}  status",
        "scenario", "runs", "median ms", "min ms", "budget ms", "limit ms"
    );
    let mut over: Vec<Measurement> = Vec::new();
    for scenario in selected {
        let runs = scenario.runs;
        let measurement = scenario.measure(runs);
        let budget = measurement.budget_ms.map_or_else(|| "-".to_owned(), |ms| format!("{ms:.2}"));
        let limit = measurement.limit_ms(args.factor).map_or_else(|| "-".to_owned(), |ms| format!("{ms:.2}"));
        let status = if !measurement.within(args.factor) {
            "OVER LIMIT"
        } else if !measurement.meets_budget() {
            "over budget, within limit"
        } else if measurement.budget_ms.is_some() {
            "ok"
        } else {
            ""
        };
        println!(
            "{:<46} {:>5} {:>10.3} {:>10.3} {:>10} {:>10}  {status}",
            measurement.name,
            measurement.samples.len(),
            measurement.median_ms,
            measurement.min_ms(),
            budget,
            limit
        );
        if !measurement.within(args.factor) {
            over.push(measurement);
        }
    }

    if args.budget && !over.is_empty() {
        eprintln!();
        for m in &over {
            eprintln!(
                "{}: median {:.3} ms is over the limit {:.2} ms × {}",
                m.name,
                m.median_ms,
                m.enforced_ms.unwrap_or_default(),
                args.factor
            );
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
