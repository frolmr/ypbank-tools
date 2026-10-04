//! Binary transaction format.
//!
//! Layout (all multi-byte integers are little-endian):
//!
//! ```text
//! record :=
//!     date      4 bytes, i32 LE, days since CE (NaiveDate::num_days_from_ce)
//!     amount    8 bytes, u64 LE
//!     kind      1 byte,  0 = Income, 1 = Expense
//!     cat_len   2 bytes, u16 LE, byte length of the following UTF-8 category
//!     category  cat_len bytes, UTF-8
//!
//! file := record*   (no header, no count, reader stops at clean EOF)
//! ```
//!
//! Fixed part of a record is 15 bytes; total per record is 15 + cat_len.

use chrono::Datelike;
use std::io::{self, Read, Write};

use crate::error::{Error, Result};
use crate::transaction::{Transaction, TransactionKind};

/// Fixed prefix size in bytes: date(4) + amount(8) + kind(1) + cat_len(2).
const PREFIX_LEN: usize = 15;

const KIND_INCOME: u8 = 0;
const KIND_EXPENSE: u8 = 1;

/// Reads and writes transactions in the packed binary format
/// described in the module header.
pub struct BinFormat;

impl BinFormat {
    /// Decodes binary input into transactions.
    ///
    /// Reads records until a clean EOF. Errors:
    /// - [`crate::Error::Io`] on read failures or a truncated trailing record.
    /// - [`crate::Error::Invalid`] on an out-of-range date, unknown kind byte,
    ///   non-UTF-8 category, or a domain rule violated by the decoded values.
    pub fn read<R: Read>(&self, mut reader: R) -> Result<Vec<Transaction>> {
        let mut txs = Vec::new();
        let mut prefix = [0u8; PREFIX_LEN];

        loop {
            if !read_exact_or_eof(&mut reader, &mut prefix)? {
                break;
            }

            let days = i32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]);
            let date = chrono::NaiveDate::from_num_days_from_ce_opt(days)
                .ok_or_else(|| Error::Invalid(format!("invalid date (days from CE = {days})")))?;

            let amount = u64::from_le_bytes([
                prefix[4], prefix[5], prefix[6], prefix[7], prefix[8], prefix[9], prefix[10],
                prefix[11],
            ]);

            let kind = match prefix[12] {
                KIND_INCOME => TransactionKind::Income,
                KIND_EXPENSE => TransactionKind::Expense,
                other => {
                    return Err(Error::Invalid(format!(
                        "invalid kind byte 0x{other:02X} (expected 0x00 or 0x01)"
                    )));
                }
            };

            let cat_len = u16::from_le_bytes([prefix[13], prefix[14]]) as usize;
            let mut category_bytes = vec![0u8; cat_len];
            reader.read_exact(&mut category_bytes)?;

            let category = String::from_utf8(category_bytes)
                .map_err(|e| Error::Invalid(format!("category is not valid UTF-8: {e}")))?;

            txs.push(Transaction::new(date, category, kind, amount)?);
        }

        Ok(txs)
    }

    /// Encodes transactions into the packed binary format.
    ///
    /// Errors:
    /// - [`crate::Error::Io`] on write failures.
    /// - [`crate::Error::Invalid`] if any category is longer than 65535 bytes
    ///   (the length field is `u16`).
    pub fn write<W: Write>(&self, mut writer: W, txs: &[Transaction]) -> Result<()> {
        let mut prefix = [0u8; PREFIX_LEN];

        for tx in txs {
            let days = tx.date.num_days_from_ce();
            prefix[0..4].copy_from_slice(&days.to_le_bytes());

            prefix[4..12].copy_from_slice(&tx.amount.to_le_bytes());

            prefix[12] = match tx.kind {
                TransactionKind::Income => KIND_INCOME,
                TransactionKind::Expense => KIND_EXPENSE,
            };

            let cat_bytes = tx.category.as_bytes();
            let cat_len = u16::try_from(cat_bytes.len()).map_err(|_| {
                Error::Invalid(format!(
                    "category is too long for the binary format ({} bytes, max 65535)",
                    cat_bytes.len()
                ))
            })?;
            prefix[13..15].copy_from_slice(&cat_len.to_le_bytes());

            writer.write_all(&prefix)?;
            writer.write_all(cat_bytes)?;
        }

        writer.flush()?;
        Ok(())
    }
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => {
                if filled == 0 {
                    return Ok(false);
                } else {
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("truncated record: got {filled} of {} bytes", buf.len()),
                    )));
                }
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Error::Io(e)),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            tx(
                3,
                "coffee shop with a longer name",
                TransactionKind::Expense,
                500,
            ),
        ]
    }

    #[test]
    fn round_trips() {
        let original = sample();

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &original).unwrap();
        let parsed = BinFormat.read(&buf[..]).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn empty_list_round_trips_to_empty_vec() {
        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &[]).unwrap();
        assert!(buf.is_empty());

        let parsed = BinFormat.read(&buf[..]).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn record_size_is_15_plus_category_len() {
        let txs = vec![tx(1, "abc", TransactionKind::Income, 1)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();

        assert_eq!(buf.len(), 15 + 3);
    }

    #[test]
    fn writes_little_endian() {
        let txs = vec![tx(1, "x", TransactionKind::Income, 1)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();

        assert_eq!(buf[4], 0x01);
        assert_eq!(&buf[5..12], &[0u8; 7]);
    }

    #[test]
    fn detects_truncated_record() {
        let txs = vec![tx(1, "salary", TransactionKind::Income, 120_000)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();
        buf.pop();

        match BinFormat.read(&buf[..]).unwrap_err() {
            Error::Io(e) => assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_kind_byte() {
        let txs = vec![tx(1, "x", TransactionKind::Income, 1)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();
        buf[12] = 0xFF;

        match BinFormat.read(&buf[..]).unwrap_err() {
            Error::Invalid(msg) => assert!(msg.contains("kind")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_utf8_category() {
        let txs = vec![tx(1, "x", TransactionKind::Income, 1)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();
        buf[15] = 0xFF;

        match BinFormat.read(&buf[..]).unwrap_err() {
            Error::Invalid(msg) => assert!(msg.contains("UTF-8")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_date() {
        let txs = vec![tx(1, "x", TransactionKind::Income, 1)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();
        buf[0..4].copy_from_slice(&i32::MAX.to_le_bytes());

        match BinFormat.read(&buf[..]).unwrap_err() {
            Error::Invalid(msg) => assert!(msg.contains("date")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn accepts_multibyte_utf8_category() {
        let txs = vec![tx(1, "кофе", TransactionKind::Expense, 500)];

        let mut buf = Vec::new();
        BinFormat.write(&mut buf, &txs).unwrap();

        assert_eq!(buf.len(), 15 + 8);

        let parsed = BinFormat.read(&buf[..]).unwrap();
        assert_eq!(parsed[0].category, "кофе");
    }
}
