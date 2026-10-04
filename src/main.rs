//! `converter` — reads transactions in one format and writes them in another.
//!
//! Usage:
//!     converter <input-path> <output-path> <input-format> <output-format>
//!
//! Formats: csv | txt | bin

use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::process::ExitCode;

use ypbank_tools::formats::Format;
use ypbank_tools::formats::bin::BinFormat;
use ypbank_tools::formats::csv::CsvFormat;
use ypbank_tools::formats::txt::TxtFormat;
use ypbank_tools::{Error, Result, Transaction};

const USAGE: &str = "\
Usage: converter <input-path> <output-path> <input-format> <output-format>

Formats: csv | txt | bin

Example:
    converter in.csv out.txt csv txt
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            let mut src = std::error::Error::source(&e);
            while let Some(inner) = src {
                eprintln!("  caused by: {inner}");
                src = inner.source();
            }
            ExitCode::FAILURE
        }
    }
}

/// Returns `Ok(())` if the conversion succeeded.
/// Errors are either usage errors (wrong argument count, unknown format name)
/// or propagate from reading the input or writing the output.
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() != 4 {
        eprintln!("{USAGE}");
        return Err(Error::Invalid(format!(
            "expected 4 arguments, got {}",
            args.len()
        )));
    }

    let input_path = &args[0];
    let output_path = &args[1];
    let input_format: Format = args[2]
        .parse()
        .map_err(|_| Error::Invalid(format!("unknown input format `{}`", args[2])))?;
    let output_format: Format = args[3]
        .parse()
        .map_err(|_| Error::Invalid(format!("unknown output format `{}`", args[3])))?;

    let txs = read_all(input_format, input_path)?;
    write_all(output_format, output_path, &txs)?;

    eprintln!("converted {} transaction(s)", txs.len());
    Ok(())
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

fn write_all(format: Format, path: &str, txs: &[Transaction]) -> Result<()> {
    let file = File::create(path).map_err(|e| {
        Error::Io(io::Error::new(
            e.kind(),
            format!("cannot open `{path}` for writing: {e}"),
        ))
    })?;
    let writer = BufWriter::new(file);

    match format {
        Format::Csv => CsvFormat.write(writer, txs)?,
        Format::Txt => TxtFormat.write(writer, txs)?,
        Format::Bin => BinFormat.write(writer, txs)?,
    }

    Ok(())
}
