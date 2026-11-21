//! Screen state machine for SQL Explorer. The shell maps device keys to
//! [`Key`], calls [`Model::on_key`], and draws from the accessors.
//!
//! Screens: `Query` (type SQL), `Results` (table with scrolling both ways),
//! `Samples` (ready-made queries, plus a database reset), `History` and `Help`
//! (the supported subset). The database lives in memory for the session.

use precursor_appkit_core::keys::Key;
use precursor_appkit_core::layout::ListState;
use precursor_appkit_core::text::TextField;
use serde::{Deserialize, Serialize};

use crate::sql::{format_table, Database, SAMPLE_QUERIES, SUPPORTED};

pub const STATE_VERSION: u32 = 1;
/// Longest query the input accepts, in characters.
pub const MAX_QUERY_CHARS: usize = 300;
/// Queries remembered.
pub const MAX_HISTORY: usize = 20;
const CELL_MAX: usize = 14;
/// Columns scrolled per Left/Right press.
const HSTEP: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub version: u32,
    /// Oldest first.
    pub history: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Written by a newer version of the app; don't overwrite it.
    NewerVersion(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Query,
    Results,
    Samples,
    History,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Stay,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub list_rows: usize,
    /// Lines of table the Results screen can show (including the 2 header lines).
    pub text_rows: usize,
    pub text_cols: usize,
}

impl Default for Viewport {
    fn default() -> Self { Viewport { list_rows: 10, text_rows: 14, text_cols: 32 } }
}

struct Shown {
    message: String,
    /// Header, rule, then body lines.
    lines: Vec<String>,
}

pub struct Model {
    mode: Mode,
    db: Database,
    input: TextField,
    error: Option<String>,
    shown: Option<Shown>,
    top: usize,
    left: usize,
    samples: ListState,
    history: Vec<String>,
    history_list: ListState,
    help: ListState,
    viewport: Viewport,
    dirty: bool,
    notice: Option<String>,
}

impl Default for Model {
    fn default() -> Self { Self::new() }
}

impl Model {
    pub fn new() -> Self {
        Model {
            mode: Mode::Query,
            db: Database::with_samples(),
            input: TextField::with_max(MAX_QUERY_CHARS),
            error: None,
            shown: None,
            top: 0,
            left: 0,
            samples: ListState::default(),
            history: Vec::new(),
            history_list: ListState::default(),
            help: ListState::default(),
            viewport: Viewport::default(),
            dirty: false,
            notice: None,
        }
    }

    pub fn mode(&self) -> Mode { self.mode }

    pub fn input_text(&self) -> &str { self.input.as_str() }

    pub fn database(&self) -> &Database { &self.db }

    // ----- persistence ---------------------------------------------------

    pub fn dirty(&self) -> bool { self.dirty }

    pub fn mark_saved(&mut self) { self.dirty = false; }

    pub fn saved(&self) -> Saved { Saved { version: STATE_VERSION, history: self.history.clone() } }

    /// Replace the remembered queries. Refuses data from a newer version.
    pub fn restore(&mut self, s: Saved) -> Result<(), LoadError> {
        if s.version > STATE_VERSION {
            return Err(LoadError::NewerVersion(s.version));
        }
        let skip = s.history.len().saturating_sub(MAX_HISTORY);
        self.history = s.history.into_iter().skip(skip).map(|q| q.chars().take(MAX_QUERY_CHARS).collect()).collect();
        self.history_list = ListState::default();
        self.dirty = false;
        Ok(())
    }

    /// Drop the remembered queries from memory (the PDDB basis they came from was locked).
    pub fn forget_history(&mut self) {
        self.history.clear();
        self.history_list = ListState::default();
        self.dirty = false;
        if self.mode == Mode::History {
            self.mode = Mode::Query;
        }
    }

    // ----- viewport ------------------------------------------------------

    pub fn set_viewport(&mut self, v: Viewport) {
        self.viewport = Viewport { list_rows: v.list_rows.max(1), text_rows: v.text_rows.max(3), text_cols: v.text_cols.max(8) };
        self.clamp_scroll();
        self.samples.clamp(self.sample_rows().len(), self.viewport.list_rows);
        self.history_list.clamp(self.history.len(), self.viewport.list_rows);
        self.help.clamp(SUPPORTED.len(), self.viewport.list_rows);
    }

    fn body_len(&self) -> usize { self.shown.as_ref().map_or(0, |s| s.lines.len().saturating_sub(2)) }

    fn body_rows(&self) -> usize { self.viewport.text_rows.saturating_sub(2).max(1) }

    fn max_width(&self) -> usize {
        self.shown.as_ref().map_or(0, |s| s.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0))
    }

    fn clamp_scroll(&mut self) {
        self.top = self.top.min(self.body_len().saturating_sub(self.body_rows()));
        self.left = self.left.min(self.max_width().saturating_sub(self.viewport.text_cols));
    }

    // ----- keys ----------------------------------------------------------

    pub fn on_key(&mut self, key: Key) -> Out {
        self.notice = None;
        match self.mode {
            Mode::Query => self.key_query(key),
            Mode::Results => self.key_results(key),
            Mode::Samples => self.key_samples(key),
            Mode::History => self.key_history(key),
            Mode::Help => self.key_help(key),
        }
    }

    fn key_query(&mut self, key: Key) -> Out {
        match key {
            Key::Char(c) => {
                if self.input.insert(c) {
                    self.error = None;
                }
            }
            Key::Left => self.input.left(),
            Key::Right => self.input.right(),
            Key::Up => self.input.home(),
            Key::Down => self.input.end(),
            Key::Backspace => {
                if self.input.is_empty() {
                    return Out::Leave;
                }
                if self.input.backspace() {
                    self.error = None;
                }
            }
            Key::Enter => self.run_input(),
            Key::F(1) => self.mode = Mode::Samples,
            Key::F(2) => {
                self.history_list = ListState::default();
                self.mode = Mode::History;
            }
            Key::F(3) => self.mode = Mode::Help,
            Key::F(4) => return Out::Leave,
            Key::F(_) => {}
        }
        Out::Stay
    }

    fn run_input(&mut self) {
        let sql = self.input.as_str().trim().to_string();
        if sql.is_empty() {
            self.error = Some("Type a query first (F1 has samples, F3 shows what is supported)".into());
            return;
        }
        self.run(&sql);
    }

    fn run(&mut self, sql: &str) {
        match self.db.execute(sql) {
            Ok(out) => {
                let lines = if out.columns.is_empty() { Vec::new() } else { format_table(&out.columns, &out.rows, CELL_MAX) };
                self.shown = Some(Shown { message: out.message, lines });
                self.top = 0;
                self.left = 0;
                self.error = None;
                self.remember(sql);
                self.mode = Mode::Results;
                self.clamp_scroll();
            }
            Err(e) => {
                // Show the error where the query is, so it can be fixed.
                if self.input.as_str().trim() != sql {
                    self.input.set(sql);
                }
                self.error = Some(e);
                self.mode = Mode::Query;
            }
        }
    }

    fn remember(&mut self, sql: &str) {
        if self.history.last().map(String::as_str) == Some(sql) {
            return;
        }
        self.history.push(sql.to_string());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.dirty = true;
    }

    fn key_results(&mut self, key: Key) -> Out {
        match key {
            Key::Up => self.top = self.top.saturating_sub(1),
            Key::Down => self.top += 1,
            Key::Left => self.left = self.left.saturating_sub(HSTEP),
            Key::Right => self.left += HSTEP,
            Key::Enter => {
                self.input.clear();
                self.mode = Mode::Query;
            }
            k if k.is_back() => self.mode = Mode::Query,
            _ => {}
        }
        self.clamp_scroll();
        Out::Stay
    }

    /// Sample queries, then the reset entry.
    fn sample_rows(&self) -> Vec<String> {
        let mut rows: Vec<String> = SAMPLE_QUERIES.iter().map(|s| s.to_string()).collect();
        rows.push("(Reset sample database)".into());
        rows
    }

    fn key_samples(&mut self, key: Key) -> Out {
        let n = self.sample_rows().len();
        let rows = self.viewport.list_rows;
        match key {
            Key::Up => self.samples.step(-1, n, rows),
            Key::Down => self.samples.step(1, n, rows),
            Key::Enter => {
                let i = self.samples.selected;
                match SAMPLE_QUERIES.get(i) {
                    Some(sql) => {
                        self.input.set(sql);
                        self.run(sql);
                    }
                    None => {
                        self.db = Database::with_samples();
                        self.notice = Some("Sample database restored".into());
                        self.mode = Mode::Query;
                    }
                }
            }
            Key::F(1) => {
                // Copy into the editor without running.
                if let Some(sql) = SAMPLE_QUERIES.get(self.samples.selected) {
                    self.input.set(sql);
                    self.error = None;
                    self.mode = Mode::Query;
                }
            }
            k if k.is_back() => self.mode = Mode::Query,
            _ => {}
        }
        Out::Stay
    }

    /// Newest first.
    fn history_rows(&self) -> Vec<String> { self.history.iter().rev().cloned().collect() }

    fn key_history(&mut self, key: Key) -> Out {
        let n = self.history.len();
        let rows = self.viewport.list_rows;
        match key {
            Key::Up => self.history_list.step(-1, n, rows),
            Key::Down => self.history_list.step(1, n, rows),
            Key::Enter => {
                if let Some(sql) = self.history_rows().get(self.history_list.selected) {
                    self.input.set(sql);
                    self.error = None;
                    self.mode = Mode::Query;
                }
            }
            k if k.is_back() => self.mode = Mode::Query,
            _ => {}
        }
        Out::Stay
    }

    fn key_help(&mut self, key: Key) -> Out {
        let rows = self.viewport.list_rows;
        match key {
            Key::Up => self.help.step(-1, SUPPORTED.len(), rows),
            Key::Down => self.help.step(1, SUPPORTED.len(), rows),
            k if k.is_back() || k.is_select() => self.mode = Mode::Query,
            _ => {}
        }
        Out::Stay
    }

    // ----- what to draw --------------------------------------------------

    pub fn title(&self) -> String {
        match self.mode {
            Mode::Query => "SQL Explorer".into(),
            Mode::Results => self.shown.as_ref().map(|s| s.message.clone()).unwrap_or_default(),
            Mode::Samples => "Sample queries".into(),
            Mode::History => "History".into(),
            Mode::Help => "Supported SQL".into(),
        }
    }

    pub fn hint(&self) -> &'static str {
        match self.mode {
            Mode::Query => "⏎ run · F1 samples · F2 history · F3 help · F4 exit",
            Mode::Results => "↑↓←→ scroll · ⏎ new · ⌫ edit",
            Mode::Samples => "↑↓ move · ⏎ run · F1 edit · ⌫ back",
            Mode::History => "↑↓ move · ⏎ edit · ⌫ back",
            Mode::Help => "↑↓ scroll · ⌫ back",
        }
    }

    /// Rows for the list screens.
    pub fn list_items(&self) -> Vec<String> {
        match self.mode {
            Mode::Samples => self.sample_rows(),
            Mode::History => self.history_rows(),
            Mode::Help => SUPPORTED.iter().map(|s| s.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn list_state(&self) -> &ListState {
        match self.mode {
            Mode::Samples => &self.samples,
            Mode::History => &self.history_list,
            _ => &self.help,
        }
    }

    /// Message for an empty history list.
    pub fn empty_note(&self) -> Option<&'static str> {
        (self.mode == Mode::History && self.history.is_empty()).then_some("No queries yet.")
    }

    /// Error under the input, or a one-off notice.
    pub fn error(&self) -> Option<&str> { self.error.as_deref().or(self.notice.as_deref()) }

    /// One line under the lists: the tables, or the selected sample in full.
    pub fn status(&self) -> String {
        match self.mode {
            Mode::Query => format!("Tables: {}", self.db.table_names().join(", ")),
            Mode::Samples => String::new(),
            _ => String::new(),
        }
    }

    /// The input as hard-wrapped lines of `cols` chars with a `|` cursor,
    /// windowed to `rows` lines so the cursor is always visible.
    pub fn input_view(&self, cols: usize, rows: usize) -> Vec<String> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let text = self.input.display_with_cursor('|');
        let chars: Vec<char> = text.chars().collect();
        let lines: Vec<String> = chars.chunks(cols).map(|c| c.iter().collect()).collect();
        let cursor_line = self.input.cursor() / cols;
        let first = (cursor_line + 1).saturating_sub(rows);
        lines.into_iter().skip(first).take(rows).collect()
    }

    /// Table lines for the Results screen: header and rule pinned, body
    /// scrolled, each cut to the window `cols` wide. Empty for statements that
    /// return no table (the title carries the message).
    pub fn result_window(&self) -> Vec<String> {
        let Some(s) = &self.shown else { return Vec::new() };
        let cols = self.viewport.text_cols;
        let cut = |l: &String| -> String { l.chars().skip(self.left).take(cols).collect() };
        let mut out: Vec<String> = s.lines.iter().take(2).map(cut).collect();
        out.extend(s.lines.iter().skip(2 + self.top).take(self.body_rows()).map(cut));
        out
    }

    /// "Rows 3-9 of 20" style position for the footer area, or empty.
    pub fn result_position(&self) -> String {
        let n = self.body_len();
        if n == 0 {
            return String::new();
        }
        let last = (self.top + self.body_rows()).min(n);
        let mut s = format!("Rows {}-{} of {}", self.top + 1, last, n);
        if self.left > 0 || self.max_width() > self.viewport.text_cols {
            s.push_str(" ←→");
        }
        s
    }

    /// True when the last statement returned no table.
    pub fn result_is_message_only(&self) -> bool { self.shown.as_ref().is_some_and(|s| s.lines.is_empty()) }
}

#[cfg(test)]
mod tests;
