# SQL Explorer: build notes

## What it is (and isn't)
A small in-memory SQL subset for learning and poking at sample tables. **Not
SQLite**: no file format, no compatibility claim. `core/src/sql.rs` holds the
exact supported grammar; the README and the in-app help (F3, `SUPPORTED`) repeat
it. Anything outside it is an error that says so, never a silent partial answer
(the old engine ignored everything after the first WHERE condition).

## Architecture
- `core/` (`sql-explorer-core`, no Xous deps, host-tested):
  - `sql/num`: exact decimals (<= 18 digits, no floats); `sql/lex`: tokenizer
    (operators without spaces, `''` escapes, `"quoted"` names); `sql/parse`:
    recursive-descent parser (condition depth capped at 24); `sql/exec`: tables,
    three-valued WHERE, ORDER BY, LIMIT, COUNT(*), typed columns, `format_table`.
  - `model`: screen state machine (Query, Results, Samples, History, Help).
- `src/main.rs` routes GAM messages (template pattern, no pump).
  `src/app.rs` saves the query history (only if dirty), reloads it on basis change
  (tables stay), draws with `appkit::screen::Screen`; results use `Monospace`.
- PDDB dict `sql-explorer`, key `history`: `{"version":1,"history":[...]}`.
  Tables are session-only by design.

## Rules worth remembering
- A parse or execute failure is `Err(String)`; nothing may panic on any input
  (`never_panics_on_arbitrary_input` and `truncating_every_sample_never_panics`).
- Every entry in `SAMPLE_QUERIES` must run, singly and in order (tests).
- New syntax: add it to the grammar comment, `SUPPORTED`, the README and a test.

## Test / gate
```bash
cd core && cargo test
cd ../xous-dev-toolkit && bin/integrate --xous-root <xous-core> --grade -v ../precursor-sqlite
```
