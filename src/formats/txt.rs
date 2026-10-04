//! TXT format adapter: semicolon-separated, no header.

use super::delimited::{self, DelimitedConfig};
use crate::error::Result;
use crate::transaction::Transaction;
use std::io::{Read, Write};

/// Reads and writes transactions as semicolon-separated rows without a header.
pub struct TxtFormat;

impl TxtFormat {
    /// Parses TXT input into transactions.
    ///
    /// Errors: [`crate::Error::Io`] on read failures, [`crate::Error::Parse`]
    /// on a malformed row (with a 1-based line number).
    pub fn read<R: Read>(&self, reader: R) -> Result<Vec<Transaction>> {
        delimited::read(reader, &TXT_CONFIG)
    }

    /// Serializes transactions as TXT.
    ///
    /// Errors: [`crate::Error::Io`] on write failures.
    pub fn write<W: Write>(&self, writer: W, txs: &[Transaction]) -> Result<()> {
        delimited::write(writer, txs, &TXT_CONFIG)
    }
}

static TXT_CONFIG: DelimitedConfig = DelimitedConfig {
    separator: ';',
    has_header: false,
    expected_header: &[],
    name: "txt",
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
2026-04-01;salary;income;120000
2026-04-02;rent;expense;40000
";
        let txs = TxtFormat.read(input.as_bytes()).unwrap();
        assert_eq!(txs, sample());
    }

    #[test]
    fn writes_the_documented_example() {
        let mut out = Vec::new();
        TxtFormat.write(&mut out, &sample()).unwrap();

        let expected = "\
2026-04-01;salary;income;120000
2026-04-02;rent;expense;40000
";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
    }

    #[test]
    fn round_trips() {
        let original = sample();

        let mut buf = Vec::new();
        TxtFormat.write(&mut buf, &original).unwrap();
        let parsed = TxtFormat.read(&buf[..]).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn has_no_header() {
        let input = "2026-04-01;salary;income;120000\n";
        let txs = TxtFormat.read(input.as_bytes()).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].category, "salary");
    }

    #[test]
    fn empty_file_yields_empty_vec() {
        let txs = TxtFormat.read(&b""[..]).unwrap();
        assert!(txs.is_empty());
    }

    #[test]
    fn empty_list_writes_nothing() {
        let mut out = Vec::new();
        TxtFormat.write(&mut out, &[]).unwrap();
        assert!(out.is_empty());
    }
}
