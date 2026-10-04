//! `comparer` — compares transactions from two files.
//!
//! Usage:
//!     comparer <first-input-path> <second-input-path> <first-input-format> <second-input-format>
//!
//! Exit codes:
//!     0  the files contain identical transactions in the same order
//!     1  the files differ (field mismatch or length mismatch)
//!     2  an error occurred (I/O, parse, bad arguments)

use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::process::ExitCode;

use ypbank_tools::formats::Format;
use ypbank_tools::formats::bin::BinFormat;
use ypbank_tools::formats::csv::CsvFormat;
use ypbank_tools::formats::txt::TxtFormat;
use ypbank_tools::transaction::{ListDiff, diff_lists};
use ypbank_tools::{Error, Result, Transaction, TransactionDiff};

const USAGE: &str = "\
Usage: comparer <first-input-path> <second-input-path> <first-input-format> <second-input-format>

Formats: csv | txt | bin

Example:
    comparer in.csv in.txt csv txt
";

/// Files differ. Kept distinct from `EXIT_ERROR` so scripts can tell
/// "the comparison succeeded and the answer is no" apart from
/// "the comparison couldn't be performed".
const EXIT_DIFFERENT: u8 = 1;

/// I/O, parse, or usage error.
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(true) => {
            println!("The files contain identical transactions");
            ExitCode::SUCCESS
        }
        Ok(false) => ExitCode::from(EXIT_DIFFERENT),
        Err(e) => {
            eprintln!("error: {e}");
            let mut src = std::error::Error::source(&e);
            while let Some(inner) = src {
                eprintln!("  caused by: {inner}");
                src = inner.source();
            }
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// Returns `Ok(true)` if the files are identical, `Ok(false)` if they differ,
/// and `Err(_)` if the comparison couldn't be performed.
fn run() -> Result<bool> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() != 4 {
        eprintln!("{USAGE}");
        return Err(Error::Invalid(format!(
            "expected 4 arguments, got {}",
            args.len()
        )));
    }

    let first_input_path = &args[0];
    let second_input_path = &args[1];
    let first_input_format: Format = args[2]
        .parse()
        .map_err(|_| Error::Invalid(format!("unknown input format `{}`", args[2])))?;
    let second_input_format: Format = args[3]
        .parse()
        .map_err(|_| Error::Invalid(format!("unknown input format `{}`", args[3])))?;

    let lhs = read_all(first_input_format, first_input_path)?;
    let rhs = read_all(second_input_format, second_input_path)?;

    let report = diff_lists(&lhs, &rhs);
    print_report(&report);

    Ok(report.is_empty())
}

fn print_report(report: &ListDiff) {
    for (index, diff) in &report.diff {
        println!("Transaction #{} differs: {}", index + 1, format_diff(diff));
    }

    match report.length_delta {
        d if d > 0 => println!("First file has {} more transaction(s) than the second", d),
        d if d < 0 => println!("Second file has {} more transaction(s) than the first", -d),
        _ => {}
    }
}

fn format_diff(diff: &TransactionDiff) -> String {
    let mut parts = Vec::new();

    if let Some((lhs, rhs)) = &diff.date {
        parts.push(format!("date: {lhs} != {rhs}"));
    }
    if let Some((lhs, rhs)) = &diff.category {
        parts.push(format!("category: {lhs} != {rhs}"));
    }
    if let Some((lhs, rhs)) = &diff.kind {
        parts.push(format!("kind: {lhs:?} != {rhs:?}"));
    }
    if let Some((lhs, rhs)) = &diff.amount {
        parts.push(format!("amount: {lhs} != {rhs}"));
    }

    parts.join(", ")
}

fn read_all(format: Format, path: &str) -> Result<Vec<Transaction>> {
    let file = File::open(path).map_err(|e| {
        Error::Io(io::Error::new(
            e.kind(),
            format!("cannot open `{path}` for reading: {e}"),
        ))
    })?;
    let reader = BufReader::new(file);

    let txs = match format {
        Format::Csv => CsvFormat.read(reader)?,
        Format::Txt => TxtFormat.read(reader)?,
        Format::Bin => BinFormat.read(reader)?,
    };

    Ok(txs)
}
