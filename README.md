# precursor-sqlite: SQL Explorer

A small SQL playground for the [Precursor](https://precursor.dev): an **in-memory** database with three sample tables, a query editor, tabular results you can scroll, sample queries and query history. **It is not SQLite.** It does not read or write SQLite files and makes no compatibility promise: it understands the small subset of SQL listed below, and rejects everything else with a message naming what isn't supported. Tables exist only while the app is running.

## Supported SQL (exactly this)

```text
SELECT * | col [, col ...] | COUNT(*)  FROM table
       [WHERE cond] [ORDER BY col [ASC|DESC] [, ...]] [LIMIT n]
INSERT INTO table [(col, ...)] VALUES (v, ...) [, (v, ...) ...]
UPDATE table SET col = v [, col = v ...] [WHERE cond]
DELETE FROM table [WHERE cond]
CREATE TABLE table (col [INTEGER|INT|REAL|DECIMAL|NUMERIC|TEXT], ...)
DROP TABLE table
SHOW TABLES            (not SQL: lists the tables)
DESCRIBE table         (not SQL: lists the columns)

cond    := NOT cond | cond AND cond | cond OR cond | ( cond ) | operand cmp operand
         | operand IS [NOT] NULL | operand [NOT] LIKE operand
cmp     := =  !=  <>  <  <=  >  >=
operand := column | integer | decimal | 'text' | NULL
```

- Names are case-insensitive. `"quoted"` identifiers allow keywords and spaces. Strings use single quotes (`''` for a quote). One statement per query; a trailing `;` is fine. Operators don't need spaces (`age>28`).
- Values are NULL, exact decimal numbers (up to 18 digits; no floating point, so `price < 5` is exactly right and `1 = 1.0`), or text up to 200 characters.
- `NULL` comparisons are unknown, so `WHERE x = NULL` matches nothing; use `IS NULL`. NULL sorts first, then numbers, then text.
- `LIKE` uses `%` (any run) and `_` (any one character) and ignores ASCII case. Other text comparison is by character and case-sensitive.
- Typed columns check values on `INSERT`/`UPDATE`: INTEGER takes whole numbers, REAL/DECIMAL/NUMERIC any number, TEXT takes text (a number is stored as its text). Columns without a type take anything.
- Limits: 16 tables, 16 columns per table, 500 rows per table, queries up to 300 characters.

**Not supported** (each is reported as an error, not ignored): JOIN, GROUP BY, HAVING, DISTINCT, OFFSET, subqueries, UNION, aliases (`AS`), `table.column` names, expressions and functions other than `COUNT(*)`, PRIMARY KEY / NOT NULL / DEFAULT and other column options, ALTER, indexes, views, transactions, `IF [NOT] EXISTS`, several statements at once, and saving tables.

## Features

- **Sample database** (`users`, `products`, `orders`) and sample queries covering every statement type; "Reset sample database" is the last entry of the samples list.
- **Results table** in a monospace font with the header pinned; scroll rows with ↑ ↓ and columns with ← →. A row count or message is shown in the title.
- **Errors where you can fix them:** a failed query stays in the editor with the reason below it.
- **History** of the last 20 successful queries (newest first), kept between runs.
- **In-app help** (F3) listing the supported subset.

## Keys

| Screen | Key | Action |
|---|---|---|
| Query | type | Insert at the cursor |
| | ← → | Move the cursor |
| | ↑ ↓ | Jump to start / end |
| | Enter | Run |
| | F1 / F2 / F3 | Samples / history / what is supported |
| | Backspace | Delete a character; on an empty query, leave the app |
| | F4 | Leave the app |
| Results | ↑ ↓ ← → | Scroll rows / columns |
| | Enter | New query (clears the editor) |
| | Backspace / F4 | Back to the editor, query kept |
| Samples | ↑ ↓, Enter | Choose and run (the last entry resets the sample database) |
| | F1 | Copy into the editor without running |
| | Backspace / F4 | Back |
| History | ↑ ↓, Enter | Choose and copy into the editor (press Enter again to run) |
| | Backspace / F4 | Back |
| Help | ↑ ↓ | Scroll |
| | Enter, Backspace, F4 | Back |

The menu key `∴` opens the system menu; the app never sees it.

## Storage

PDDB dictionary `sql-explorer`, key `history`: `{"version":1,"history":["SELECT ...", ...]}`, oldest first, at most 20 queries. Written only when it changed, when the app goes to the background or you leave it. When a PDDB basis is locked or unlocked the saved history is reloaded (the in-memory tables are left alone). Data that can't be read, or comes from a newer version, is left untouched and not overwritten. **Tables and rows are never stored**; they reset when the app restarts.

## Limitations

- Everything under "Not supported" above; this is a teaching tool, not a database.
- Tables are lost when the app exits.
- Wide results scroll sideways; cells longer than 14 characters are shortened with an ellipsis in the table view.
- The result table assumes a fixed character width and is clipped, not wrapped, so very wide glyphs may be cut off.
- Renode capture not yet run. `capture.py` is written but not executed; verification so far is the hosted build, clippy, lint and the host tests in `core/`.

## Build and test

```bash
cd core && cargo test                                   # host tests
cd ../xous-dev-toolkit
bin/integrate --xous-root <xous-core> --grade -v ../precursor-sqlite
```

The app lives at `xous-core/apps/sql-explorer/`; add the `manifest-fragment.json` entry to `apps/manifest.json` (`bin/integrate` does this). The manifest key, package name and `gam::APP_NAME_SQL_EXPLORER` must match. The package was renamed from `sqlite` to `sql-explorer` because it isn't SQLite.

## Development

Developed with [xous-dev-toolkit](https://github.com/tbcolby/xous-dev-toolkit).

## Author

Made by Tyler Colby, [Colby's Data Movers, LLC](https://colbysdatamovers.com). Contact: [tyler@colbysdatamovers.com](mailto:tyler@colbysdatamovers.com) | [GitHub Issues](https://github.com/tbcolby/precursor-sqlite/issues)

## License

Apache License, Version 2.0. See [LICENSE](LICENSE).
