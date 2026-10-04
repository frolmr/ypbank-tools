pub mod bin;
pub mod csv;
mod delimited;
pub mod txt;

pub use bin::BinFormat;
pub use csv::CsvFormat;
pub use txt::TxtFormat;

/// Accepted file formats for transactions lists
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Txt,
    Bin,
}

impl std::str::FromStr for Format {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "csv" => Ok(Format::Csv),
            "txt" => Ok(Format::Txt),
            "bin" => Ok(Format::Bin),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod cross_format_tests {
    use super::*;
    use crate::transaction::{Transaction, TransactionKind};
    use chrono::NaiveDate;
    use std::io::Cursor;

    fn sample() -> Vec<Transaction> {
        vec![
            Transaction {
                date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                category: "salary".into(),
                kind: TransactionKind::Income,
                amount: 120_000,
            },
            Transaction {
                date: NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
                category: "rent".into(),
                kind: TransactionKind::Expense,
                amount: 40_000,
            },
        ]
    }

    #[test]
    fn csv_to_txt_via_cursor() {
        let original = sample();

        let mut csv_bytes = Vec::new();
        csv::CsvFormat
            .write(Cursor::new(&mut csv_bytes), &original)
            .unwrap();

        let from_csv = csv::CsvFormat.read(Cursor::new(&csv_bytes)).unwrap();

        let mut txt_bytes = Vec::new();
        txt::TxtFormat
            .write(Cursor::new(&mut txt_bytes), &from_csv)
            .unwrap();

        let from_txt = txt::TxtFormat.read(Cursor::new(&txt_bytes)).unwrap();

        assert_eq!(from_txt, original);
    }

    #[test]
    fn csv_to_bin_to_csv_via_cursor() {
        let original = sample();

        let mut bin = Vec::new();
        csv::CsvFormat
            .write(Cursor::new(&mut bin), &original)
            .unwrap();
        let parsed_from_csv = csv::CsvFormat.read(Cursor::new(&bin)).unwrap();

        let mut bin2 = Vec::new();
        bin::BinFormat
            .write(Cursor::new(&mut bin2), &parsed_from_csv)
            .unwrap();
        let parsed_from_bin = bin::BinFormat.read(Cursor::new(&bin2)).unwrap();

        let mut csv_again = Vec::new();
        csv::CsvFormat
            .write(Cursor::new(&mut csv_again), &parsed_from_bin)
            .unwrap();
        let parsed_again = csv::CsvFormat.read(Cursor::new(&csv_again)).unwrap();

        assert_eq!(parsed_again, original);
    }
}
