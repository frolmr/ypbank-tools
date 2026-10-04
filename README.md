# ypbank_tools

A small Rust library and command-line toolkit for reading, writing, converting,
and comparing financial transactions across three formats: CSV, TXT, and a
compact binary format.

The project is split into a library (`ypbank_tools`) that does all the work and
two thin CLI binaries (`converter`, `comparer`) that expose it on the command
line.

## Launch

For quick launch:

```bash
cargo run --bin <name> -- <args...>
```

## CLI: converter

Reads transactions in one format and writes them in another.

### Usage

```bash
converter <input-path> <output-path> <input-format> <output-format>
```

\<input-format> and \<output-format> are one of csv, txt, bin (case-insensitive).

### Examples

Convert CSV to TXT:

```bash
converter in.csv out.txt csv txt
```

Convert CSV to the binary format:

```bash
converter in.csv out.bin csv bin
```

### Exit codes

| Code | Meaning                                          |
| ---- | ------------------------------------------------ |
| 0    | Conversion succeeded                             |
| 1    | Reserved for `comparer`; not used by `converter` |
| 2    | Argument error, I/O failure, or parse failure    |

Diagnostics go to stderr, the converted data goes to the file given as \<output-path>.

## CLI: comparer

Compares two files of transactions by index — the _n_-th transaction of the first file is compared with the _n_-th of the second.
Order matters: the same set of transactions in a different order counts as different.

### Usage

```bash
comparer <first-input-path> <second-input-path> <first-input-format> <second-input-format>
```

Formats are one of csv, txt, bin (case-insensitive). The two files may use different formats; the comparer decodes each independently.

### Examples

Compare two CSV files:

```bash
comparer a.csv b.csv csv csv
```

Compare a CSV against a TXT file (same data, different encoding):

```bash
comparer a.csv b.txt csv txt
```

### Output

For each index where the transactions differ, one line is printed:

```bash
The 3 transaction differs: amount: 6500 != 7000
```

Multiple differing fields on the same transaction are comma-separated:

```bash
The 3 transaction differs: date: 2026-04-01 != 2026-04-02, amount: 6500 != 7000
```

If one file has more transactions than the other, a summary line follows:

```bash
First file has 2 more transaction(s) than the second
```

If the files are identical:

```bash
The files contain identical transactions
```

### Exit codes

| Code | Meaning                                          |
| ---- | ------------------------------------------------ |
| 0    | Files contain identical transactions             |
| 1    | Files differ (field mismatch or length mismatch) |
| 2    | Argument error, I/O failure, or parse failure    |

## Formats

All three formats store the same four fields per transaction:

- _date_ — calendar date
- _category_ — a free-form label such as salary or rent
- _kind_ — either income or expense
- _amount_ — a non-negative integer; direction is carried by kind

Amounts are always non-negative. "Expense of 500" is represented as `kind = expense, amount = 500`, not as `amount = -500`.

### CSV

Comma-separated, with a header line. Field order is fixed.

```csv
date,category,kind,amount
2026-04-01,salary,income,120000
2026-04-02,rent,expense,40000
```

Details:

- The header is required and must list exactly date,category,kind,amount. Comparison is case-insensitive.
- Field values may be surrounded by whitespace; it is trimmed on read.
- kind accepts income or expense, case-insensitive.
- Dates are YYYY-MM-DD.
- Both \n and \r\n line endings are accepted.
- A leading UTF-8 BOM is stripped from the header.
- Blank lines are skipped.
- Commas inside a field value are not supported (no quoting). The writer replaces a `,` inside a category with `_` to keep the output re-readable.

### TXT

Semicolon-separated, no header.

```txt
2026-04-01;salary;income;120000
2026-04-02;rent;expense;40000
```

Everything else matches CSV: whitespace is trimmed, kind is case-insensitive, dates are YYYY-MM-DD, blank lines are skipped, no escaping of `;` inside fields (the writer replaces it with `_`).

A TXT file with no content is a valid, empty list of transactions. A CSV file with no content is an error, because the required header is missing.

### Binary

A compact, self-describing record format. All multi-byte integers are little-endian.
There is no header, no magic bytes, and no record count: the reader consumes records until a clean EOF.

Layout per record:

| Field      | Size     | Encoding                                                      |
| ---------- | -------- | ------------------------------------------------------------- |
| `date`     | 4 bytes  | `i32` LE, days since CE (`NaiveDate::num_days_from_ce`)       |
| `amount`   | 8 bytes  | `u64` LE                                                      |
| `kind`     | 1 byte   | `0x00` = income, `0x01` = expense; any other byte is an error |
| `cat_len`  | 2 bytes  | `u16` LE, byte length of the category                         |
| `category` | variable | UTF-8, `cat_len` bytes, no terminator or padding              |

The fixed part of a record is 15 bytes; the total per record is `15 + cat_len`.

Reader behavior:

- Reads records until EOF.
- A clean EOF _between_ records ends the file successfully.
- EOF _inside_ a record (fewer than the expected bytes for a field) is a truncated-record error.
- Invalid kind byte, invalid UTF-8 category, and out-of-range date are all reported as errors.
- Domain invariants (non-empty category, non-zero amount) are enforced by the same constructor used by text formats.

Writer behavior:

- Emits records back-to-back with no separators or padding.
- Rejects any category longer than 65535 bytes (the length field is `u16`).

A worked example, transaction 2026-04-01, salary, income, 120000, encoded:

| offset | bytes                   | field           |
| ------ | ----------------------- | --------------- |
| 0..4   | <days from CE, LE>      | date            |
| 4..12  | C0 4F 00 00 00 00 00 00 | amount = 120000 |
| 12 00  | 00                      | kind = income   |
| 13..15 | 06 00                   | cat_len = 6     |
| 15..21 | 73 61 6C 61 72 79       | "salary"        |
