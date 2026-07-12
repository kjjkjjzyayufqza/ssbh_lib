//! Full-corpus AnimData decode scan for `.nuanmb` trees.
//! Read-only on the game dump; prints summary to stdout only (no temp output files).
//!
//! Usage:
//!   cargo run -p ssbh_test --release --bin scan_nuanmb -- "E:\XB\解包\vs2\x64\003motion"

use clap::Parser;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use ssbh_data::prelude::*;

#[derive(Parser)]
#[command(about = "Scan all .nuanmb under a folder via AnimData::from_file (read-only)")]
struct Cli {
    /// Root folder to scan recursively
    root_folder: PathBuf,

    /// Print first N failure paths (default 30)
    #[arg(long, default_value_t = 30)]
    max_failures: usize,

    /// Progress print every N files (0 = off)
    #[arg(long, default_value_t = 2000)]
    progress_every: u64,

    /// If set, print every failure path as FAIL\t<path>\t<error> (stdout)
    #[arg(long, default_value_t = false)]
    list_all_failures: bool,
}

#[derive(Default)]
struct Counters {
    total: AtomicU64,
    ok: AtomicU64,
    fail: AtomicU64,
    not_v12: AtomicU64,
}

fn main() {
    let cli = Cli::parse();
    let root = cli.root_folder.as_path();
    if !root.is_dir() {
        eprintln!("not a directory: {}", root.display());
        std::process::exit(2);
    }

    eprintln!("Scanning .nuanmb under {} ...", root.display());
    let start = Instant::now();

    let paths: Vec<PathBuf> = globwalk::GlobWalkerBuilder::from_patterns(root, &["*.nuanmb"])
        .build()
        .expect("globwalk")
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .collect();

    let total_found = paths.len();
    eprintln!("Found {total_found} .nuanmb files");

    let counters = Counters::default();
    let error_hist: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
    let failures: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let all_fails: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let progress_every = cli.progress_every;
    let list_all = cli.list_all_failures;

    paths.par_iter().for_each(|path| {
        let n = counters.total.fetch_add(1, Ordering::Relaxed) + 1;
        if progress_every > 0 && n % progress_every == 0 {
            eprintln!(
                "  progress {n}/{total_found}  ok={} fail={}",
                counters.ok.load(Ordering::Relaxed),
                counters.fail.load(Ordering::Relaxed)
            );
        }

        // Isolate panics so one bad buffer cannot abort the whole corpus scan.
        let result = catch_unwind(AssertUnwindSafe(|| AnimData::from_file(path)));
        match result {
            Ok(Ok(data)) => {
                counters.ok.fetch_add(1, Ordering::Relaxed);
                if data.major_version != 1 || data.minor_version != 2 {
                    counters.not_v12.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(Err(e)) => {
                record_fail(
                    &counters,
                    &error_hist,
                    &failures,
                    &all_fails,
                    list_all,
                    cli.max_failures,
                    path,
                    e.to_string(),
                );
            }
            Err(panic_payload) => {
                let msg = panic_message(&panic_payload);
                record_fail(
                    &counters,
                    &error_hist,
                    &failures,
                    &all_fails,
                    list_all,
                    cli.max_failures,
                    path,
                    format!("PANIC: {msg}"),
                );
            }
        }
    });

    let elapsed = start.elapsed();
    let ok = counters.ok.load(Ordering::Relaxed);
    let fail = counters.fail.load(Ordering::Relaxed);
    let total = counters.total.load(Ordering::Relaxed);
    let not_v12 = counters.not_v12.load(Ordering::Relaxed);

    println!("=== nuanmb AnimData full corpus scan ===");
    println!("root: {}", root.display());
    println!("files_found: {total_found}");
    println!("files_scanned: {total}");
    println!("ok: {ok}");
    println!("fail: {fail}");
    println!("ok_non_v12: {not_v12}");
    println!(
        "ok_rate: {:.4}%",
        if total == 0 {
            0.0
        } else {
            100.0 * ok as f64 / total as f64
        }
    );
    println!("elapsed_sec: {:.3}", elapsed.as_secs_f64());
    println!(
        "files_per_sec: {:.1}",
        if elapsed.as_secs_f64() > 0.0 {
            total as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        }
    );

    println!("=== error histogram (bucket => count) ===");
    let hist = error_hist.lock().unwrap();
    if hist.is_empty() {
        println!("(none)");
    } else {
        for (k, v) in hist.iter() {
            println!("{v}\t{k}");
        }
    }

    println!("=== first failures (up to {}) ===", cli.max_failures);
    let fails = failures.lock().unwrap();
    if fails.is_empty() {
        println!("(none)");
    } else {
        for (p, e) in fails.iter() {
            println!("{p}");
            println!("  {e}");
        }
    }

    if list_all {
        println!("=== ALL_FAILURES_BEGIN ===");
        let all = all_fails.lock().unwrap();
        for (p, e) in all.iter() {
            println!("FAIL\t{p}\t{e}");
        }
        println!("=== ALL_FAILURES_END ===");
    }

    if fail > 0 {
        std::process::exit(1);
    }
}

fn record_fail(
    counters: &Counters,
    error_hist: &Mutex<BTreeMap<String, u64>>,
    failures: &Mutex<Vec<(String, String)>>,
    all_fails: &Mutex<Vec<(String, String)>>,
    list_all: bool,
    max_failures: usize,
    path: &Path,
    err: String,
) {
    counters.fail.fetch_add(1, Ordering::Relaxed);
    let msg = classify_error(&err);
    {
        let mut hist = error_hist.lock().unwrap();
        *hist.entry(msg).or_insert(0) += 1;
    }
    let mut fails = failures.lock().unwrap();
    if fails.len() < max_failures {
        fails.push((path.display().to_string(), err.clone()));
    }
    if list_all {
        all_fails
            .lock()
            .unwrap()
            .push((path.display().to_string(), err));
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "unknown panic".into()
}

fn classify_error(msg: &str) -> String {
    // Bucket similar errors so the histogram is readable for ~46k files.
    // Order matters: avoid false positives (e.g. "animat**io**n" matching "io").
    let lower = msg.to_lowercase();
    if lower.contains("panic") {
        return "Panic".into();
    }
    if lower.contains("failed residual/layout decode") {
        // "header 0x3409" style
        if let Some(idx) = lower.find("header 0x") {
            let rest = &msg[idx + "header ".len()..];
            let hex: String = rest
                .chars()
                .take_while(|c| c.is_ascii_hexdigit() || *c == 'x' || *c == 'X')
                .collect();
            return format!("DecodeFailed({hex})");
        }
        return "DecodeFailed".into();
    }
    if lower.contains("malformed")
        || lower.contains("incomplete")
        || lower.contains("invaliddata")
        || lower.contains("invalid data")
        || lower.contains("invalid_data")
    {
        return "InvalidData".into();
    }
    if lower.contains("unsupported") && lower.contains("header") {
        // keep hex if present
        if let Some(idx) = msg.find("0x") {
            let hex: String = msg[idx..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit() || *c == 'x')
                .collect();
            return format!("UnsupportedV12PropertyHeader({hex})");
        }
        return "UnsupportedV12PropertyHeader".into();
    }
    if lower.contains("unsupported version") {
        return "UnsupportedVersion".into();
    }
    if lower.contains("unexpected") {
        return "UnexpectedV12PropertyValue".into();
    }
    if lower.contains("os error") || lower.contains("permission denied") || lower.contains("i/o")
    {
        return "IoError".into();
    }
    // Truncate long messages
    let t = msg.replace('\n', " ");
    if t.len() > 120 {
        format!("{}…", &t[..120])
    } else {
        t
    }
}
