use super::*;

fn type_str(m: &mut Model, s: &str) {
    for c in s.chars() {
        m.on_key(Key::Char(c));
    }
}

fn query(sql: &str) -> Model {
    let mut m = Model::new();
    type_str(&mut m, sql);
    m.on_key(Key::Enter);
    m
}

#[test]
fn run_a_query_and_read_the_table() {
    let m = query("SELECT name, age FROM users WHERE age > 28");
    assert_eq!(m.mode(), Mode::Results);
    assert_eq!(m.title(), "3 rows");
    assert_eq!(m.result_window(), vec!["name  | age", "------+----", "Alice | 30", "Carol | 35", "Eve   | 32"]);
    assert_eq!(m.result_position(), "Rows 1-3 of 3");
    assert!(!m.result_is_message_only());
}

#[test]
fn statements_without_a_table_show_only_a_message() {
    let m = query("DELETE FROM users WHERE id = 5");
    assert_eq!(m.mode(), Mode::Results);
    assert_eq!(m.title(), "1 row deleted");
    assert!(m.result_is_message_only());
    assert!(m.result_window().is_empty());
}

#[test]
fn errors_stay_on_the_query_screen_with_the_text_kept() {
    let mut m = query("SELECT * FROM nope");
    assert_eq!(m.mode(), Mode::Query);
    assert_eq!(m.error(), Some("no table 'nope'"));
    assert_eq!(m.input_text(), "SELECT * FROM nope");
    assert!(m.history.is_empty(), "failed queries are not remembered");
    m.on_key(Key::Char(' '));
    assert_eq!(m.error(), None, "editing clears the error");
    let mut m = Model::new();
    m.on_key(Key::Enter);
    assert!(m.error().unwrap().contains("Type a query"));
}

#[test]
fn every_sample_runs_from_the_samples_screen() {
    for (i, sample) in SAMPLE_QUERIES.iter().enumerate() {
        let mut m = Model::new();
        m.on_key(Key::F(1));
        assert_eq!(m.mode(), Mode::Samples);
        for _ in 0..i {
            m.on_key(Key::Down);
        }
        m.on_key(Key::Enter);
        assert_eq!(m.mode(), Mode::Results, "sample {i}: {:?}", m.error());
        assert_eq!(m.input_text(), *sample);
    }
}

#[test]
fn samples_edit_copies_without_running_and_reset_restores_the_database() {
    let mut m = Model::new();
    m.on_key(Key::F(1));
    m.on_key(Key::Down);
    m.on_key(Key::F(1));
    assert_eq!(m.mode(), Mode::Query);
    assert_eq!(m.input_text(), SAMPLE_QUERIES[1]);
    assert!(m.history.is_empty());
    // Delete everything, then reset from the samples screen.
    m.on_key(Key::F(1)); // samples
    let n = SAMPLE_QUERIES.len();
    for _ in 0..n {
        m.on_key(Key::Down);
    }
    // First wreck the data.
    let mut m2 = query("DELETE FROM users");
    assert_eq!(m2.database().row_count("users"), Some(0));
    m2.on_key(Key::Backspace); // edit
    m2.on_key(Key::F(1));
    for _ in 0..n {
        m2.on_key(Key::Down);
    }
    m2.on_key(Key::Enter); // reset entry
    assert_eq!(m2.mode(), Mode::Query);
    assert_eq!(m2.database().row_count("users"), Some(5));
    assert_eq!(m2.error(), Some("Sample database restored"));
    m2.on_key(Key::Char('x'));
    assert_eq!(m2.error(), None);
    let _ = m;
}

#[test]
fn state_persists_across_queries_within_a_session() {
    let mut m = query("CREATE TABLE t (a INTEGER, b TEXT)");
    m.on_key(Key::Enter); // new query
    assert_eq!(m.mode(), Mode::Query);
    assert_eq!(m.input_text(), "");
    type_str(&mut m, "INSERT INTO t VALUES (1, 'x'), (2, 'y')");
    m.on_key(Key::Enter);
    m.on_key(Key::Enter);
    type_str(&mut m, "SELECT * FROM t ORDER BY a DESC");
    m.on_key(Key::Enter);
    assert_eq!(m.result_window()[2..], ["2 | y", "1 | x"]);
}

#[test]
fn results_scroll_both_ways_with_pinned_header() {
    let mut m = Model::new();
    m.set_viewport(Viewport { list_rows: 8, text_rows: 6, text_cols: 20 });
    type_str(&mut m, "CREATE TABLE wide (aaaaaaaaaaaa, bbbbbbbbbbbb, cccccccccccc)");
    m.on_key(Key::Enter);
    m.on_key(Key::Enter);
    let vals: Vec<String> = (0..30).map(|i| format!("({i}, 'bbbbbbbbbbbb{i}', 'c')")).collect();
    // The input holds 300 characters, so insert in batches.
    for chunk in vals.chunks(6) {
        type_str(&mut m, &format!("INSERT INTO wide VALUES {}", chunk.join(",")));
        m.on_key(Key::Enter);
        assert_eq!(m.mode(), Mode::Results, "{:?}", m.error());
        m.on_key(Key::Enter);
    }
    type_str(&mut m, "SELECT * FROM wide");
    m.on_key(Key::Enter);
    let w = m.result_window();
    assert_eq!(w.len(), 2 + 4);
    assert!(w[0].starts_with("aaaaaaaaaaaa"));
    assert_eq!(w[2], "0            | bbbbb");
    assert!(w.iter().all(|l| l.chars().count() <= 20));
    for _ in 0..100 {
        m.on_key(Key::Down);
    }
    let w = m.result_window();
    assert!(w[0].starts_with("aaaaaaaaaaaa"), "header stays");
    assert!(w.last().unwrap().starts_with("29"), "{w:?}");
    assert_eq!(m.result_position(), "Rows 27-30 of 30 ←→");
    for _ in 0..100 {
        m.on_key(Key::Up);
    }
    assert!(m.result_window()[2].starts_with('0'));
    for _ in 0..100 {
        m.on_key(Key::Right);
    }
    let w = m.result_window();
    assert!(!w[0].starts_with("aaaaaaaaaaaa"));
    assert!(w.iter().all(|l| l.chars().count() <= 20));
    let before = m.left;
    m.on_key(Key::Right);
    assert_eq!(m.left, before, "can't scroll past the end");
    for _ in 0..100 {
        m.on_key(Key::Left);
    }
    assert_eq!(m.left, 0);
}

#[test]
fn history_is_newest_first_deduplicated_and_capped() {
    let mut m = Model::new();
    for i in 0..(MAX_HISTORY + 5) {
        type_str(&mut m, &format!("SELECT id FROM users WHERE id = {i}"));
        m.on_key(Key::Enter);
        m.on_key(Key::Enter);
    }
    assert_eq!(m.history.len(), MAX_HISTORY);
    type_str(&mut m, &format!("SELECT id FROM users WHERE id = {}", MAX_HISTORY + 4));
    m.on_key(Key::Enter);
    assert_eq!(m.history.len(), MAX_HISTORY, "same query twice in a row is kept once");
    m.on_key(Key::Backspace);
    m.on_key(Key::F(2));
    assert_eq!(m.mode(), Mode::History);
    assert_eq!(m.list_items()[0], format!("SELECT id FROM users WHERE id = {}", MAX_HISTORY + 4));
    m.on_key(Key::Down);
    m.on_key(Key::Enter);
    assert_eq!(m.mode(), Mode::Query);
    assert_eq!(m.input_text(), format!("SELECT id FROM users WHERE id = {}", MAX_HISTORY + 3));
}

#[test]
fn empty_history_has_a_message() {
    let mut m = Model::new();
    m.on_key(Key::F(2));
    assert_eq!(m.empty_note(), Some("No queries yet."));
    m.on_key(Key::Enter); // nothing to pick
    assert_eq!(m.mode(), Mode::History);
    m.on_key(Key::Backspace);
    assert_eq!(m.mode(), Mode::Query);
}

#[test]
fn help_lists_the_supported_subset() {
    let mut m = Model::new();
    m.on_key(Key::F(3));
    assert_eq!(m.mode(), Mode::Help);
    assert_eq!(m.list_items().len(), SUPPORTED.len());
    assert!(m.list_items().iter().any(|l| l.contains("no JOIN")));
    m.on_key(Key::Enter);
    assert_eq!(m.mode(), Mode::Query);
}

#[test]
fn every_screen_exits_with_backspace_or_f4() {
    let mut m = Model::new();
    assert_eq!(m.on_key(Key::Backspace), Out::Leave); // empty input
    let mut m = Model::new();
    type_str(&mut m, "x");
    assert_eq!(m.on_key(Key::F(4)), Out::Leave);
    for back in [Key::Backspace, Key::F(4)] {
        for open in [Key::F(1), Key::F(2), Key::F(3)] {
            let mut m = Model::new();
            m.on_key(open);
            assert_ne!(m.mode(), Mode::Query);
            assert_eq!(m.on_key(back), Out::Stay);
            assert_eq!(m.mode(), Mode::Query);
        }
        let mut m = query("SHOW TABLES");
        assert_eq!(m.mode(), Mode::Results);
        assert_eq!(m.on_key(back), Out::Stay);
        assert_eq!(m.mode(), Mode::Query);
        assert_eq!(m.input_text(), "SHOW TABLES", "back keeps the query for editing");
    }
}

#[test]
fn input_editing_and_limits() {
    let mut m = Model::new();
    type_str(&mut m, &"x".repeat(MAX_QUERY_CHARS + 50));
    assert_eq!(m.input_text().chars().count(), MAX_QUERY_CHARS);
    for _ in 0..MAX_QUERY_CHARS {
        m.on_key(Key::Backspace);
    }
    assert_eq!(m.input_text(), "");
    type_str(&mut m, "SELECT '\u{e9}\u{4e2d}'");
    m.on_key(Key::Left);
    m.on_key(Key::Left);
    m.on_key(Key::Backspace);
    assert_eq!(m.input_text(), "SELECT '\u{4e2d}'");
    let v = m.input_view(8, 2);
    assert!(v.len() <= 2 && v.iter().any(|l| l.contains('|')));
}

#[test]
fn history_persists_and_the_database_does_not() {
    let mut m = query("SELECT * FROM users");
    m.on_key(Key::Enter);
    assert!(m.dirty());
    let s = m.saved();
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"version":1,"history":["SELECT * FROM users"]}"#);
    m.mark_saved();
    assert!(!m.dirty());
    let mut m2 = Model::new();
    m2.restore(s).unwrap();
    assert_eq!(m2.history, vec!["SELECT * FROM users"]);
    assert!(!m2.dirty());
    assert_eq!(m2.restore(Saved { version: 2, history: vec![] }), Err(LoadError::NewerVersion(2)));
    assert_eq!(m2.history.len(), 1);
    // Oversized saved history is trimmed.
    let big = Saved { version: 1, history: (0..100).map(|i| format!("q{i}")).collect() };
    m2.restore(big).unwrap();
    assert_eq!(m2.history.len(), MAX_HISTORY);
    assert_eq!(m2.history.last().unwrap(), "q99");
    m2.forget_history();
    assert!(m2.history.is_empty());
    // Browsing and failing queries leave nothing to save.
    let mut m3 = Model::new();
    m3.on_key(Key::F(1));
    m3.on_key(Key::Backspace);
    type_str(&mut m3, "bogus");
    m3.on_key(Key::Enter);
    assert!(!m3.dirty());
}

#[test]
fn status_lists_tables_including_new_ones() {
    let mut m = Model::new();
    assert_eq!(m.status(), "Tables: users, products, orders");
    type_str(&mut m, "CREATE TABLE zed (a)");
    m.on_key(Key::Enter);
    m.on_key(Key::Enter);
    assert!(m.status().ends_with("orders, zed"));
}
