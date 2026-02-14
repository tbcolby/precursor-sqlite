//! State machine for SQLite Client.
//! States: QueryInput, Results, Samples, History

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use crate::sql::{Database, QueryResult, SAMPLE_QUERIES};

const KEY_UP: char = '\u{F700}';
const KEY_DOWN: char = '\u{F701}';
const KEY_LEFT: char = '\u{F702}';
const KEY_RIGHT: char = '\u{F703}';
const KEY_ENTER: char = '\u{000D}';
const KEY_BACKSPACE: char = '\u{0008}';
const KEY_MENU: char = '\u{2234}';

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState { QueryInput, Results, Samples, History }

pub struct SqlApp {
    pub state: AppState,
    pub needs_redraw: bool,
    pub db: Database,
    pub input: String,
    pub result: Option<QueryResult>,
    pub result_scroll: usize,
    pub sample_cursor: usize,
    pub history: Vec<String>,
    pub history_cursor: usize,
    pub history_scroll: usize,
}

impl SqlApp {
    pub fn new() -> Self {
        Self {
            state: AppState::QueryInput,
            needs_redraw: true,
            db: Database::with_samples(),
            input: String::new(),
            result: None,
            result_scroll: 0,
            sample_cursor: 0,
            history: Vec::new(),
            history_cursor: 0,
            history_scroll: 0,
        }
    }

    fn execute_query(&mut self) {
        if self.input.is_empty() { return; }
        let result = self.db.execute(&self.input);
        self.history.push(self.input.clone());
        self.result = Some(result);
        self.result_scroll = 0;
        self.state = AppState::Results;
    }

    pub fn handle_key(&mut self, key: char) -> bool {
        self.needs_redraw = true;
        match self.state {
            AppState::QueryInput => self.key_input(key),
            AppState::Results => self.key_results(key),
            AppState::Samples => self.key_samples(key),
            AppState::History => self.key_history(key),
        }
    }

    fn key_input(&mut self, key: char) -> bool {
        match key {
            KEY_MENU => return false,
            KEY_ENTER => { self.execute_query(); }
            KEY_BACKSPACE => { self.input.pop(); }
            KEY_RIGHT => { self.sample_cursor = 0; self.state = AppState::Samples; }
            KEY_UP => {
                if !self.history.is_empty() {
                    self.history_cursor = self.history.len() - 1;
                    self.history_scroll = if self.history.len() > 10 { self.history.len() - 10 } else { 0 };
                    self.state = AppState::History;
                }
            }
            c if c >= ' ' && c <= '~' => {
                if self.input.len() < 120 { self.input.push(c); }
            }
            _ => {}
        }
        true
    }

    fn key_results(&mut self, key: char) -> bool {
        match key {
            KEY_MENU | KEY_LEFT => { self.state = AppState::QueryInput; }
            KEY_DOWN => { self.result_scroll += 1; }
            KEY_UP => { if self.result_scroll > 0 { self.result_scroll -= 1; } }
            KEY_ENTER => {
                // Run another query
                self.input = String::new();
                self.state = AppState::QueryInput;
            }
            _ => {}
        }
        true
    }

    fn key_samples(&mut self, key: char) -> bool {
        match key {
            KEY_MENU | KEY_LEFT => { self.state = AppState::QueryInput; }
            KEY_UP => { if self.sample_cursor > 0 { self.sample_cursor -= 1; } }
            KEY_DOWN => { if self.sample_cursor < SAMPLE_QUERIES.len() - 1 { self.sample_cursor += 1; } }
            KEY_ENTER => {
                self.input = String::from(SAMPLE_QUERIES[self.sample_cursor]);
                self.execute_query();
            }
            _ => {}
        }
        true
    }

    fn key_history(&mut self, key: char) -> bool {
        match key {
            KEY_MENU | KEY_LEFT => { self.state = AppState::QueryInput; }
            KEY_UP => {
                if self.history_cursor > 0 {
                    self.history_cursor -= 1;
                    if self.history_cursor < self.history_scroll { self.history_scroll = self.history_cursor; }
                }
            }
            KEY_DOWN => {
                if self.history_cursor < self.history.len() - 1 {
                    self.history_cursor += 1;
                    if self.history_cursor >= self.history_scroll + 10 { self.history_scroll = self.history_cursor - 9; }
                }
            }
            KEY_ENTER => {
                self.input = self.history[self.history_cursor].clone();
                self.execute_query();
            }
            _ => {}
        }
        true
    }
}
