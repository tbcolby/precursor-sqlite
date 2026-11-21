//! Parser for the subset documented in the parent module. Every failure is an
//! `Err(String)`; nothing indexes without checking.

use super::lex::{tokenize, Tok};
use super::num::Num;
use super::{MAX_COLUMNS, MAX_COND_DEPTH, MAX_TEXT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Num(Num),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Integer,
    Real,
    Text,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Integer => "INTEGER",
            Ty::Real => "REAL",
            Ty::Text => "TEXT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    Lit(Value),
    Col(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    Cmp(Operand, Cmp, Operand),
    IsNull(Operand, bool),
    Like(Operand, Operand, bool),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proj {
    Star,
    Cols(Vec<String>),
    Count,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Select {
    pub proj: Proj,
    pub table: String,
    pub filter: Option<Cond>,
    pub order: Vec<(String, bool)>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Select(Select),
    Insert { table: String, columns: Option<Vec<String>>, rows: Vec<Vec<Value>> },
    Update { table: String, sets: Vec<(String, Value)>, filter: Option<Cond> },
    Delete { table: String, filter: Option<Cond> },
    Create { table: String, columns: Vec<(String, Option<Ty>)> },
    Drop { table: String },
    ShowTables,
    Describe { table: String },
}

/// Words that can't be used as bare names, and words we recognise but don't support.
const KEYWORDS: &[&str] = &[
    "select", "from", "where", "order", "by", "limit", "insert", "into", "values", "update", "set", "delete", "create", "drop",
    "table", "and", "or", "not", "null", "is", "like", "asc", "desc", "show", "describe", "count",
];
const UNSUPPORTED: &[&str] = &[
    "join", "inner", "outer", "left", "right", "cross", "group", "having", "distinct", "offset", "union", "intersect", "except",
    "as", "in", "between", "exists", "case", "when", "primary", "key", "default", "unique", "references", "check", "alter",
    "index", "view", "begin", "commit", "rollback", "replace", "with", "using", "natural", "on",
];

struct P {
    toks: Vec<Tok>,
    pos: usize,
}

type R<T> = Result<T, String>;

impl P {
    fn peek(&self) -> Option<&Tok> { self.toks.get(self.pos) }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn peek_kw(&self, kw: &str) -> bool { matches!(self.peek(), Some(Tok::Word(w)) if w.eq_ignore_ascii_case(kw)) }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.peek_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> R<()> {
        if self.eat_kw(kw) {
            Ok(())
        } else if let Some(Tok::Word(w)) = self.peek().filter(|t| matches!(t, Tok::Word(w) if UNSUPPORTED.contains(&w.to_ascii_lowercase().as_str()))) {
            Err(format!("{} is not supported", w.to_uppercase()))
        } else {
            Err(format!("expected {}, found {}", kw.to_uppercase(), self.found()))
        }
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: Tok) -> R<()> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(format!("expected {}, found {}", t.describe(), self.found()))
        }
    }

    fn found(&self) -> String { self.peek().map_or("end of query".to_string(), Tok::describe) }

    /// A table or column name.
    fn ident(&mut self, what: &str) -> R<String> {
        match self.next() {
            Some(Tok::Quoted(q)) => {
                if q.is_empty() {
                    Err(format!("empty {what} name"))
                } else {
                    Ok(q)
                }
            }
            Some(Tok::Word(w)) => {
                let lw = w.to_ascii_lowercase();
                if KEYWORDS.contains(&lw.as_str()) {
                    Err(format!("'{w}' is a keyword; quote it (\"{w}\") to use it as a {what} name"))
                } else if UNSUPPORTED.contains(&lw.as_str()) {
                    Err(format!("{} is not supported", w.to_uppercase()))
                } else {
                    Ok(w)
                }
            }
            Some(t) => Err(format!("expected a {what} name, found {}", t.describe())),
            None => Err(format!("expected a {what} name, found end of query")),
        }
    }

    fn number(&mut self) -> R<Num> {
        let neg = self.eat(&Tok::Minus);
        match self.next() {
            Some(Tok::Num(n)) => Num::parse(&if neg { format!("-{n}") } else { n }),
            Some(t) => Err(format!("expected a number, found {}", t.describe())),
            None => Err("expected a number, found end of query".into()),
        }
    }

    fn literal(&mut self) -> R<Value> {
        match self.peek() {
            Some(Tok::Str(_)) => match self.next() {
                Some(Tok::Str(s)) => {
                    if s.chars().count() > MAX_TEXT {
                        Err(format!("text longer than {MAX_TEXT} characters"))
                    } else {
                        Ok(Value::Text(s))
                    }
                }
                _ => Err("internal parser error".into()),
            },
            Some(Tok::Num(_) | Tok::Minus) => Ok(Value::Num(self.number()?)),
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("null") => {
                self.pos += 1;
                Ok(Value::Null)
            }
            _ => Err(format!("expected a value (number, 'text' or NULL), found {}", self.found())),
        }
    }

    fn operand(&mut self) -> R<Operand> {
        match self.peek() {
            Some(Tok::Str(_) | Tok::Num(_) | Tok::Minus) => Ok(Operand::Lit(self.literal()?)),
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("null") => Ok(Operand::Lit(self.literal()?)),
            Some(Tok::Word(_) | Tok::Quoted(_)) => Ok(Operand::Col(self.ident("column")?)),
            _ => Err(format!("expected a column or value, found {}", self.found())),
        }
    }

    // cond := or ; or := and (OR and)* ; and := not (AND not)* ; not := NOT not | primary
    fn cond(&mut self, depth: usize) -> R<Cond> {
        if depth > MAX_COND_DEPTH {
            return Err(format!("condition nested deeper than {MAX_COND_DEPTH}"));
        }
        let mut left = self.and(depth)?;
        while self.eat_kw("or") {
            let right = self.and(depth)?;
            left = Cond::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self, depth: usize) -> R<Cond> {
        let mut left = self.not(depth)?;
        while self.eat_kw("and") {
            let right = self.not(depth)?;
            left = Cond::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn not(&mut self, depth: usize) -> R<Cond> {
        if depth > MAX_COND_DEPTH {
            return Err(format!("condition nested deeper than {MAX_COND_DEPTH}"));
        }
        if self.eat_kw("not") {
            return Ok(Cond::Not(Box::new(self.not(depth + 1)?)));
        }
        if self.eat(&Tok::LParen) {
            let c = self.cond(depth + 1)?;
            self.expect(Tok::RParen)?;
            return Ok(c);
        }
        let left = self.operand()?;
        if self.eat_kw("is") {
            let neg = self.eat_kw("not");
            self.expect_kw("null")?;
            return Ok(Cond::IsNull(left, neg));
        }
        let neg = self.peek_kw("not") && matches!(self.toks.get(self.pos + 1), Some(Tok::Word(w)) if w.eq_ignore_ascii_case("like"));
        if neg {
            self.pos += 1;
        }
        if self.eat_kw("like") {
            let right = self.operand()?;
            return Ok(Cond::Like(left, right, neg));
        }
        let op = match self.next() {
            Some(Tok::Eq) => Cmp::Eq,
            Some(Tok::Ne) => Cmp::Ne,
            Some(Tok::Lt) => Cmp::Lt,
            Some(Tok::Le) => Cmp::Le,
            Some(Tok::Gt) => Cmp::Gt,
            Some(Tok::Ge) => Cmp::Ge,
            Some(Tok::Word(w)) if UNSUPPORTED.contains(&w.to_ascii_lowercase().as_str()) => {
                return Err(format!("{} is not supported", w.to_uppercase()))
            }
            Some(t) => return Err(format!("expected a comparison (= != < <= > >= IS LIKE), found {}", t.describe())),
            None => return Err("incomplete condition: expected a comparison".into()),
        };
        let right = self.operand()?;
        Ok(Cond::Cmp(left, op, right))
    }

    fn name_list(&mut self, what: &str) -> R<Vec<String>> {
        let mut v = vec![self.ident(what)?];
        while self.eat(&Tok::Comma) {
            v.push(self.ident(what)?);
        }
        Ok(v)
    }

    fn where_clause(&mut self) -> R<Option<Cond>> {
        if self.eat_kw("where") {
            Ok(Some(self.cond(0)?))
        } else {
            Ok(None)
        }
    }

    fn select(&mut self) -> R<Stmt> {
        if self.peek_kw("from") {
            return Err("expected a column list (or *) before FROM".into());
        }
        let proj = if self.eat(&Tok::Star) {
            Proj::Star
        } else if self.peek_kw("count") {
            self.pos += 1;
            self.expect(Tok::LParen)?;
            self.expect(Tok::Star)?;
            self.expect(Tok::RParen)?;
            Proj::Count
        } else {
            Proj::Cols(self.name_list("column")?)
        };
        self.expect_kw("from")?;
        let table = self.ident("table")?;
        let filter = self.where_clause()?;
        let mut order = Vec::new();
        if self.eat_kw("order") {
            self.expect_kw("by")?;
            loop {
                let col = self.ident("column")?;
                let desc = if self.eat_kw("desc") {
                    true
                } else {
                    self.eat_kw("asc");
                    false
                };
                order.push((col, desc));
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        let limit = if self.eat_kw("limit") {
            let n = self.number()?;
            if !n.is_integer() || n.cmp_num(&Num::int(0)).is_lt() {
                return Err("LIMIT needs a whole number, 0 or more".into());
            }
            n.to_string().parse::<usize>().ok().or(Some(usize::MAX))
        } else {
            None
        };
        Ok(Stmt::Select(Select { proj, table, filter, order, limit }))
    }

    fn insert(&mut self) -> R<Stmt> {
        self.expect_kw("into")?;
        let table = self.ident("table")?;
        let columns = if self.eat(&Tok::LParen) {
            let c = self.name_list("column")?;
            self.expect(Tok::RParen)?;
            Some(c)
        } else {
            None
        };
        self.expect_kw("values")?;
        let mut rows = Vec::new();
        loop {
            self.expect(Tok::LParen)?;
            let mut row = vec![self.literal()?];
            while self.eat(&Tok::Comma) {
                row.push(self.literal()?);
            }
            self.expect(Tok::RParen)?;
            rows.push(row);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Ok(Stmt::Insert { table, columns, rows })
    }

    fn update(&mut self) -> R<Stmt> {
        let table = self.ident("table")?;
        self.expect_kw("set")?;
        let mut sets = Vec::new();
        loop {
            let col = self.ident("column")?;
            self.expect(Tok::Eq)?;
            sets.push((col, self.literal()?));
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        let filter = self.where_clause()?;
        Ok(Stmt::Update { table, sets, filter })
    }

    fn create(&mut self) -> R<Stmt> {
        self.expect_kw("table")?;
        let table = self.ident("table")?;
        self.expect(Tok::LParen)?;
        let mut columns = Vec::new();
        loop {
            let name = self.ident("column")?;
            let ty = match self.peek() {
                Some(Tok::Word(w)) => {
                    let lw = w.to_ascii_lowercase();
                    let ty = match lw.as_str() {
                        "integer" | "int" => Some(Ty::Integer),
                        "real" | "decimal" | "numeric" => Some(Ty::Real),
                        "text" => Some(Ty::Text),
                        other if UNSUPPORTED.contains(&other) || KEYWORDS.contains(&other) => {
                            return Err(format!("column option {} is not supported", w.to_uppercase()))
                        }
                        _ => return Err(format!("unknown column type '{w}' (use INTEGER, REAL, TEXT or none)")),
                    };
                    self.pos += 1;
                    ty
                }
                _ => None,
            };
            if let Some(Tok::Word(w)) = self.peek() {
                return Err(format!("column option {} is not supported", w.to_uppercase()));
            }
            columns.push((name, ty));
            if columns.len() > MAX_COLUMNS {
                return Err(format!("more than {MAX_COLUMNS} columns"));
            }
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RParen)?;
        Ok(Stmt::Create { table, columns })
    }

    fn statement(&mut self) -> R<Stmt> {
        let first = match self.next() {
            Some(Tok::Word(w)) => w,
            Some(t) => return Err(format!("a statement can't start with {}", t.describe())),
            None => return Err("empty query".into()),
        };
        let stmt = match first.to_ascii_lowercase().as_str() {
            "select" => self.select()?,
            "insert" => self.insert()?,
            "update" => self.update()?,
            "delete" => {
                self.expect_kw("from")?;
                let table = self.ident("table")?;
                let filter = self.where_clause()?;
                Stmt::Delete { table, filter }
            }
            "create" => self.create()?,
            "drop" => {
                self.expect_kw("table")?;
                Stmt::Drop { table: self.ident("table")? }
            }
            "show" => {
                self.expect_kw("tables")?;
                Stmt::ShowTables
            }
            "describe" => Stmt::Describe { table: self.ident("table")? },
            other if UNSUPPORTED.contains(&other) => return Err(format!("{} is not supported", first.to_uppercase())),
            _ => return Err(format!("unknown command '{first}'. Try SELECT, INSERT, UPDATE, DELETE, CREATE TABLE, DROP TABLE, SHOW TABLES, DESCRIBE")),
        };
        let had_semi = self.eat(&Tok::Semi);
        match self.peek() {
            None => Ok(stmt),
            Some(Tok::Word(w)) if UNSUPPORTED.contains(&w.to_ascii_lowercase().as_str()) => Err(format!("{} is not supported", w.to_uppercase())),
            Some(_) if had_semi => Err("one statement at a time".into()),
            Some(Tok::Dot) => Err("qualified names (table.column) are not supported".into()),
            Some(t) => Err(format!("unexpected {} after the statement", t.describe())),
        }
    }
}

pub fn parse(sql: &str) -> R<Stmt> {
    let toks = tokenize(sql)?;
    if toks.is_empty() {
        return Err("empty query".into());
    }
    P { toks, pos: 0 }.statement()
}
