//! A small in-memory SQL subset.
//!
//! # Supported (and nothing else)
//!
//! ```text
//! SELECT * | col [, col ...] | COUNT(*)  FROM table
//!        [WHERE cond] [ORDER BY col [ASC|DESC] [, ...]] [LIMIT n]
//! INSERT INTO table [(col, ...)] VALUES (v, ...) [, (v, ...) ...]
//! UPDATE table SET col = v [, col = v ...] [WHERE cond]
//! DELETE FROM table [WHERE cond]
//! CREATE TABLE table (col [INTEGER|INT|REAL|DECIMAL|NUMERIC|TEXT], ...)
//! DROP TABLE table
//! SHOW TABLES            (not SQL: lists the tables)
//! DESCRIBE table         (not SQL: lists the columns)
//!
//! cond  := NOT cond | cond AND cond | cond OR cond | ( cond ) | operand cmp operand
//!        | operand IS [NOT] NULL | operand [NOT] LIKE operand
//! cmp   := = != <> < <= > >=
//! operand := column | integer | decimal | 'text' | NULL
//! ```
//!
//! Identifiers are case-insensitive; `"quoted"` identifiers allow keywords and
//! spaces. Strings use single quotes (`''` for a quote). One statement per
//! query (a trailing `;` is fine).
//!
//! Values are NULL, exact decimal numbers (up to 18 digits, no floating
//! point) or text. Comparisons with NULL are unknown, so such rows don't match
//! (`WHERE x = NULL` matches nothing: use `IS NULL`). Numbers compare
//! numerically (`1 = 1.0`), text compares by characters, case-sensitively;
//! mixed number/text compares as number < text. `LIKE` uses `%` and `_` and
//! ignores ASCII case. Typed columns check on insert/update: INTEGER needs a
//! whole number, REAL/DECIMAL/NUMERIC any number, TEXT takes text (numbers are
//! stored as their text). Untyped columns take anything.
//!
//! Not supported (reported as errors): JOIN, GROUP BY, HAVING, DISTINCT,
//! OFFSET, subqueries, UNION, aliases (`AS`), expressions and functions other
//! than `COUNT(*)`, PRIMARY KEY/NOT NULL/DEFAULT, ALTER, indexes, transactions,
//! views, `IF [NOT] EXISTS`, multiple statements, and persistence of tables.
//!
//! Limits: 16 tables, 16 columns per table, 500 rows per table, text values
//! 200 characters, condition nesting 24.

mod exec;
mod lex;
mod num;
mod parse;

pub use exec::{format_table, Database, Outcome, SAMPLE_QUERIES};
pub use num::Num;

/// Short statement summary for the help text.
pub const SUPPORTED: &[&str] = &[
    "SELECT */cols/COUNT(*) FROM t",
    "  WHERE a=1 AND (b<'x' OR c IS NULL)",
    "  ORDER BY col [DESC] LIMIT n",
    "INSERT INTO t [(cols)] VALUES (..),(..)",
    "UPDATE t SET col=v [WHERE ..]",
    "DELETE FROM t [WHERE ..]",
    "CREATE TABLE t (col [INTEGER|REAL|TEXT],..)",
    "DROP TABLE t",
    "SHOW TABLES / DESCRIBE t",
    "LIKE uses % and _; no JOIN/GROUP BY",
];

pub const MAX_TABLES: usize = 16;
pub const MAX_COLUMNS: usize = 16;
pub const MAX_ROWS: usize = 500;
pub const MAX_TEXT: usize = 200;
pub const MAX_COND_DEPTH: usize = 24;

#[cfg(test)]
mod tests;
