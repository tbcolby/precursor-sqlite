# SQL Explorer: agent notes

Rebuilt 2026-11-21 on `precursor-appkit` (strategy R). Read `CLAUDE.md` for layout.
Invariants with tests in `core/`:

- Multi-column `INSERT` (also several rows, with a column list) and `CREATE TABLE`
  work; every built-in sample query runs.
- Decimal columns compare numerically (`price < 5` is exactly right); NULL
  comparisons are unknown; unsupported clauses are rejected by name.
- No query, however mangled, panics.
- The README states the exact subset and does not claim SQLite compatibility.
- Every screen exits on Backspace or F4.
