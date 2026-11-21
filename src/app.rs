//! Rendering and persistence for SQL Explorer. The SQL engine and the screen
//! state machine are in `sql-explorer-core` (host-tested); this file maps device
//! state to those, saves the query history, and draws.
//!
//! The tables live in memory for the session only. Only the history is stored.

use gam::GlyphStyle;
use precursor_appkit::keys::Key;
use precursor_appkit::layout::rows_that_fit;
use precursor_appkit::screen::{Screen, MARGIN};
use precursor_appkit::store::Store;
use sql_explorer_core::{LoadError, Mode, Model, Out, Saved, Viewport};

pub const DICT: &str = "sql-explorer";
const HISTORY_KEY: &str = "history";

/// Average glyph widths assumed when turning pixels into characters per line.
/// Deliberately wide so a line of capitals still fits.
const GLYPH_W: isize = 8;
const MONO_W: isize = 10;

#[derive(PartialEq, Eq)]
pub enum Action {
    Stay,
    Leave,
}

pub struct App {
    screen: Screen,
    store: Store,
    model: Model,
    /// Set when saved data couldn't be used; blocks saves so it isn't overwritten.
    read_only: bool,
    notice: String,
}

impl App {
    pub fn new(screen: Screen, store: Store) -> Self {
        let mut app = Self { screen, store, model: Model::new(), read_only: false, notice: String::new() };
        app.reload();
        app
    }

    pub fn screen(&self) -> &Screen { &self.screen }

    /// (Re)load the saved query history. Called at start and whenever the set
    /// of open PDDB bases changes: history from a just-locked basis leaves
    /// memory. The in-memory tables are not touched.
    pub fn reload(&mut self) {
        self.read_only = false;
        self.notice.clear();
        self.model.forget_history();
        match self.store.load::<Saved>(HISTORY_KEY) {
            Ok(Some(saved)) => {
                if let Err(LoadError::NewerVersion(v)) = self.model.restore(saved) {
                    log::warn!("saved history version {} is newer than this app", v);
                    self.read_only = true;
                    self.notice = "Saved data is from a newer version".into();
                }
            }
            Ok(None) => {}
            Err(e) => {
                log::error!("couldn't load saved history: {:?}", e);
                self.read_only = true;
                self.notice = "Couldn't read saved data".into();
            }
        }
        self.draw();
    }

    fn save(&mut self) {
        if !self.model.dirty() || self.read_only {
            return;
        }
        match self.store.save(HISTORY_KEY, &self.model.saved()) {
            Ok(()) => {
                self.model.mark_saved();
                self.notice.clear();
            }
            Err(e) => {
                log::error!("save failed: {:?}", e);
                self.notice = "Save failed".into();
            }
        }
    }

    pub fn on_key(&mut self, key: Key) -> Action {
        if self.model.on_key(key) == Out::Leave {
            self.save();
            return Action::Leave;
        }
        Action::Stay
    }

    pub fn on_foreground(&mut self) {
        self.screen.refresh_size();
        self.draw();
    }

    /// Save only if the history changed.
    pub fn on_background(&mut self) { self.save(); }

    pub fn draw(&mut self) {
        let s = &self.screen;
        s.clear();
        let top = s.header(&self.model.title());
        let bottom = s.footer(self.model.hint());

        let small_h = s.line_height(GlyphStyle::Small);
        let mono_h = s.line_height(GlyphStyle::Monospace);
        let row_h = s.line_height(GlyphStyle::Regular) + 4;
        let notice_h = if self.notice.is_empty() { 0 } else { small_h };
        let content_bottom = bottom - notice_h;
        let status_h = small_h;
        let avail = content_bottom - top;
        let width = s.size().x - 2 * MARGIN;
        let cols = (width / GLYPH_W).max(8) as usize;
        self.model.set_viewport(Viewport {
            list_rows: rows_that_fit(avail - status_h, row_h),
            text_rows: rows_that_fit(avail - small_h, mono_h),
            text_cols: (width / MONO_W).max(8) as usize,
        });

        match self.model.mode() {
            Mode::Query => {
                let mut y = top;
                y += s.text(y, GlyphStyle::Small, "SQL (F3 = what is supported):");
                let err_h = if self.model.error().is_some() { small_h * 3 } else { 0 };
                let rows = rows_that_fit(content_bottom - y - err_h - status_h, small_h);
                for line in self.model.input_view(cols, rows) {
                    s.text(y, GlyphStyle::Small, &line);
                    y += small_h;
                }
                if let Some(e) = self.model.error() {
                    s.para(content_bottom - err_h - status_h, content_bottom - status_h, GlyphStyle::Small, e);
                }
                let tables: String = self.model.status().chars().take(cols).collect();
                s.text(content_bottom - status_h, GlyphStyle::Small, &tables);
            }
            Mode::Results => {
                let mut y = top;
                for line in self.model.result_window() {
                    s.text_in(
                        gam::menu::Point::new(MARGIN, y),
                        gam::menu::Point::new(s.size().x - MARGIN, y + mono_h),
                        GlyphStyle::Monospace,
                        &line,
                    );
                    y += mono_h;
                }
                if self.model.result_is_message_only() {
                    s.text(top, GlyphStyle::Regular, "Done.");
                }
                s.text(content_bottom - small_h, GlyphStyle::Small, &self.model.result_position());
            }
            Mode::Samples | Mode::History | Mode::Help => {
                let items = self.model.list_items();
                s.list(top, content_bottom - status_h, &items, self.model.list_state());
                if let Some(note) = self.model.empty_note() {
                    s.text(top, GlyphStyle::Regular, note);
                }
            }
        }
        if !self.notice.is_empty() {
            s.text(content_bottom, GlyphStyle::Small, &self.notice);
        }
        s.flush();
    }
}
