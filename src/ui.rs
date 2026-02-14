//! UI rendering for SQLite Client.

extern crate alloc;
use alloc::format;
use alloc::string::String;

use gam::{Gam, Point, Line, TextView, TextBounds, GlyphStyle};
use crate::app::{SqlApp, AppState};
use crate::sql::SAMPLE_QUERIES;

const SCREEN_W: i16 = 336;
const HEADER_H: i16 = 30;
const FOOTER_Y: i16 = 490;

fn draw_header(gam: &Gam, title: &str) {
    gam.draw_rectangle(Point::new(0, 0), Point::new(SCREEN_W, HEADER_H)).ok();
    let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(8, 2), Point::new(SCREEN_W - 8, HEADER_H)));
    tv.style = GlyphStyle::Bold; tv.invert = true;
    tv.text = String::from(title);
    gam.post_textview(&mut tv).ok();
}

fn draw_footer(gam: &Gam, text: &str) {
    gam.draw_line(Line::new(Point::new(0, FOOTER_Y), Point::new(SCREEN_W, FOOTER_Y))).ok();
    let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(8, FOOTER_Y + 4), Point::new(SCREEN_W - 8, FOOTER_Y + 40)));
    tv.style = GlyphStyle::Small;
    tv.text = String::from(text);
    gam.post_textview(&mut tv).ok();
}

fn text_at(gam: &Gam, x: i16, y: i16, h: i16, s: &str, style: GlyphStyle) {
    let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(x, y), Point::new(SCREEN_W - 8, y + h)));
    tv.style = style; tv.text = String::from(s);
    gam.post_textview(&mut tv).ok();
}

pub fn draw(gam: &Gam, app: &SqlApp) {
    gam.clear().ok();
    match app.state {
        AppState::QueryInput => draw_input(gam, app),
        AppState::Results => draw_results(gam, app),
        AppState::Samples => draw_samples(gam, app),
        AppState::History => draw_history(gam, app),
    }
    gam.redraw().ok();
}

fn draw_input(gam: &Gam, app: &SqlApp) {
    draw_header(gam, "SQL Explorer");

    let mut y: i16 = 48;
    text_at(gam, 12, y, 22, "Enter SQL query:", GlyphStyle::Regular);
    y += 28;

    // Input area
    gam.draw_line(Line::new(Point::new(8, y - 2), Point::new(SCREEN_W - 8, y - 2))).ok();
    let display = if app.input.is_empty() { String::from("sql> _") } else { format!("sql> {}|", app.input) };
    let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(8, y), Point::new(SCREEN_W - 8, y + 60)));
    tv.style = GlyphStyle::Small;
    tv.text = display;
    gam.post_textview(&mut tv).ok();
    y += 70;

    // Database info
    text_at(gam, 12, y, 18, "Tables:", GlyphStyle::Small);
    y += 18;
    for table in &app.db.tables {
        let info = format!("  {} ({} cols, {} rows)", table.name, table.columns.len(), table.rows.len());
        text_at(gam, 12, y, 16, &info, GlyphStyle::Small);
        y += 16;
    }
    y += 10;
    text_at(gam, 12, y, 16, "Right: sample queries", GlyphStyle::Small);
    y += 16;
    text_at(gam, 12, y, 16, "Up: query history", GlyphStyle::Small);

    draw_footer(gam, "Enter: execute  Right: samples  Up: history  Menu: quit");
}

fn draw_results(gam: &Gam, app: &SqlApp) {
    draw_header(gam, "Query Results");

    if let Some(ref result) = app.result {
        let mut y: i16 = 36;

        // Status message
        text_at(gam, 8, y, 18, &result.message, GlyphStyle::Small);
        y += 22;

        if !result.columns.is_empty() {
            // Column headers
            let header_line = result.columns.iter().map(|c| {
                let mut s = c.clone();
                while s.len() < 12 { s.push(' '); }
                s.truncate(12);
                s
            }).collect::<alloc::vec::Vec<_>>().join(" ");

            let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(8, y), Point::new(SCREEN_W - 4, y + 18)));
            tv.style = GlyphStyle::Bold;
            tv.text = header_line;
            gam.post_textview(&mut tv).ok();
            y += 18;

            gam.draw_line(Line::new(Point::new(8, y), Point::new(SCREEN_W - 8, y))).ok();
            y += 4;

            // Data rows
            let visible = 20;
            let end = (app.result_scroll + visible).min(result.rows.len());
            for ri in app.result_scroll..end {
                let row = &result.rows[ri];
                let row_line = row.iter().map(|v| {
                    let mut s = v.clone();
                    while s.len() < 12 { s.push(' '); }
                    s.truncate(12);
                    s
                }).collect::<alloc::vec::Vec<_>>().join(" ");

                text_at(gam, 8, y, 18, &row_line, GlyphStyle::Small);
                y += 18;
                if y > FOOTER_Y - 20 { break; }
            }

            if result.rows.len() > visible {
                let ind = format!("{}-{}/{}", app.result_scroll + 1, end, result.rows.len());
                text_at(gam, SCREEN_W - 80, FOOTER_Y - 16, 16, &ind, GlyphStyle::Small);
            }
        }
    }

    draw_footer(gam, "Up/Down: scroll  Enter: new query  Left: back");
}

fn draw_samples(gam: &Gam, app: &SqlApp) {
    draw_header(gam, "Sample Queries");

    let start_y: i16 = 38;
    let line_h: i16 = 42;

    for (i, &query) in SAMPLE_QUERIES.iter().enumerate() {
        let y = start_y + (i as i16) * line_h;
        if y > FOOTER_Y - line_h { break; }
        let selected = i == app.sample_cursor;

        if selected {
            gam.draw_rectangle(Point::new(4, y - 2), Point::new(SCREEN_W - 4, y + line_h - 6)).ok();
        }

        let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(12, y), Point::new(SCREEN_W - 12, y + line_h - 8)));
        tv.style = GlyphStyle::Small;
        if selected { tv.invert = true; }
        tv.text = String::from(query);
        gam.post_textview(&mut tv).ok();
    }

    draw_footer(gam, "Up/Down: browse  Enter: execute  Left: back");
}

fn draw_history(gam: &Gam, app: &SqlApp) {
    draw_header(gam, "Query History");

    let start_y: i16 = 38;
    let line_h: i16 = 36;
    let visible = 10;

    if app.history.is_empty() {
        text_at(gam, 12, start_y + 20, 22, "No queries yet.", GlyphStyle::Regular);
    } else {
        let end = (app.history_scroll + visible).min(app.history.len());
        for vi in app.history_scroll..end {
            let row = vi - app.history_scroll;
            let y = start_y + (row as i16) * line_h;
            let selected = vi == app.history_cursor;

            if selected {
                gam.draw_rectangle(Point::new(4, y - 2), Point::new(SCREEN_W - 4, y + line_h - 6)).ok();
            }

            let mut tv = TextView::new(TextBounds::BoundingBox(Point::new(12, y), Point::new(SCREEN_W - 12, y + line_h - 8)));
            tv.style = GlyphStyle::Small;
            if selected { tv.invert = true; }
            tv.text = app.history[vi].clone();
            gam.post_textview(&mut tv).ok();
        }
    }

    draw_footer(gam, "Up/Down: browse  Enter: re-run  Left: back");
}
