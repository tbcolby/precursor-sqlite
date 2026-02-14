# SQLite Client — Build Notes
## What This Is
SQL query explorer with in-memory database for Precursor.
## Key Decisions
- In-memory tables (Vec<Vec<String>>) — no actual SQLite dependency
- Tokenizer handles quoted strings, commas, parentheses
- Case-insensitive keyword/column matching
- WHERE with 6 comparison operators, numeric-aware
- 3 sample tables (users, products, orders) with 5 rows each
- 4 states: QueryInput, Results, Samples, History
