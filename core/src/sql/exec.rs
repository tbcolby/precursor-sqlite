//! Executor: runs parsed statements against in-memory tables. All failures are
//! `Err(String)`; row and column access is by checked lookup.

use std::cmp::Ordering;

use super::parse::{parse, Cmp, Cond, Operand, Proj, Select, Stmt, Ty, Value};
use super::{MAX_COLUMNS, MAX_ROWS, MAX_TABLES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: Option<Ty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
}

/// What a statement produced, already formatted for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub message: String,
}

impl Outcome {
    fn msg(m: impl Into<String>) -> Outcome { Outcome { columns: Vec::new(), rows: Vec::new(), message: m.into() } }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Database {
    tables: Vec<Table>,
}

fn show(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Num(n) => n.to_string(),
        Value::Text(t) => t.clone(),
    }
}

/// Total order used by ORDER BY and comparisons: NULL < numbers < text.
fn order(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Num(x), Value::Num(y)) => x.cmp_num(y),
        (Value::Num(_), Value::Text(_)) => Ordering::Less,
        (Value::Text(_), Value::Num(_)) => Ordering::Greater,
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
    }
}

/// `%` any run, `_` any one char, ASCII case-insensitive.
fn like(text: &str, pat: &str) -> bool {
    let t: Vec<char> = text.chars().map(|c| c.to_ascii_lowercase()).collect();
    let p: Vec<char> = pat.chars().map(|c| c.to_ascii_lowercase()).collect();
    let (mut ti, mut pi) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '_' || p[pi] == t[ti]) && p[pi] != '%' {
            ti += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '%' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '%' {
        pi += 1;
    }
    pi == p.len()
}

impl Table {
    fn col_index(&self, name: &str) -> Option<usize> { self.columns.iter().position(|c| c.name.eq_ignore_ascii_case(name)) }

    fn need_col(&self, name: &str) -> Result<usize, String> {
        self.col_index(name).ok_or_else(|| format!("no column '{name}' in table '{}'", self.name))
    }

    fn operand(&self, op: &Operand, row: &[Value]) -> Result<Value, String> {
        match op {
            Operand::Lit(v) => Ok(v.clone()),
            Operand::Col(c) => {
                let i = self.need_col(c)?;
                row.get(i).cloned().ok_or_else(|| "internal error: short row".to_string())
            }
        }
    }

    fn check_cond(&self, c: &Cond) -> Result<(), String> {
        let check = |o: &Operand| match o {
            Operand::Col(n) => self.need_col(n).map(|_| ()),
            Operand::Lit(_) => Ok(()),
        };
        match c {
            Cond::Cmp(a, _, b) | Cond::Like(a, b, _) => {
                check(a)?;
                check(b)
            }
            Cond::IsNull(a, _) => check(a),
            Cond::And(a, b) | Cond::Or(a, b) => {
                self.check_cond(a)?;
                self.check_cond(b)
            }
            Cond::Not(a) => self.check_cond(a),
        }
    }

    /// Three-valued evaluation: `None` is "unknown" (NULL involved).
    fn eval(&self, c: &Cond, row: &[Value]) -> Result<Option<bool>, String> {
        Ok(match c {
            Cond::Cmp(a, op, b) => {
                let (x, y) = (self.operand(a, row)?, self.operand(b, row)?);
                if x == Value::Null || y == Value::Null {
                    None
                } else {
                    let o = order(&x, &y);
                    Some(match op {
                        Cmp::Eq => o == Ordering::Equal,
                        Cmp::Ne => o != Ordering::Equal,
                        Cmp::Lt => o == Ordering::Less,
                        Cmp::Le => o != Ordering::Greater,
                        Cmp::Gt => o == Ordering::Greater,
                        Cmp::Ge => o != Ordering::Less,
                    })
                }
            }
            Cond::IsNull(a, neg) => Some((self.operand(a, row)? == Value::Null) != *neg),
            Cond::Like(a, b, neg) => {
                let (x, y) = (self.operand(a, row)?, self.operand(b, row)?);
                if x == Value::Null || y == Value::Null {
                    None
                } else {
                    Some(like(&show(&x), &show(&y)) != *neg)
                }
            }
            Cond::And(a, b) => match (self.eval(a, row)?, self.eval(b, row)?) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            },
            Cond::Or(a, b) => match (self.eval(a, row)?, self.eval(b, row)?) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
            Cond::Not(a) => self.eval(a, row)?.map(|v| !v),
        })
    }

    fn matches(&self, filter: &Option<Cond>, row: &[Value]) -> Result<bool, String> {
        match filter {
            None => Ok(true),
            Some(c) => Ok(self.eval(c, row)? == Some(true)),
        }
    }

    /// Check/convert a value for column `i`.
    fn coerce(&self, i: usize, v: &Value) -> Result<Value, String> {
        let col = self.columns.get(i).ok_or_else(|| "internal error: bad column".to_string())?;
        Ok(match (col.ty, v) {
            (_, Value::Null) => Value::Null,
            (Some(Ty::Integer), Value::Num(n)) if n.is_integer() => v.clone(),
            (Some(Ty::Integer), other) => return Err(format!("column '{}' is INTEGER, got {}", col.name, describe(other))),
            (Some(Ty::Real), Value::Num(_)) => v.clone(),
            (Some(Ty::Real), other) => return Err(format!("column '{}' is REAL, got {}", col.name, describe(other))),
            (Some(Ty::Text), Value::Num(n)) => Value::Text(n.to_string()),
            (Some(Ty::Text), t) => t.clone(),
            (None, other) => other.clone(),
        })
    }
}

fn describe(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Num(n) => n.to_string(),
        Value::Text(t) => format!("'{t}'"),
    }
}

fn plural(n: usize) -> &'static str { if n == 1 { "" } else { "s" } }

impl Database {
    pub fn new() -> Self { Self::default() }

    /// The demo database: users, products, orders.
    pub fn with_samples() -> Self {
        let mut db = Self::new();
        for sql in [
            "CREATE TABLE users (id INTEGER, name TEXT, email TEXT, age INTEGER)",
            "INSERT INTO users VALUES (1, 'Alice', 'alice@example.com', 30), (2, 'Bob', 'bob@example.com', 25), \
             (3, 'Carol', 'carol@example.com', 35), (4, 'Dave', 'dave@example.com', 28), (5, 'Eve', 'eve@example.com', 32)",
            "CREATE TABLE products (id INTEGER, name TEXT, price REAL, stock INTEGER)",
            "INSERT INTO products VALUES (1, 'Widget', 9.99, 100), (2, 'Gadget', 24.99, 50), (3, 'Doohickey', 4.99, 200), \
             (4, 'Thingamajig', 14.99, 75)",
            "CREATE TABLE orders (id INTEGER, user_id INTEGER, product TEXT, qty INTEGER)",
            "INSERT INTO orders VALUES (1, 1, 'Widget', 3), (2, 2, 'Gadget', 1), (3, 1, 'Doohickey', 5), (4, 3, 'Widget', 2), \
             (5, 4, 'Thingamajig', 1)",
        ] {
            // The built-in setup is fixed text; a failure would be a bug caught by tests.
            let _ = db.execute(sql);
        }
        db
    }

    pub fn table_names(&self) -> Vec<&str> { self.tables.iter().map(|t| t.name.as_str()).collect() }

    pub fn row_count(&self, table: &str) -> Option<usize> { self.find(table).map(|t| t.rows.len()) }

    fn find(&self, name: &str) -> Option<&Table> { self.tables.iter().find(|t| t.name.eq_ignore_ascii_case(name)) }

    fn need(&self, name: &str) -> Result<usize, String> {
        self.tables.iter().position(|t| t.name.eq_ignore_ascii_case(name)).ok_or_else(|| format!("no table '{name}'"))
    }

    /// Parse and run one statement.
    pub fn execute(&mut self, sql: &str) -> Result<Outcome, String> {
        match parse(sql)? {
            Stmt::Select(s) => self.select(&s),
            Stmt::Insert { table, columns, rows } => self.insert(&table, columns, rows),
            Stmt::Update { table, sets, filter } => self.update(&table, sets, filter),
            Stmt::Delete { table, filter } => self.delete(&table, filter),
            Stmt::Create { table, columns } => self.create(table, columns),
            Stmt::Drop { table } => {
                let i = self.need(&table)?;
                let t = self.tables.remove(i);
                Ok(Outcome::msg(format!("Table '{}' dropped", t.name)))
            }
            Stmt::ShowTables => Ok(Outcome {
                columns: vec!["table".into(), "columns".into(), "rows".into()],
                rows: self.tables.iter().map(|t| vec![t.name.clone(), t.columns.len().to_string(), t.rows.len().to_string()]).collect(),
                message: format!("{} table{}", self.tables.len(), plural(self.tables.len())),
            }),
            Stmt::Describe { table } => {
                let t = &self.tables[self.need(&table)?];
                Ok(Outcome {
                    columns: vec!["column".into(), "type".into()],
                    rows: t.columns.iter().map(|c| vec![c.name.clone(), c.ty.map_or("(any)", Ty::name).to_string()]).collect(),
                    message: format!("{} column{} in {}", t.columns.len(), plural(t.columns.len()), t.name),
                })
            }
        }
    }

    fn select(&self, s: &Select) -> Result<Outcome, String> {
        let t = &self.tables[self.need(&s.table)?];
        if let Some(c) = &s.filter {
            t.check_cond(c)?;
        }
        let order_idx: Vec<(usize, bool)> = s.order.iter().map(|(c, d)| t.need_col(c).map(|i| (i, *d))).collect::<Result<_, _>>()?;
        let mut hits: Vec<&Vec<Value>> = Vec::new();
        for row in &t.rows {
            if t.matches(&s.filter, row)? {
                hits.push(row);
            }
        }
        if matches!(s.proj, Proj::Count) {
            if !s.order.is_empty() {
                return Err("ORDER BY makes no sense with COUNT(*)".into());
            }
            let rows = if s.limit == Some(0) { Vec::new() } else { vec![vec![hits.len().to_string()]] };
            let message = format!("{} row{}", rows.len(), plural(rows.len()));
            return Ok(Outcome { columns: vec!["count".into()], rows, message });
        }
        // Stable sort so equal keys keep insertion order.
        hits.sort_by(|a, b| {
            for &(i, desc) in &order_idx {
                let o = match (a.get(i), b.get(i)) {
                    (Some(x), Some(y)) => order(x, y),
                    _ => Ordering::Equal,
                };
                let o = if desc { o.reverse() } else { o };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        if let Some(l) = s.limit {
            hits.truncate(l);
        }
        let idx: Vec<usize> = match &s.proj {
            Proj::Star => (0..t.columns.len()).collect(),
            Proj::Cols(cs) => cs.iter().map(|c| t.need_col(c)).collect::<Result<_, _>>()?,
            Proj::Count => Vec::new(),
        };
        let columns = idx.iter().filter_map(|&i| t.columns.get(i).map(|c| c.name.clone())).collect();
        let rows: Vec<Vec<String>> = hits.iter().map(|r| idx.iter().filter_map(|&i| r.get(i).map(show)).collect()).collect();
        let message = format!("{} row{}", rows.len(), plural(rows.len()));
        Ok(Outcome { columns, rows, message })
    }

    fn insert(&mut self, table: &str, columns: Option<Vec<String>>, rows: Vec<Vec<Value>>) -> Result<Outcome, String> {
        let ti = self.need(table)?;
        let t = &self.tables[ti];
        if t.rows.len() + rows.len() > MAX_ROWS {
            return Err(format!("table '{}' would exceed {MAX_ROWS} rows", t.name));
        }
        // Target column positions.
        let targets: Vec<usize> = match &columns {
            Some(cs) => {
                let mut v = Vec::new();
                for c in cs {
                    let i = t.need_col(c)?;
                    if v.contains(&i) {
                        return Err(format!("column '{c}' listed twice"));
                    }
                    v.push(i);
                }
                v
            }
            None => (0..t.columns.len()).collect(),
        };
        let mut built = Vec::new();
        for (n, vals) in rows.iter().enumerate() {
            if vals.len() != targets.len() {
                return Err(format!(
                    "row {} has {} value{}, expected {} (table '{}' has {} column{})",
                    n + 1,
                    vals.len(),
                    plural(vals.len()),
                    targets.len(),
                    t.name,
                    t.columns.len(),
                    plural(t.columns.len())
                ));
            }
            let mut row = vec![Value::Null; t.columns.len()];
            for (v, &i) in vals.iter().zip(&targets) {
                row[i] = t.coerce(i, v)?;
            }
            built.push(row);
        }
        let n = built.len();
        let name = t.name.clone();
        self.tables[ti].rows.extend(built);
        Ok(Outcome::msg(format!("{n} row{} inserted into {name}", plural(n))))
    }

    fn update(&mut self, table: &str, sets: Vec<(String, Value)>, filter: Option<Cond>) -> Result<Outcome, String> {
        let ti = self.need(table)?;
        let t = &self.tables[ti];
        if let Some(c) = &filter {
            t.check_cond(c)?;
        }
        let mut changes = Vec::new();
        for (c, v) in &sets {
            let i = t.need_col(c)?;
            if changes.iter().any(|(j, _)| *j == i) {
                return Err(format!("column '{c}' set twice"));
            }
            changes.push((i, t.coerce(i, v)?));
        }
        let mut hit = Vec::new();
        for (ri, row) in t.rows.iter().enumerate() {
            if t.matches(&filter, row)? {
                hit.push(ri);
            }
        }
        let n = hit.len();
        for ri in hit {
            if let Some(row) = self.tables[ti].rows.get_mut(ri) {
                for (i, v) in &changes {
                    if let Some(cell) = row.get_mut(*i) {
                        *cell = v.clone();
                    }
                }
            }
        }
        Ok(Outcome::msg(format!("{n} row{} updated", plural(n))))
    }

    fn delete(&mut self, table: &str, filter: Option<Cond>) -> Result<Outcome, String> {
        let ti = self.need(table)?;
        if let Some(c) = &filter {
            self.tables[ti].check_cond(c)?;
        }
        let t = &self.tables[ti];
        let mut keep = Vec::with_capacity(t.rows.len());
        for row in &t.rows {
            keep.push(!t.matches(&filter, row)?);
        }
        let before = t.rows.len();
        let mut it = keep.iter();
        self.tables[ti].rows.retain(|_| *it.next().unwrap_or(&true));
        let n = before - self.tables[ti].rows.len();
        Ok(Outcome::msg(format!("{n} row{} deleted", plural(n))))
    }

    fn create(&mut self, table: String, columns: Vec<(String, Option<Ty>)>) -> Result<Outcome, String> {
        if self.find(&table).is_some() {
            return Err(format!("table '{table}' already exists"));
        }
        if self.tables.len() >= MAX_TABLES {
            return Err(format!("at most {MAX_TABLES} tables"));
        }
        if columns.is_empty() || columns.len() > MAX_COLUMNS {
            return Err(format!("a table needs 1 to {MAX_COLUMNS} columns"));
        }
        for (i, (n, _)) in columns.iter().enumerate() {
            if columns[..i].iter().any(|(m, _)| m.eq_ignore_ascii_case(n)) {
                return Err(format!("column '{n}' appears twice"));
            }
        }
        let cols = columns.into_iter().map(|(name, ty)| Column { name, ty }).collect::<Vec<_>>();
        let n = cols.len();
        self.tables.push(Table { name: table.clone(), columns: cols, rows: Vec::new() });
        Ok(Outcome::msg(format!("Table '{table}' created with {n} column{}", plural(n))))
    }
}

/// Lay a result out as text lines: header, rule, rows. Columns are padded to
/// their widest cell (at most `cell_max` characters, longer cells end in `…`)
/// so the lines align in a monospace font.
pub fn format_table(columns: &[String], rows: &[Vec<String>], cell_max: usize) -> Vec<String> {
    let clean = |s: &str| -> String {
        let one: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        if one.chars().count() > cell_max {
            let mut t: String = one.chars().take(cell_max.saturating_sub(1)).collect();
            t.push('\u{2026}');
            t
        } else {
            one
        }
    };
    let n = columns.len();
    let head: Vec<String> = columns.iter().map(|c| clean(c)).collect();
    let body: Vec<Vec<String>> = rows.iter().map(|r| r.iter().take(n).map(|c| clean(c)).collect()).collect();
    let mut widths: Vec<usize> = head.iter().map(|h| h.chars().count()).collect();
    for r in &body {
        for (i, c) in r.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(c.chars().count());
            }
        }
    }
    let line = |cells: &[String]| -> String {
        let parts: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let c = cells.get(i).map_or("", |s| s.as_str());
                format!("{c:<w$}", w = *w)
            })
            .collect();
        parts.join(" | ").trim_end().to_string()
    };
    let mut out = vec![line(&head)];
    out.push(widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("-+-"));
    out.extend(body.iter().map(|r| line(r)));
    out
}

/// Queries offered on the samples screen. Every one runs on the sample database
/// (a test executes them all, singly and in order).
pub const SAMPLE_QUERIES: &[&str] = &[
    "SELECT * FROM users",
    "SELECT name, email FROM users WHERE age > 28",
    "SELECT name, price FROM products WHERE price < 10",
    "SELECT name FROM users WHERE age >= 25 AND name LIKE '%a%' ORDER BY age DESC LIMIT 2",
    "SELECT COUNT(*) FROM orders WHERE user_id = 1",
    "SHOW TABLES",
    "DESCRIBE products",
    "INSERT INTO users VALUES (6, 'Frank', 'frank@example.com', 29)",
    "UPDATE products SET stock = 60 WHERE name = 'Gadget'",
    "DELETE FROM users WHERE id = 5",
    "CREATE TABLE notes (id INTEGER, title TEXT, body)",
];
