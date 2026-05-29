//! Workspace task runner for nbcache.
//!
//! Wraps the common developer and CI commands behind `cargo xtask <cmd>` so
//! that local runs and GitHub Actions invoke exactly the same thing. Inspired
//! by the `xtask` crate in <https://github.com/fogodev/ars-ui>, scaled down to
//! this crate's needs (fmt, clippy, test, coverage, plus a loom helper).

use std::{
    env,
    ffi::OsString,
    process::{Command, ExitCode},
};

use clap::{Parser, Subcommand};

/// File-path regex for sources excluded from coverage reports: the task runner
/// and the examples are tooling/demo code, not part of the measured library.
const COVERAGE_IGNORE_REGEX: &str = "(xtask|examples|tests)/";

/// nbcache workspace task runner.
#[derive(Parser)]
#[command(name = "xtask", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Format the whole workspace in place.
    Fmt {
        /// Verify formatting without rewriting files (used in CI).
        #[arg(long)]
        check: bool,
    },

    /// Run clippy across the workspace, treating warnings as errors.
    Clippy,

    /// Run the workspace test suite (all targets plus doctests).
    Test,

    /// Measure coverage with cargo-llvm-cov and write `lcov.info`.
    Coverage {
        /// Also emit an HTML report under `target/llvm-cov/html`.
        #[arg(long)]
        html: bool,

        /// Fail if line coverage is below this percentage (0-100).
        #[arg(long)]
        fail_under: Option<f64>,
    },

    /// Run the loom concurrency models (builds with `--cfg loom`, release).
    Loom,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Task::Fmt { check } => fmt(check),
        Task::Clippy => clippy(),
        Task::Test => test(),
        Task::Coverage { html, fail_under } => coverage(html, fail_under),
        Task::Loom => loom(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask: {err}");
            ExitCode::FAILURE
        }
    }
}

/// A `cargo` command, honoring the `CARGO` env var that cargo sets for xtasks.
fn cargo() -> Command {
    Command::new(env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

/// Spawn `command`, inheriting stdio, and error if it fails to start or exits
/// with a non-zero status.
fn run(mut command: Command) -> Result<(), String> {
    let rendered = format!("{command:?}");
    let status = command
        .status()
        .map_err(|e| format!("failed to spawn {rendered}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed ({status}): {rendered}"))
    }
}

fn fmt(check: bool) -> Result<(), String> {
    let mut cmd = cargo();
    cmd.args(["fmt", "--all"]);
    if check {
        cmd.args(["--", "--check"]);
    }
    run(cmd)
}

fn clippy() -> Result<(), String> {
    let mut cmd = cargo();
    cmd.args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ]);
    run(cmd)
}

fn test() -> Result<(), String> {
    // `--all-targets` covers lib/bin/test/example targets but skips doctests,
    // so doctests get their own pass.
    let mut all_targets = cargo();
    all_targets.args(["test", "--workspace", "--all-targets", "--all-features"]);
    run(all_targets)?;

    let mut doctests = cargo();
    doctests.args(["test", "--workspace", "--all-features", "--doc"]);
    run(doctests)
}

fn coverage(html: bool, fail_under: Option<f64>) -> Result<(), String> {
    // Collect instrumentation across the workspace without emitting a report,
    // so the report passes below can format the same data multiple ways.
    let mut collect = cargo();
    collect.args(["llvm-cov", "--no-report", "--workspace", "--all-features"]);
    run(collect)?;

    // lcov for Codecov.
    let mut lcov = cargo();
    lcov.args([
        "llvm-cov",
        "report",
        "--lcov",
        "--output-path",
        "lcov.info",
        "--ignore-filename-regex",
        COVERAGE_IGNORE_REGEX,
    ]);
    run(lcov)?;

    // Human-readable summary, optionally enforcing a line-coverage floor.
    let mut summary = cargo();
    summary.args([
        "llvm-cov",
        "report",
        "--summary-only",
        "--ignore-filename-regex",
        COVERAGE_IGNORE_REGEX,
    ]);
    if let Some(min) = fail_under {
        summary.arg("--fail-under-lines").arg(min.to_string());
    }
    run(summary)?;

    if html {
        let mut report = cargo();
        report.args([
            "llvm-cov",
            "report",
            "--html",
            "--ignore-filename-regex",
            COVERAGE_IGNORE_REGEX,
        ]);
        run(report)?;
    }

    Ok(())
}

fn loom() -> Result<(), String> {
    let mut cmd = cargo();
    // loom recommends release builds; `--test loom` runs only the loom models.
    cmd.args(["test", "--release", "--test", "loom"]);
    cmd.env("RUSTFLAGS", append_rustflags("--cfg loom"));

    // Bound the exploration so the models terminate quickly in CI unless the
    // caller overrides it.
    if env::var_os("LOOM_MAX_PREEMPTIONS").is_none() {
        cmd.env("LOOM_MAX_PREEMPTIONS", "3");
    }

    run(cmd)
}

/// Append `extra` to any inherited `RUSTFLAGS` rather than clobbering them.
fn append_rustflags(extra: &str) -> String {
    match env::var("RUSTFLAGS") {
        Ok(existing) if !existing.trim().is_empty() => format!("{existing} {extra}"),
        _ => extra.to_owned(),
    }
}
