//! Core domain types for the transaction library.
//!
//! Everything in this module is pure: no I/O, no format-specific logic.
//! Format readers/writers construct `Transaction` values via `Transaction::new`
//! (or directly, when the format can guarantee the invariants hold).

use crate::error::{Error, Result};
use chrono::NaiveDate;

/// Calendar date on which the transaction occurred.
pub type TransactionDate = NaiveDate;

/// Free-form category label (e.g. "salary", "rent").
pub type TransactionCategory = String;

/// Transaction amount, always non-negative. Income/expense is expressed
/// via [`TransactionKind`], not via the sign of the amount.
pub type TransactionAmount = u64;

/// Whether a transaction adds to or subtracts from the balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Income,
    Expense,
}

/// A single financial transaction.
///
/// Use [`Transaction::new`] to construct one with validation; direct struct
/// literal construction bypasses the invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub date: TransactionDate,
    pub category: TransactionCategory,
    pub kind: TransactionKind,
    pub amount: TransactionAmount,
}

impl Transaction {
    /// Builds a validated transaction.
    ///
    /// Returns [`Error::Invalid`] if `category` is empty or whitespace-only,
    /// or if `amount` is zero.
    pub fn new(
        date: TransactionDate,
        category: impl Into<TransactionCategory>,
        kind: TransactionKind,
        amount: TransactionAmount,
    ) -> Result<Self> {
        let category = category.into();
        if category.trim().is_empty() {
            return Err(Error::Invalid("category must not be empty".into()));
        }
        if amount == 0 {
            return Err(Error::Invalid("amount must be greater than zero".into()));
        }
        Ok(Self {
            date,
            category,
            kind,
            amount,
        })
    }

    /// Compares `self` and `other` field by field.
    ///
    /// Returns `None` if they are equal, otherwise `Some(diff)` listing only
    /// the fields that differ, each as `(self_value, other_value)`.
    pub fn diff(&self, other: &Self) -> Option<TransactionDiff> {
        let diff = TransactionDiff {
            date: if self.date != other.date {
                Some((self.date, other.date))
            } else {
                None
            },
            category: if self.category != other.category {
                Some((self.category.clone(), other.category.clone()))
            } else {
                None
            },
            kind: if self.kind != other.kind {
                Some((self.kind.clone(), other.kind.clone()))
            } else {
                None
            },
            amount: if self.amount != other.amount {
                Some((self.amount, other.amount))
            } else {
                None
            },
        };

        (!diff.is_empty()).then_some(diff)
    }
}

/// Field-level differences between two transactions.
///
/// Each field is `Some((left, right))` when the values differ and `None` when
/// they match. Produced by [`Transaction::diff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionDiff {
    pub date: Option<(TransactionDate, TransactionDate)>,
    pub category: Option<(TransactionCategory, TransactionCategory)>,
    pub kind: Option<(TransactionKind, TransactionKind)>,
    pub amount: Option<(TransactionAmount, TransactionAmount)>,
}

impl TransactionDiff {
    /// Returns `true` if no field differs.
    pub fn is_empty(&self) -> bool {
        self.date.is_none()
            && self.category.is_none()
            && self.kind.is_none()
            && self.amount.is_none()
    }
}

/// Differences between two ordered lists of transactions.
///
/// `diff` holds one entry per index where both lists have an element and the
/// elements differ. `length_delta` is `lhs.len() - rhs.len()` as a signed
/// value: positive means `lhs` is longer, negative means `rhs` is longer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDiff {
    pub diff: Vec<(usize, TransactionDiff)>,
    pub length_delta: isize,
}

impl ListDiff {
    /// Returns `true` if the lists were element-wise equal and the same length.
    pub fn is_empty(&self) -> bool {
        self.diff.is_empty() && self.length_delta == 0
    }
}

/// Compares two ordered lists of transactions index-by-index.
///
/// Only the shared prefix is compared pairwise; the remainder of the longer
/// list is summarized by `ListDiff::length_delta`. Order matters: two lists
/// with the same elements in a different order will report differences.
pub fn diff_lists(lhs: &[Transaction], rhs: &[Transaction]) -> ListDiff {
    let common = lhs.len().min(rhs.len());

    let diff = lhs[..common]
        .iter()
        .zip(&rhs[..common])
        .enumerate()
        .filter_map(|(i, (a, b))| a.diff(b).map(|d| (i, d)))
        .collect();

    let length_delta = lhs.len() as isize - rhs.len() as isize;

    ListDiff { diff, length_delta }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers -----------------------------------------------------------

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn tx(day: u32, category: &str, kind: TransactionKind, amount: u64) -> Transaction {
        Transaction {
            date: date(2026, 4, day),
            category: category.into(),
            kind,
            amount,
        }
    }

    // ---- Transaction::new --------------------------------------------------

    #[test]
    fn new_accepts_valid_input() {
        let t =
            Transaction::new(date(2026, 4, 1), "salary", TransactionKind::Income, 120_000).unwrap();

        assert_eq!(t.date, date(2026, 4, 1));
        assert_eq!(t.category, "salary");
        assert_eq!(t.kind, TransactionKind::Income);
        assert_eq!(t.amount, 120_000);
    }

    #[test]
    fn new_accepts_owned_and_borrowed_category() {
        let borrowed =
            Transaction::new(date(2026, 4, 1), "rent", TransactionKind::Expense, 40_000).unwrap();
        let owned = Transaction::new(
            date(2026, 4, 1),
            String::from("rent"),
            TransactionKind::Expense,
            40_000,
        )
        .unwrap();

        assert_eq!(borrowed, owned);
    }

    #[test]
    fn new_rejects_zero_amount() {
        let err =
            Transaction::new(date(2026, 4, 1), "salary", TransactionKind::Income, 0).unwrap_err();

        match err {
            Error::Invalid(msg) => assert!(msg.contains("amount")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn new_rejects_empty_category() {
        let err = Transaction::new(date(2026, 4, 1), "", TransactionKind::Income, 1).unwrap_err();

        match err {
            Error::Invalid(msg) => assert!(msg.contains("category")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn new_rejects_whitespace_only_category() {
        let err =
            Transaction::new(date(2026, 4, 1), "   \t\n", TransactionKind::Income, 1).unwrap_err();

        assert!(matches!(err, Error::Invalid(_)));
    }

    // ---- Transaction::diff -------------------------------------------------

    #[test]
    fn diff_returns_none_for_equal_transactions() {
        let a = tx(1, "salary", TransactionKind::Income, 120_000);
        let b = a.clone();
        assert!(a.diff(&b).is_none());
    }

    #[test]
    fn diff_is_symmetric_for_equal_values() {
        let a = tx(1, "salary", TransactionKind::Income, 120_000);
        let b = tx(2, "rent", TransactionKind::Expense, 40_000);

        let ab = a.diff(&b).unwrap();
        let ba = b.diff(&a).unwrap();

        assert_eq!(ab.date, Some((a.date, b.date)));
        assert_eq!(ba.date, Some((b.date, a.date)));

        assert_eq!(ab.amount, Some((a.amount, b.amount)));
        assert_eq!(ba.amount, Some((b.amount, a.amount)));
    }

    #[test]
    fn diff_reports_only_changed_fields() {
        let a = tx(1, "salary", TransactionKind::Income, 120_000);
        let b = Transaction {
            amount: 125_000,
            ..a.clone()
        };

        let diff = a.diff(&b).unwrap();

        assert!(diff.date.is_none());
        assert!(diff.category.is_none());
        assert!(diff.kind.is_none());
        assert_eq!(diff.amount, Some((120_000, 125_000)));
    }

    #[test]
    fn diff_reports_all_fields_when_all_differ() {
        let a = tx(1, "salary", TransactionKind::Income, 120_000);
        let b = tx(2, "rent", TransactionKind::Expense, 40_000);

        let diff = a.diff(&b).unwrap();

        assert!(diff.date.is_some());
        assert!(diff.category.is_some());
        assert!(diff.kind.is_some());
        assert!(diff.amount.is_some());
    }

    #[test]
    fn diff_preserves_self_other_order() {
        let a = tx(1, "salary", TransactionKind::Income, 120_000);
        let b = tx(1, "salary", TransactionKind::Income, 125_000);

        let diff = a.diff(&b).unwrap();
        assert_eq!(diff.amount, Some((120_000, 125_000)));

        let diff_rev = b.diff(&a).unwrap();
        assert_eq!(diff_rev.amount, Some((125_000, 120_000)));
    }

    #[test]
    fn diff_distinguishes_kind_variants() {
        let a = tx(1, "x", TransactionKind::Income, 1);
        let b = tx(1, "x", TransactionKind::Expense, 1);

        let diff = a.diff(&b).unwrap();
        assert_eq!(
            diff.kind,
            Some((TransactionKind::Income, TransactionKind::Expense))
        );
        assert!(diff.amount.is_none());
    }

    // ---- TransactionDiff::is_empty ----------------------------------------

    #[test]
    fn diff_is_empty_when_no_fields_differ() {
        let d = TransactionDiff {
            date: None,
            category: None,
            kind: None,
            amount: None,
        };
        assert!(d.is_empty());
    }

    #[test]
    fn diff_is_not_empty_when_any_field_differs() {
        let d = TransactionDiff {
            date: None,
            category: None,
            kind: None,
            amount: Some((1, 2)),
        };
        assert!(!d.is_empty());
    }

    // ---- diff_lists --------------------------------------------------------

    #[test]
    fn diff_lists_empty_when_equal() {
        let a = vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 40_000),
        ];
        let b = a.clone();

        let report = diff_lists(&a, &b);
        assert!(report.is_empty());
        assert_eq!(report.length_delta, 0);
        assert!(report.diff.is_empty());
    }

    #[test]
    fn diff_lists_reports_changed_index() {
        let a = vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 40_000),
        ];
        let b = vec![
            tx(1, "salary", TransactionKind::Income, 120_000),
            tx(2, "rent", TransactionKind::Expense, 41_000),
        ];

        let report = diff_lists(&a, &b);
        assert!(!report.is_empty());
        assert_eq!(report.diff.len(), 1);

        let (index, field_diff) = &report.diff[0];
        assert_eq!(*index, 1);
        assert_eq!(field_diff.amount, Some((40_000, 41_000)));
    }

    #[test]
    fn diff_lists_handles_both_empty() {
        let report = diff_lists(&[], &[]);
        assert!(report.is_empty());
        assert_eq!(report.length_delta, 0);
    }

    #[test]
    fn diff_lists_reports_lhs_longer() {
        let a = vec![
            tx(1, "a", TransactionKind::Income, 1),
            tx(2, "b", TransactionKind::Income, 2),
            tx(3, "c", TransactionKind::Income, 3),
        ];
        let b = vec![tx(1, "a", TransactionKind::Income, 1)];

        let report = diff_lists(&a, &b);
        assert_eq!(report.length_delta, 2);
        assert!(report.diff.is_empty());
        assert!(!report.is_empty());
    }

    #[test]
    fn diff_lists_reports_rhs_longer() {
        let a = vec![tx(1, "a", TransactionKind::Income, 1)];
        let b = vec![
            tx(1, "a", TransactionKind::Income, 1),
            tx(2, "b", TransactionKind::Income, 2),
        ];

        let report = diff_lists(&a, &b);
        assert_eq!(report.length_delta, -1);
        assert!(report.diff.is_empty());
    }

    #[test]
    fn diff_lists_order_matters() {
        let a = vec![
            tx(1, "a", TransactionKind::Income, 1),
            tx(2, "b", TransactionKind::Income, 2),
        ];
        let b = vec![
            tx(2, "b", TransactionKind::Income, 2),
            tx(1, "a", TransactionKind::Income, 1),
        ];

        let report = diff_lists(&a, &b);
        assert_eq!(report.diff.len(), 2);
        assert_eq!(report.length_delta, 0);
    }
}
