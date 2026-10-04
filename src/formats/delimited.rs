//! Shared reader/writer for line-delimited text formats (CSV, TXT).
//!
//! A `DelimitedConfig` describes the differences between formats; everything
//! else — field order, date format, kind spelling, whitespace handling — is
//! identical and lives here.

use std::io::{BufRead, BufReader, Read, Write};

use crate::error::{Error, Result};
use crate::transaction::{Transaction, TransactionKind};

/// Number of fields expected in every data row.
pub const EXPECTED_COLUMNS: usize = 4;

/// Configuration describing a line-based, single-character-delimited format.
pub struct DelimitedConfig {
    /// Field separator, e.g. `,` for CSV or `;` for TXT.
    pub separator: char,
    /// Whether the first line is a header to validate and skip.
    pub has_header: bool,
    /// Expected header column names; only meaningful when `has_header == true`.
    pub expected_header: &'static [&'static str],
    /// Short format name used in error messages ("csv", "txt").
    pub name: &'static str,
}

/// Reads a delimited text stream into transactions.
///
/// Errors:
/// - [`Error::Io`] on read failures.
/// - [`Error::Parse`] with a 1-based line number for a malformed header or
///   data row (wrong column count, bad date, unknown kind, non-numeric amount),
///   or for a domain rule violated by an otherwise well-formed row.
pub fn read<R: Read>(reader: R, cfg: &DelimitedConfig) -> Result<Vec<Transaction>> {
    let mut lines = BufReader::new(reader).lines().enumerate();

    if cfg.has_header {
        match lines.next() {
            Some((_, header)) => {
                let header = header?;
                validate_header(&header, 1, cfg)?;
            }
            None => {
                return Err(Error::Parse {
                    line: 1,
                    msg: "empty input".into(),
                });
            }
        }
    }

    let mut txs = Vec::new();
    for (idx, raw_line) in lines {
        let line_no = (idx + 1) as u64;
        let line = raw_line?;

        if line.trim().is_empty() {
            continue;
        }

        txs.push(parse_line(&line, line_no, cfg)?);
    }

    Ok(txs)
}

fn validate_header(header: &str, line_no: u64, cfg: &DelimitedConfig) -> Result<()> {
    let header = header.trim_start_matches('\u{FEFF}').trim_end_matches('\r');
    let got: Vec<&str> = header.split(cfg.separator).map(str::trim).collect();

    if got.len() != cfg.expected_header.len() {
        return Err(Error::Parse {
            line: line_no,
            msg: format!(
                "{} header must have {} columns, got {}",
                cfg.name,
                cfg.expected_header.len(),
                got.len()
            ),
        });
    }

    for (i, want) in cfg.expected_header.iter().enumerate() {
        if !got[i].eq_ignore_ascii_case(want) {
            return Err(Error::Parse {
                line: line_no,
                msg: format!(
                    "{} header column {}: expected `{want}`, got `{}`",
                    cfg.name,
                    i + 1,
                    got[i]
                ),
            });
        }
    }

    Ok(())
}

fn parse_line(line: &str, line_no: u64, cfg: &DelimitedConfig) -> Result<Transaction> {
    let fields: Vec<&str> = line.split(cfg.separator).map(str::trim).collect();

    if fields.len() != EXPECTED_COLUMNS {
        return Err(Error::Parse {
            line: line_no,
            msg: format!(
                "{}: expected {} columns, got {}",
                cfg.name,
                EXPECTED_COLUMNS,
                fields.len()
            ),
        });
    }

    let date =
        chrono::NaiveDate::parse_from_str(fields[0], "%Y-%m-%d").map_err(|e| Error::Parse {
            line: line_no,
            msg: format!("invalid date `{}`: {e}", fields[0]),
        })?;

    let kind = match fields[2].to_ascii_lowercase().as_str() {
        "income" => TransactionKind::Income,
        "expense" => TransactionKind::Expense,
        other => {
            return Err(Error::Parse {
                line: line_no,
                msg: format!("unknown kind `{other}` (expected `income` or `expense`)"),
            });
        }
    };

    let amount: u64 = fields[3].parse().map_err(|e| Error::Parse {
        line: line_no,
        msg: format!("invalid amount `{}`: {e}", fields[3]),
    })?;

    Transaction::new(date, fields[1], kind, amount).map_err(|e| match e {
        Error::Invalid(msg) => Error::Parse { line: line_no, msg },
        other => other,
    })
}

/// Writes transactions as delimited text.
///
/// Errors: [`Error::Io`] on write failures.
pub fn write<W: Write>(mut writer: W, txs: &[Transaction], cfg: &DelimitedConfig) -> Result<()> {
    if cfg.has_header {
        write_header(&mut writer, cfg)?;
    }

    for tx in txs {
        write_row(&mut writer, tx, cfg)?;
    }

    writer.flush()?;
    Ok(())
}

fn write_header<W: Write>(writer: &mut W, cfg: &DelimitedConfig) -> Result<()> {
    let mut first = true;
    for name in cfg.expected_header {
        if !first {
            write!(writer, "{}", cfg.separator)?;
        }
        first = false;
        write!(writer, "{name}")?;
    }
    writeln!(writer)?;
    Ok(())
}

fn write_row<W: Write>(writer: &mut W, tx: &Transaction, cfg: &DelimitedConfig) -> Result<()> {
    let date = tx.date.format("%Y-%m-%d");
    let category = sanitize_field(&tx.category, cfg.separator);
    let kind = match tx.kind {
        TransactionKind::Income => "income",
        TransactionKind::Expense => "expense",
    };

    writeln!(
        writer,
        "{date}{sep}{category}{sep}{kind}{sep}{amount}",
        sep = cfg.separator,
        amount = tx.amount,
    )?;

    Ok(())
}

fn sanitize_field(field: &str, separator: char) -> String {
    if field.contains(separator) || field.contains('\n') || field.contains('\r') {
        field.replace(separator, "_").replace(['\n', '\r'], " ")
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::TransactionKind;
    use chrono::NaiveDate;

    const CSV: DelimitedConfig = DelimitedConfig {
        separator: ',',
        has_header: true,
        expected_header: &["date", "category", "kind", "amount"],
        name: "csv",
    };

    const TXT: DelimitedConfig = DelimitedConfig {
        separator: ';',
        has_header: false,
        expected_header: &[],
        name: "txt",
    };

    fn tx(day: u32, category: &str, kind: TransactionKind, amount: u64) -> Transaction {
        Transaction {
            date: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            category: category.into(),
            kind,
            amount,
        }
    }

    // ---- read: happy path --------------------------------------------------

    #[test]
    fn reads_two_rows() {
        let input = "\
date,category,kind,amount
2026-04-01,salary,income,120000
2026-04-02,rent,expense,40000
";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(
            txs,
            vec![
                tx(1, "salary", TransactionKind::Income, 120_000),
                tx(2, "rent", TransactionKind::Expense, 40_000),
            ]
        );
    }

    #[test]
    fn header_only_yields_empty_vec() {
        let input = "date,category,kind,amount\n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert!(txs.is_empty());
    }

    // ---- read: input normalization ----------------------------------------

    #[test]
    fn trims_whitespace_around_fields() {
        let input = "date,category,kind,amount\n2026-04-01 , salary , income , 120000 \n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs[0].category, "salary");
        assert_eq!(txs[0].amount, 120_000);
    }

    #[test]
    fn accepts_crlf_line_endings() {
        let input = "date,category,kind,amount\r\n2026-04-01,salary,income,120000\r\n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].category, "salary");
    }

    #[test]
    fn skips_blank_lines() {
        let input = "\
date,category,kind,amount

2026-04-01,salary,income,120000

2026-04-02,rent,expense,40000

";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn strips_utf8_bom_from_header() {
        let input = "\u{FEFF}date,category,kind,amount\n2026-04-01,x,income,1\n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs.len(), 1);
    }

    #[test]
    fn header_is_case_insensitive() {
        let input = "DATE,Category,Kind,Amount\n2026-04-01,x,income,1\n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs.len(), 1);
    }

    #[test]
    fn kind_is_case_insensitive() {
        let input = "date,category,kind,amount\n2026-04-01,x,INCOME,1\n";
        let txs = read(input.as_bytes(), &CSV).unwrap();
        assert_eq!(txs[0].kind, TransactionKind::Income);
    }

    // ---- read: error cases -------------------------------------------------

    #[test]
    fn empty_input_with_header_is_an_error() {
        match read(&b""[..], &CSV).unwrap_err() {
            Error::Parse { line: 1, .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn header_with_wrong_column_count_is_an_error() {
        let input = "date,category,kind\n2026-04-01,x,income,1\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 1, msg } => assert!(msg.contains("4 columns")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn header_with_wrong_name_is_an_error() {
        let input = "date,category,type,amount\n2026-04-01,x,income,1\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 1, msg } => assert!(msg.contains("type")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn row_with_wrong_column_count_is_an_error() {
        let input = "date,category,kind,amount\n2026-04-01,salary,income\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 2, msg } => assert!(msg.contains("expected 4 columns")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn invalid_date_reports_line_and_value() {
        let input = "date,category,kind,amount\n2026-13-01,x,income,1\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 2, msg } => {
                assert!(msg.contains("2026-13-01"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn unknown_kind_reports_line_and_value() {
        let input = "date,category,kind,amount\n2026-04-01,x,donation,1\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 2, msg } => assert!(msg.contains("donation")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn non_numeric_amount_reports_line_and_value() {
        let input = "date,category,kind,amount\n2026-04-01,x,income,abc\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 2, msg } => assert!(msg.contains("abc")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn zero_amount_is_reported_as_a_parse_error_with_line() {
        let input = "date,category,kind,amount\n2026-04-01,x,income,0\n";
        match read(input.as_bytes(), &CSV).unwrap_err() {
            Error::Parse { line: 2, msg } => assert!(msg.contains("amount")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // ---- write -------------------------------------------------------------

    #[test]
    fn write_includes_header_when_configured() {
        let txs = vec![tx(1, "salary", TransactionKind::Income, 120_000)];

        let mut out = Vec::new();
        write(&mut out, &txs, &CSV).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("date,category,kind,amount\n"));
    }

    #[test]
    fn write_omits_header_when_configured() {
        let txs = vec![tx(1, "salary", TransactionKind::Income, 120_000)];

        let mut out = Vec::new();
        write(&mut out, &txs, &TXT).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("date"));
        assert!(text.starts_with("2026-04-01;"));
    }

    #[test]
    fn write_uses_configured_separator() {
        let txs = vec![tx(1, "salary", TransactionKind::Income, 120_000)];

        let mut out = Vec::new();
        write(&mut out, &txs, &TXT).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "2026-04-01;salary;income;120000\n");
    }

    #[test]
    fn write_sanitizes_separator_in_category() {
        let txs = vec![tx(1, "food,drink", TransactionKind::Expense, 500)];

        let mut buf = Vec::new();
        write(&mut buf, &txs, &CSV).unwrap();

        let parsed = read(&buf[..], &CSV).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].amount, 500);
    }

    // ---- round-trip --------------------------------------------------------

    #[test]
    fn round_trips_csv() {
        let original = vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 40_000),
        ];

        let mut buf = Vec::new();
        write(&mut buf, &original, &CSV).unwrap();
        let parsed = read(&buf[..], &CSV).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn round_trips_txt() {
        let original = vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 40_000),
        ];

        let mut buf = Vec::new();
        write(&mut buf, &original, &TXT).unwrap();
        let parsed = read(&buf[..], &TXT).unwrap();

        assert_eq!(parsed, original);
    }
}
