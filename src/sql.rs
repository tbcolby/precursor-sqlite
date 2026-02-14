//! Minimal SQL query engine with in-memory tables.
//! Supports SELECT, INSERT, CREATE TABLE, and WHERE filtering.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// A table column definition.
#[derive(Clone)]
pub struct Column {
    pub name: String,
}

/// An in-memory table.
#[derive(Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

/// Query result.
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub message: String,
}

/// In-memory database.
pub struct Database {
    pub tables: Vec<Table>,
}

impl Database {
    pub fn new() -> Self {
        Self { tables: Vec::new() }
    }

    /// Create a sample database with demo data.
    pub fn with_samples() -> Self {
        let mut db = Self::new();

        // Users table
        let mut users = Table {
            name: String::from("users"),
            columns: vec![
                Column { name: String::from("id") },
                Column { name: String::from("name") },
                Column { name: String::from("email") },
                Column { name: String::from("age") },
            ],
            rows: Vec::new(),
        };
        users.rows.push(vec![s("1"), s("Alice"), s("alice@example.com"), s("30")]);
        users.rows.push(vec![s("2"), s("Bob"), s("bob@example.com"), s("25")]);
        users.rows.push(vec![s("3"), s("Carol"), s("carol@example.com"), s("35")]);
        users.rows.push(vec![s("4"), s("Dave"), s("dave@example.com"), s("28")]);
        users.rows.push(vec![s("5"), s("Eve"), s("eve@example.com"), s("32")]);
        db.tables.push(users);

        // Products table
        let mut products = Table {
            name: String::from("products"),
            columns: vec![
                Column { name: String::from("id") },
                Column { name: String::from("name") },
                Column { name: String::from("price") },
                Column { name: String::from("stock") },
            ],
            rows: Vec::new(),
        };
        products.rows.push(vec![s("1"), s("Widget"), s("9.99"), s("100")]);
        products.rows.push(vec![s("2"), s("Gadget"), s("24.99"), s("50")]);
        products.rows.push(vec![s("3"), s("Doohickey"), s("4.99"), s("200")]);
        products.rows.push(vec![s("4"), s("Thingamajig"), s("14.99"), s("75")]);
        db.tables.push(products);

        // Orders table
        let mut orders = Table {
            name: String::from("orders"),
            columns: vec![
                Column { name: String::from("id") },
                Column { name: String::from("user_id") },
                Column { name: String::from("product") },
                Column { name: String::from("qty") },
            ],
            rows: Vec::new(),
        };
        orders.rows.push(vec![s("1"), s("1"), s("Widget"), s("3")]);
        orders.rows.push(vec![s("2"), s("2"), s("Gadget"), s("1")]);
        orders.rows.push(vec![s("3"), s("1"), s("Doohickey"), s("5")]);
        orders.rows.push(vec![s("4"), s("3"), s("Widget"), s("2")]);
        orders.rows.push(vec![s("5"), s("4"), s("Thingamajig"), s("1")]);
        db.tables.push(orders);

        db
    }

    fn find_table(&self, name: &str) -> Option<&Table> {
        let lower = to_lower_str(name);
        self.tables.iter().find(|t| to_lower_str(&t.name) == lower)
    }

    fn find_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        let lower = to_lower_str(name);
        self.tables.iter_mut().find(|t| to_lower_str(&t.name) == lower)
    }

    /// Execute a SQL query string.
    pub fn execute(&mut self, sql: &str) -> QueryResult {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return QueryResult { columns: Vec::new(), rows: Vec::new(), message: String::from("Empty query") };
        }

        let tokens = tokenize(trimmed);
        if tokens.is_empty() {
            return err("Empty query");
        }

        let cmd = to_lower_str(&tokens[0]);
        match cmd.as_str() {
            "select" => self.exec_select(&tokens),
            "insert" => self.exec_insert(&tokens),
            "create" => self.exec_create(&tokens),
            "show" | ".tables" => self.exec_show(),
            "describe" | ".schema" => {
                if tokens.len() >= 2 { self.exec_describe(&tokens[1]) } else { err("Usage: DESCRIBE table") }
            }
            "delete" => self.exec_delete(&tokens),
            _ => err(&format!("Unknown command: {}", tokens[0])),
        }
    }

    fn exec_select(&self, tokens: &[String]) -> QueryResult {
        // SELECT cols FROM table [WHERE col op val]
        let from_pos = tokens.iter().position(|t| to_lower_str(t) == "from");
        let from_pos = match from_pos { Some(p) => p, None => return err("Missing FROM") };
        if from_pos + 1 >= tokens.len() { return err("Missing table name"); }

        let table_name = &tokens[from_pos + 1];
        let table = match self.find_table(table_name) {
            Some(t) => t,
            None => return err(&format!("Table '{}' not found", table_name)),
        };

        // Parse column list
        let col_tokens: Vec<&str> = tokens[1..from_pos].iter().map(|s| s.as_str()).collect();
        let select_all = col_tokens.len() == 1 && col_tokens[0] == "*";
        let col_indices: Vec<usize> = if select_all {
            (0..table.columns.len()).collect()
        } else {
            let mut indices = Vec::new();
            for col_name in &col_tokens {
                let clean = col_name.trim_matches(',');
                if let Some(idx) = table.columns.iter().position(|c| to_lower_str(&c.name) == to_lower_str(clean)) {
                    indices.push(idx);
                } else {
                    return err(&format!("Column '{}' not found", clean));
                }
            }
            indices
        };

        // Parse WHERE clause
        let where_pos = tokens.iter().position(|t| to_lower_str(t) == "where");
        let filter: Option<(usize, String, String)> = if let Some(wp) = where_pos {
            if wp + 3 < tokens.len() {
                let col = &tokens[wp + 1];
                let op = &tokens[wp + 2];
                let val = tokens[wp + 3].trim_matches('\'').trim_matches('"');
                if let Some(idx) = table.columns.iter().position(|c| to_lower_str(&c.name) == to_lower_str(col)) {
                    Some((idx, String::from(op.as_str()), String::from(val)))
                } else {
                    return err(&format!("WHERE column '{}' not found", col));
                }
            } else {
                return err("Incomplete WHERE clause");
            }
        } else {
            None
        };

        let result_cols: Vec<String> = col_indices.iter().map(|&i| table.columns[i].name.clone()).collect();
        let mut result_rows = Vec::new();

        for row in &table.rows {
            if let Some((idx, ref op, ref val)) = filter {
                if !compare_value(&row[idx], op, val) { continue; }
            }
            let result_row: Vec<String> = col_indices.iter().map(|&i| row[i].clone()).collect();
            result_rows.push(result_row);
        }

        let msg = format!("{} row(s) returned", result_rows.len());
        QueryResult { columns: result_cols, rows: result_rows, message: msg }
    }

    fn exec_insert(&mut self, tokens: &[String]) -> QueryResult {
        // INSERT INTO table VALUES (v1, v2, ...)
        if tokens.len() < 3 || to_lower_str(&tokens[1]) != "into" {
            return err("Usage: INSERT INTO table VALUES (...)");
        }
        let table_name = tokens[2].clone();
        let values_pos = tokens.iter().position(|t| to_lower_str(t) == "values");
        let values_pos = match values_pos { Some(p) => p, None => return err("Missing VALUES") };

        // Extract values from parenthesized list
        let rest: String = tokens[values_pos + 1..].join(" ");
        let rest = rest.trim_start_matches('(').trim_end_matches(')').trim_end_matches(';');
        let values: Vec<String> = rest.split(',').map(|v| String::from(v.trim().trim_matches('\'').trim_matches('"'))).collect();

        let table = match self.find_table_mut(&table_name) {
            Some(t) => t,
            None => return err(&format!("Table '{}' not found", table_name)),
        };

        if values.len() != table.columns.len() {
            return err(&format!("Expected {} values, got {}", table.columns.len(), values.len()));
        }

        table.rows.push(values);
        QueryResult { columns: Vec::new(), rows: Vec::new(), message: format!("1 row inserted into {}", table_name) }
    }

    fn exec_create(&mut self, tokens: &[String]) -> QueryResult {
        // CREATE TABLE name (col1, col2, ...)
        if tokens.len() < 4 || to_lower_str(&tokens[1]) != "table" {
            return err("Usage: CREATE TABLE name (col1, col2, ...)");
        }
        let name = tokens[2].clone();
        if self.find_table(&name).is_some() {
            return err(&format!("Table '{}' already exists", name));
        }

        let rest: String = tokens[3..].join(" ");
        let rest = rest.trim_start_matches('(').trim_end_matches(')').trim_end_matches(';');
        let cols: Vec<Column> = rest.split(',').map(|c| {
            let parts: Vec<&str> = c.trim().split_whitespace().collect();
            Column { name: String::from(parts[0]) }
        }).collect();

        if cols.is_empty() { return err("Need at least one column"); }

        self.tables.push(Table { name: name.clone(), columns: cols, rows: Vec::new() });
        QueryResult { columns: Vec::new(), rows: Vec::new(), message: format!("Table '{}' created", name) }
    }

    fn exec_show(&self) -> QueryResult {
        let cols = vec![String::from("table_name"), String::from("columns"), String::from("rows")];
        let rows: Vec<Vec<String>> = self.tables.iter().map(|t| {
            vec![t.name.clone(), format!("{}", t.columns.len()), format!("{}", t.rows.len())]
        }).collect();
        let msg = format!("{} table(s)", rows.len());
        QueryResult { columns: cols, rows, message: msg }
    }

    fn exec_describe(&self, name: &str) -> QueryResult {
        let table = match self.find_table(name) {
            Some(t) => t,
            None => return err(&format!("Table '{}' not found", name)),
        };
        let cols = vec![String::from("column_name"), String::from("position")];
        let rows: Vec<Vec<String>> = table.columns.iter().enumerate().map(|(i, c)| {
            vec![c.name.clone(), format!("{}", i)]
        }).collect();
        let msg = format!("{} column(s) in {}", rows.len(), name);
        QueryResult { columns: cols, rows, message: msg }
    }

    fn exec_delete(&mut self, tokens: &[String]) -> QueryResult {
        // DELETE FROM table WHERE col op val
        if tokens.len() < 3 || to_lower_str(&tokens[1]) != "from" {
            return err("Usage: DELETE FROM table WHERE ...");
        }
        let table_name = tokens[2].clone();
        let where_pos = tokens.iter().position(|t| to_lower_str(t) == "where");

        let table = match self.find_table_mut(&table_name) {
            Some(t) => t,
            None => return err(&format!("Table '{}' not found", table_name)),
        };

        if let Some(wp) = where_pos {
            if wp + 3 < tokens.len() {
                let col = &tokens[wp + 1];
                let op = &tokens[wp + 2];
                let val = tokens[wp + 3].trim_matches('\'').trim_matches('"');
                if let Some(idx) = table.columns.iter().position(|c| to_lower_str(&c.name) == to_lower_str(col)) {
                    let before = table.rows.len();
                    table.rows.retain(|row| !compare_value(&row[idx], op, val));
                    let deleted = before - table.rows.len();
                    return QueryResult { columns: Vec::new(), rows: Vec::new(), message: format!("{} row(s) deleted", deleted) };
                } else {
                    return err(&format!("Column '{}' not found", col));
                }
            }
        }
        err("DELETE requires WHERE clause")
    }
}

fn s(val: &str) -> String { String::from(val) }

fn err(msg: &str) -> QueryResult {
    QueryResult { columns: Vec::new(), rows: Vec::new(), message: format!("Error: {}", msg) }
}

fn to_lower_str(s: &str) -> String {
    s.chars().map(|c| if c >= 'A' && c <= 'Z' { (c as u8 + 32) as char } else { c }).collect()
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '\'';

    for ch in input.chars() {
        if in_quote {
            if ch == quote_char {
                in_quote = false;
                tokens.push(current.clone());
                current = String::new();
            } else {
                current.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            in_quote = true;
            quote_char = ch;
            if !current.is_empty() { tokens.push(current.clone()); current = String::new(); }
        } else if ch == ' ' || ch == '\t' || ch == ',' || ch == '(' || ch == ')' || ch == ';' {
            if !current.is_empty() { tokens.push(current.clone()); current = String::new(); }
            // Keep commas and parens as part of context but don't create tokens for them
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() { tokens.push(current); }
    tokens
}

fn compare_value(cell: &str, op: &str, val: &str) -> bool {
    match op {
        "=" | "==" => cell == val,
        "!=" | "<>" => cell != val,
        ">" => {
            if let (Ok(a), Ok(b)) = (cell.parse::<i64>(), val.parse::<i64>()) { a > b }
            else { cell > val }
        }
        "<" => {
            if let (Ok(a), Ok(b)) = (cell.parse::<i64>(), val.parse::<i64>()) { a < b }
            else { cell < val }
        }
        ">=" => {
            if let (Ok(a), Ok(b)) = (cell.parse::<i64>(), val.parse::<i64>()) { a >= b }
            else { cell >= val }
        }
        "<=" => {
            if let (Ok(a), Ok(b)) = (cell.parse::<i64>(), val.parse::<i64>()) { a <= b }
            else { cell <= val }
        }
        _ => false,
    }
}

/// Sample queries for the demo.
pub static SAMPLE_QUERIES: &[&str] = &[
    "SELECT * FROM users",
    "SELECT name, email FROM users",
    "SELECT * FROM users WHERE age > 28",
    "SELECT * FROM products WHERE stock > 60",
    "SELECT * FROM orders WHERE user_id = 1",
    "SHOW TABLES",
    "DESCRIBE users",
    "INSERT INTO users VALUES (6, 'Frank', 'frank@example.com', 29)",
    "DELETE FROM users WHERE id = 5",
    "CREATE TABLE notes (id, title, body)",
];
