# SQLite Client

A SQL query explorer and learning tool for the [Precursor](https://precursor.dev) handheld, with in-memory database, sample data, query history, and tabular results on the 336x536 monochrome 1-bit display.

## Features

- **SQL engine** — SELECT, INSERT, CREATE TABLE, DELETE, SHOW TABLES, DESCRIBE
- **WHERE filtering** — Comparison operators (=, !=, <, >, <=, >=)
- **3 sample tables** — users, products, orders with demo data
- **10 sample queries** — Common SQL patterns to browse and execute
- **Query history** — Re-run previous queries
- **Tabular results** — Column-aligned output with scrolling

## Architecture

- `sql.rs` — In-memory Database, Table, tokenizer, query executor, sample data
- `app.rs` — 4-state machine (QueryInput → Results, Samples, History)
- `ui.rs` — SQL input, tabular results, sample browser, history
- `main.rs` — Standard Xous event loop

## Controls

| Key | Action |
|-----|--------|
| Type | Enter SQL query |
| Enter | Execute query |
| Right | Sample queries |
| Up | Query history |
| Up/Down | Scroll results |
| Left | Back |
| Menu (∴) | Quit |

---

## Development

This app was developed using the methodology described in [xous-dev-toolkit](https://github.com/tbcolby/xous-dev-toolkit) — an LLM-assisted approach to Precursor app development on macOS ARM64.

---

## Author

Made by Tyler Colby — [Colby's Data Movers, LLC](https://colbysdatamovers.com)

Contact: [tyler@colbysdatamovers.com](mailto:tyler@colbysdatamovers.com) | [GitHub Issues](https://github.com/tbcolby/precursor-sqlite/issues)

---

## License

Licensed under the Apache License, Version 2.0.

See [LICENSE](LICENSE) for the full text.
