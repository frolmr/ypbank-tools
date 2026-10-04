//! CSV format adapter: comma-separated, with a header line.

use std::io::{Read, Write};

use super::delimited::{self, DelimitedConfig};
use crate::error::Result;
use crate::transaction::Transaction;

/// Reads and writes transactions as header + comma-separated rows.
pub struct CsvFormat;

impl CsvFormat {
    /// Parses CSV input into transactions.
    ///
    /// Errors: [`crate::Error::Io`] on read failures, [`crate::Error::Parse`]
    /// on a malformed header or row (with a 1-based line number).
    pub fn read<R: Read>(&self, reader: R) -> Result<Vec<Transaction>> {
        delimited::read(reader, &CSV_CONFIG)
    }

    /// Serializes transactions as CSV.
    ///
    /// Errors: [`crate::Error::Io`] on write failures.
    pub fn write<W: Write>(&self, writer: W, txs: &[Transaction]) -> Result<()> {
        delimited::write(writer, txs, &CSV_CONFIG)
    }
}

static CSV_CONFIG: DelimitedConfig = DelimitedConfig {
    separator: ',',
    has_header: true,
    expected_header: &["date", "category", "kind", "amount"],
    name: "csv",
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::TransactionKind;
    use chrono::NaiveDate;

    fn tx(day: u32, category: &str, kind: TransactionKind, amount: u64) -> Transaction {
        Transaction {
            date: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            category: category.into(),
            kind,
            amount,
        }
    }

    fn sample() -> Vec<Transaction> {
        vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 40_000),
        ]
    }

    #[test]
    fn reads_the_documented_example() {
        let input = "\
date,category,kind,amount
2026-04-01,salary,income,120000
2026-04-02,rent,expense,40000
";
        let txs = CsvFormat.read(input.as_bytes()).unwrap();
        assert_eq!(txs, sample());
    }

    #[test]
    fn writes_the_documented_example() {
        let mut out = Vec::new();
        CsvFormat.write(&mut out, &sample()).unwrap();

        let expected = "\
date,category,kind,amount
2026-04-01,salary,income,120000
2026-04-02,rent,expense,40000
";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
    }

    #[test]
    fn round_trips() {
        let original = sample();

        let mut buf = Vec::new();
        CsvFormat.write(&mut buf, &original).unwrap();
        let parsed = CsvFormat.read(&buf[..]).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn empty_list_writes_header_only() {
        let mut out = Vec::new();
        CsvFormat.write(&mut out, &[]).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "date,category,kind,amount\n"
        );
    }

    #[test]
    fn empty_file_is_a_parse_error() {
        match CsvFormat.read(&b""[..]).unwrap_err() {
            crate::Error::Parse { line: 1, .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn header_columns_must_match() {
        let input = "date,category,type,amount\n2026-04-01,x,income,1\n";
        match CsvFormat.read(input.as_bytes()).unwrap_err() {
            crate::Error::Parse { line: 1, msg } => assert!(msg.contains("type")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
