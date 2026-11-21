use super::*;

fn db() -> Database { Database::with_samples() }

fn run(db: &mut Database, sql: &str) -> Outcome { db.execute(sql).unwrap_or_else(|e| panic!("{sql:?} should run: {e}")) }

fn fail(db: &mut Database, sql: &str) -> String {
    match db.execute(sql) {
        Ok(o) => panic!("{sql:?} should fail, got {o:?}"),
        Err(e) => e,
    }
}

fn q(sql: &str) -> Vec<Vec<String>> { run(&mut db(), sql).rows }

fn col(sql: &str) -> Vec<String> { q(sql).into_iter().map(|r| r.into_iter().next().unwrap_or_default()).collect() }

// ----- audit regressions -----------------------------------------------------

#[test]
fn multi_column_insert_works() {
    // Old engine: the sample INSERT failed with "Expected 4 values, got 1".
    let mut d = db();
    let o = run(&mut d, "INSERT INTO users VALUES (6, 'Frank', 'frank@example.com', 29)");
    assert_eq!(o.message, "1 row inserted into users");
    let rows = run(&mut d, "SELECT * FROM users WHERE id = 6").rows;
    assert_eq!(rows, vec![vec!["6", "Frank", "frank@example.com", "29"]]);
}

#[test]
fn multi_column_create_works() {
    // Old engine: CREATE TABLE notes (id, title, body) made a one-column table.
    let mut d = db();
    run(&mut d, "CREATE TABLE notes (id, title, body)");
    let o = run(&mut d, "DESCRIBE notes");
    let names: Vec<&str> = o.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["id", "title", "body"]);
    run(&mut d, "INSERT INTO notes VALUES (1, 'hello', 'world')");
    assert_eq!(run(&mut d, "SELECT * FROM notes").rows, vec![vec!["1", "hello", "world"]]);
}

#[test]
fn create_with_empty_name_is_an_error_not_a_panic() {
    // Old engine: CREATE TABLE t '' panicked on parts[0].
    let mut d = db();
    assert!(fail(&mut d, "CREATE TABLE t ''").contains("expected '('"));
    assert!(fail(&mut d, "CREATE TABLE t (\"\")").contains("empty column name"));
    assert!(fail(&mut d, "CREATE TABLE t ()").contains("expected a column name"));
    assert!(fail(&mut d, "CREATE TABLE t (,)").contains("expected a column name"));
    assert!(fail(&mut d, "CREATE TABLE").contains("expected a table name"));
}

#[test]
fn decimal_comparisons_are_numeric() {
    // Old engine: `price < 5` compared as strings and returned 24.99, 14.99, 4.99.
    assert_eq!(col("SELECT name FROM products WHERE price < 5"), vec!["Doohickey"]);
    assert_eq!(col("SELECT name FROM products WHERE price < 10"), vec!["Widget", "Doohickey"]);
    assert_eq!(col("SELECT name FROM products WHERE price >= 14.99 ORDER BY price"), vec!["Thingamajig", "Gadget"]);
    assert_eq!(col("SELECT name FROM products WHERE price = 9.990"), vec!["Widget"]);
    assert_eq!(col("SELECT name FROM products WHERE price > 9.99 AND price < 24.99"), vec!["Thingamajig"]);
    assert_eq!(col("SELECT name FROM products WHERE price <> 9.99 AND stock > 60 ORDER BY id"), vec!["Doohickey", "Thingamajig"]);
}

#[test]
fn trailing_clauses_are_applied_or_rejected_never_ignored() {
    // Old engine ignored everything after the first WHERE condition.
    assert_eq!(col("SELECT name FROM users WHERE age > 28 AND name = 'Alice'"), vec!["Alice"]);
    assert_eq!(col("SELECT name FROM users ORDER BY age LIMIT 1"), vec!["Bob"]);
    assert_eq!(col("SELECT name FROM users ORDER BY age DESC LIMIT 2"), vec!["Carol", "Eve"]);
    let mut d = db();
    for (sql, word) in [
        ("SELECT name FROM users GROUP BY name", "GROUP"),
        ("SELECT name FROM users WHERE age > 1 GROUP BY name", "GROUP"),
        ("SELECT * FROM users JOIN orders ON users.id = orders.user_id", "JOIN"),
        ("SELECT * FROM users HAVING age > 1", "HAVING"),
        ("SELECT DISTINCT name FROM users", "DISTINCT"),
        ("SELECT name AS n FROM users", "AS"),
        ("SELECT * FROM users LIMIT 1 OFFSET 1", "OFFSET"),
        ("SELECT * FROM users UNION SELECT * FROM users", "UNION"),
        ("SELECT * FROM users WHERE id IN (1,2)", "IN"),
        ("SELECT * FROM users WHERE age BETWEEN 1 AND 2", "BETWEEN"),
        ("SELECT * FROM users LEFT JOIN orders", "LEFT"),
        ("ALTER TABLE users ADD x", "ALTER"),
        ("CREATE TABLE t (id INTEGER PRIMARY KEY)", "PRIMARY"),
    ] {
        let e = fail(&mut d, sql);
        assert!(e.contains(word) && e.contains("not supported"), "{sql}: {e}");
    }
}

#[test]
fn operators_need_no_spaces_and_keywords_in_strings_are_data() {
    // Old tokenizer: `age>28` was "Incomplete WHERE clause"; 'from' could be read as a keyword.
    assert_eq!(col("SELECT name FROM users WHERE age>=32 ORDER BY id"), vec!["Carol", "Eve"]);
    assert_eq!(col("SELECT name FROM users WHERE age<>30 AND age!=25 AND id<4"), vec!["Carol"]);
    let mut d = db();
    run(&mut d, "CREATE TABLE words (w TEXT)");
    run(&mut d, "INSERT INTO words VALUES ('from'), ('where'), ('select * from x'), ('it''s')");
    assert_eq!(run(&mut d, "SELECT w FROM words WHERE w = 'from'").rows, vec![vec!["from"]]);
    assert_eq!(run(&mut d, "SELECT w FROM words WHERE w = 'where'").rows, vec![vec!["where"]]);
    assert_eq!(run(&mut d, "SELECT w FROM words WHERE w = 'it''s'").rows, vec![vec!["it's"]]);
    assert_eq!(run(&mut d, "SELECT COUNT(*) FROM words").rows, vec![vec!["4"]]);
}

// ----- the built-in samples ------------------------------------------------------

#[test]
fn every_sample_query_runs_on_a_fresh_database() {
    for sql in SAMPLE_QUERIES {
        let mut d = db();
        d.execute(sql).unwrap_or_else(|e| panic!("sample {sql:?}: {e}"));
    }
}

#[test]
fn every_sample_query_runs_in_order_on_one_database() {
    let mut d = db();
    for sql in SAMPLE_QUERIES {
        d.execute(sql).unwrap_or_else(|e| panic!("sample {sql:?}: {e}"));
    }
    assert_eq!(d.row_count("users"), Some(5)); // +Frank, -Eve
    assert!(d.table_names().contains(&"notes"));
}

#[test]
fn sample_query_results() {
    assert_eq!(col("SELECT name FROM users WHERE age > 28"), vec!["Alice", "Carol", "Eve"]);
    assert_eq!(q("SELECT COUNT(*) FROM orders WHERE user_id = 1"), vec![vec!["2"]]);
    assert_eq!(col("SELECT name FROM users WHERE age >= 25 AND name LIKE '%a%' ORDER BY age DESC LIMIT 2"), vec!["Carol", "Alice"]);
    let mut d = db();
    run(&mut d, "UPDATE products SET stock = 60 WHERE name = 'Gadget'");
    assert_eq!(run(&mut d, "SELECT stock FROM products WHERE id = 2").rows, vec![vec!["60"]]);
    let o = run(&mut d, "SHOW TABLES");
    assert_eq!(o.rows, vec![vec!["users", "4", "5"], vec!["products", "4", "4"], vec!["orders", "4", "5"]]);
}

// ----- SELECT --------------------------------------------------------------------

#[test]
fn select_projection_order_limit() {
    assert_eq!(q("SELECT name, age FROM users WHERE id = 2"), vec![vec!["Bob", "25"]]);
    assert_eq!(col("SELECT age FROM users ORDER BY age"), vec!["25", "28", "30", "32", "35"]);
    // Case-insensitive names and keywords; quoted identifiers; trailing semicolon.
    assert_eq!(col("select NAME from USERS where AGE = 25;"), vec!["Bob"]);
    assert_eq!(col("SELECT \"name\" FROM \"users\" ORDER BY \"id\" LIMIT 0"), Vec::<String>::new());
    // Multi-key order, descending, stable for ties.
    let mut d = db();
    run(&mut d, "CREATE TABLE t (a INTEGER, b TEXT)");
    run(&mut d, "INSERT INTO t VALUES (1,'x'),(2,'y'),(1,'z'),(2,'a')");
    assert_eq!(run(&mut d, "SELECT b FROM t ORDER BY a DESC, b").rows, vec![vec!["a"], vec!["y"], vec!["x"], vec!["z"]]);
    assert_eq!(run(&mut d, "SELECT b FROM t ORDER BY a").rows, vec![vec!["x"], vec!["z"], vec!["y"], vec!["a"]]);
    assert_eq!(run(&mut d, "SELECT * FROM t LIMIT 2").message, "2 rows");
    assert_eq!(run(&mut d, "SELECT * FROM t WHERE a = 1 LIMIT 1").message, "1 row");
}

#[test]
fn count_star() {
    assert_eq!(q("SELECT COUNT(*) FROM users"), vec![vec!["5"]]);
    assert_eq!(q("SELECT COUNT(*) FROM users WHERE age > 100"), vec![vec!["0"]]);
    let mut d = db();
    assert!(fail(&mut d, "SELECT COUNT(*), name FROM users").contains("expected FROM"));
    assert!(fail(&mut d, "SELECT COUNT(name) FROM users").contains("expected '*'"));
    assert!(fail(&mut d, "SELECT COUNT(*) FROM users ORDER BY id").contains("COUNT"));
}

#[test]
fn where_logic_and_precedence() {
    // AND binds tighter than OR.
    assert_eq!(col("SELECT id FROM users WHERE id = 1 OR id = 2 AND age > 100 ORDER BY id"), vec!["1"]);
    assert_eq!(col("SELECT id FROM users WHERE (id = 1 OR id = 2) AND age > 25 ORDER BY id"), vec!["1"]);
    assert_eq!(col("SELECT id FROM users WHERE NOT id < 4 ORDER BY id"), vec!["4", "5"]);
    assert_eq!(col("SELECT id FROM users WHERE NOT (id = 1 OR id = 2) AND NOT id = 3 ORDER BY id"), vec!["4", "5"]);
    assert_eq!(col("SELECT id FROM users WHERE 3 = id"), vec!["3"]);
    assert_eq!(col("SELECT id FROM users WHERE id = age"), Vec::<String>::new());
    assert_eq!(col("SELECT id FROM users WHERE 1 = 1 ORDER BY id LIMIT 1"), vec!["1"]);
}

#[test]
fn null_semantics() {
    let mut d = db();
    run(&mut d, "CREATE TABLE t (a INTEGER, b TEXT)");
    run(&mut d, "INSERT INTO t VALUES (1, 'x'), (NULL, 'y'), (3, NULL)");
    let sel = |d: &mut Database, w: &str| run(d, &format!("SELECT b FROM t WHERE {w} ORDER BY b")).rows.concat();
    assert_eq!(sel(&mut d, "a = NULL"), Vec::<String>::new()); // unknown, not true
    assert_eq!(sel(&mut d, "a IS NULL"), vec!["y"]);
    assert_eq!(sel(&mut d, "a IS NOT NULL"), vec!["NULL", "x"]);
    assert_eq!(sel(&mut d, "a > 0 OR b = 'y'"), vec!["NULL", "x", "y"]);
    assert_eq!(sel(&mut d, "a > 0 AND b = 'x'"), vec!["x"]);
    assert_eq!(sel(&mut d, "NOT a = 1"), vec!["NULL"]); // NOT unknown is unknown
    assert_eq!(sel(&mut d, "b LIKE '%'"), vec!["x", "y"]);
    // NULL sorts first.
    assert_eq!(run(&mut d, "SELECT a FROM t ORDER BY a").rows.concat(), vec!["NULL", "1", "3"]);
}

#[test]
fn like_patterns() {
    let mut d = db();
    let names = |d: &mut Database, p: &str| run(d, &format!("SELECT name FROM users WHERE name LIKE '{p}' ORDER BY id")).rows.concat();
    assert_eq!(names(&mut d, "A%"), vec!["Alice"]);
    assert_eq!(names(&mut d, "a%"), vec!["Alice"]); // ASCII case-insensitive
    assert_eq!(names(&mut d, "%e"), vec!["Alice", "Dave", "Eve"]);
    assert_eq!(names(&mut d, "_ob"), vec!["Bob"]);
    assert_eq!(names(&mut d, "%a%e%"), vec!["Alice", "Dave"]);
    assert_eq!(names(&mut d, "Bob"), vec!["Bob"]);
    assert_eq!(names(&mut d, "%%%"), vec!["Alice", "Bob", "Carol", "Dave", "Eve"]);
    assert_eq!(names(&mut d, "B"), Vec::<String>::new());
    assert_eq!(run(&mut d, "SELECT name FROM users WHERE name NOT LIKE '%e' ORDER BY id").rows.concat(), vec!["Bob", "Carol"]);
    assert_eq!(run(&mut d, "SELECT name FROM users WHERE id LIKE '1'").rows.concat(), vec!["Alice"]);
    // Pathological pattern stays fast.
    run(&mut d, "CREATE TABLE long (v TEXT)");
    run(&mut d, &format!("INSERT INTO long VALUES ('{}')", "a".repeat(200)));
    let pat = format!("{}b", "%a".repeat(50));
    assert_eq!(run(&mut d, &format!("SELECT COUNT(*) FROM long WHERE v LIKE '{pat}'")).rows, vec![vec!["0"]]);
}

#[test]
fn mixed_type_ordering() {
    let mut d = db();
    run(&mut d, "CREATE TABLE m (v)");
    run(&mut d, "INSERT INTO m VALUES ('b'), (2), (NULL), ('a'), (10), (1.5)");
    assert_eq!(run(&mut d, "SELECT v FROM m ORDER BY v").rows.concat(), vec!["NULL", "1.5", "2", "10", "a", "b"]);
    assert_eq!(run(&mut d, "SELECT v FROM m WHERE v > 5 ORDER BY v").rows.concat(), vec!["10", "a", "b"]);
    assert_eq!(run(&mut d, "SELECT v FROM m WHERE v = '2'").rows.len(), 0); // number is not text
}

// ----- INSERT / UPDATE / DELETE --------------------------------------------------

#[test]
fn insert_forms_and_type_checks() {
    let mut d = db();
    let o = run(&mut d, "INSERT INTO users (name, id) VALUES ('Gina', 7), ('Hal', 8)");
    assert_eq!(o.message, "2 rows inserted into users");
    assert_eq!(run(&mut d, "SELECT id, name, email, age FROM users WHERE id > 6 ORDER BY id").rows, vec![vec!["7", "Gina", "NULL", "NULL"], vec!["8", "Hal", "NULL", "NULL"]]);
    assert!(fail(&mut d, "INSERT INTO users VALUES (9, 'x', 'y')").contains("row 1 has 3 values, expected 4"));
    assert!(fail(&mut d, "INSERT INTO users VALUES (9, 'x', 'y', 1, 2)").contains("has 5 values"));
    assert!(fail(&mut d, "INSERT INTO users (id) VALUES (1, 2)").contains("has 2 values, expected 1"));
    assert!(fail(&mut d, "INSERT INTO users (id, id) VALUES (1, 2)").contains("twice"));
    assert!(fail(&mut d, "INSERT INTO users (nope) VALUES (1)").contains("no column 'nope'"));
    assert!(fail(&mut d, "INSERT INTO users VALUES ('x', 'n', 'e', 1)").contains("column 'id' is INTEGER, got 'x'"));
    assert!(fail(&mut d, "INSERT INTO users VALUES (1.5, 'n', 'e', 1)").contains("INTEGER, got 1.5"));
    assert!(fail(&mut d, "INSERT INTO products VALUES (9, 'n', 'cheap', 1)").contains("column 'price' is REAL"));
    assert!(fail(&mut d, "INSERT INTO nope VALUES (1)").contains("no table 'nope'"));
    assert!(fail(&mut d, "INSERT users VALUES (1)").contains("expected INTO"));
    assert!(fail(&mut d, "INSERT INTO users").contains("expected VALUES"));
    assert!(fail(&mut d, "INSERT INTO users VALUES").contains("expected '('"));
    assert!(fail(&mut d, "INSERT INTO users VALUES ()").contains("expected a value"));
    assert!(fail(&mut d, "INSERT INTO users VALUES (1,)").contains("expected a value"));
    // A failed multi-row insert adds nothing.
    let before = d.row_count("users");
    assert!(d.execute("INSERT INTO users VALUES (20,'a','b',1), (21,'c','d','bad')").is_err());
    assert_eq!(d.row_count("users"), before);
    // TEXT columns store numbers as text; REAL accepts integers; negatives work.
    run(&mut d, "INSERT INTO users VALUES (-3, 42, 'e', 0)");
    assert_eq!(run(&mut d, "SELECT name, id FROM users WHERE id < 0").rows, vec![vec!["42", "-3"]]);
    run(&mut d, "INSERT INTO products VALUES (9, 'n', 3, 1)");
    assert_eq!(run(&mut d, "SELECT price FROM products WHERE id = 9").rows, vec![vec!["3"]]);
}

#[test]
fn update_and_delete() {
    let mut d = db();
    assert_eq!(run(&mut d, "UPDATE users SET age = 31, email = 'new@x' WHERE name = 'Alice'").message, "1 row updated");
    assert_eq!(run(&mut d, "SELECT age, email FROM users WHERE id = 1").rows, vec![vec!["31", "new@x"]]);
    assert_eq!(run(&mut d, "UPDATE users SET age = 0").message, "5 rows updated");
    assert_eq!(run(&mut d, "UPDATE users SET age = NULL WHERE id = 1").message, "1 row updated");
    assert_eq!(run(&mut d, "UPDATE users SET age = 1 WHERE id = 99").message, "0 rows updated");
    assert!(fail(&mut d, "UPDATE users SET age = 'old'").contains("INTEGER"));
    assert!(fail(&mut d, "UPDATE users SET nope = 1").contains("no column"));
    assert!(fail(&mut d, "UPDATE users SET age = 1, age = 2").contains("twice"));
    assert!(fail(&mut d, "UPDATE users SET age 1").contains("expected '='"));
    assert!(fail(&mut d, "UPDATE users SET age = 1 WHERE nope = 1").contains("no column 'nope'"));
    // Failed update changes nothing.
    assert_eq!(run(&mut d, "SELECT COUNT(*) FROM users WHERE age = 0").rows, vec![vec!["4"]]);
    assert_eq!(run(&mut d, "DELETE FROM users WHERE age = 0 AND id > 3").message, "2 rows deleted");
    assert_eq!(d.row_count("users"), Some(3));
    assert_eq!(run(&mut d, "DELETE FROM users").message, "3 rows deleted");
    assert_eq!(d.row_count("users"), Some(0));
    assert!(fail(&mut d, "DELETE users").contains("expected FROM"));
    assert!(fail(&mut d, "DELETE FROM nope").contains("no table"));
    assert!(fail(&mut d, "DELETE FROM users WHERE").contains("expected a column or value"));
}

#[test]
fn create_and_drop() {
    let mut d = Database::new();
    assert_eq!(run(&mut d, "CREATE TABLE a (x INTEGER, y REAL, z TEXT, w, v NUMERIC, u DECIMAL, t INT)").message, "Table 'a' created with 7 columns");
    let o = run(&mut d, "DESCRIBE a");
    let types: Vec<&str> = o.rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(types, vec!["INTEGER", "REAL", "TEXT", "(any)", "REAL", "REAL", "INTEGER"]);
    assert!(fail(&mut d, "CREATE TABLE A (x)").contains("already exists"));
    assert!(fail(&mut d, "CREATE TABLE b (x, X)").contains("appears twice"));
    assert!(fail(&mut d, "CREATE TABLE b (x BLOB)").contains("unknown column type 'BLOB'"));
    assert!(fail(&mut d, "CREATE TABLE b (x INTEGER NOT NULL)").contains("NOT is not supported"));
    assert!(fail(&mut d, "CREATE TABLE b (x NOT NULL)").contains("NOT is not supported"));
    assert!(fail(&mut d, "CREATE TABLE select (x)").contains("keyword"));
    assert!(fail(&mut d, "CREATE TABLE t (a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q)").contains("more than 16 columns"));
    run(&mut d, "CREATE TABLE \"select\" (\"from\" TEXT)");
    run(&mut d, "INSERT INTO \"select\" VALUES ('ok')");
    assert_eq!(run(&mut d, "SELECT \"from\" FROM \"select\"").rows, vec![vec!["ok"]]);
    assert_eq!(run(&mut d, "DROP TABLE a").message, "Table 'a' dropped");
    assert!(fail(&mut d, "DROP TABLE a").contains("no table"));
    assert!(fail(&mut d, "DROP a").contains("expected TABLE"));
    for i in 0..15 {
        run(&mut d, &format!("CREATE TABLE t{i} (x)"));
    }
    assert!(fail(&mut d, "CREATE TABLE overflow (x)").contains("at most 16 tables"));
}

#[test]
fn row_and_text_limits() {
    let mut d = Database::new();
    run(&mut d, "CREATE TABLE t (x INTEGER)");
    let batch: Vec<String> = (0..250).map(|i| format!("({i})")).collect();
    run(&mut d, &format!("INSERT INTO t VALUES {}", batch.join(",")));
    run(&mut d, &format!("INSERT INTO t VALUES {}", batch.join(",")));
    assert_eq!(d.row_count("t"), Some(500));
    assert!(fail(&mut d, "INSERT INTO t VALUES (1)").contains("exceed 500 rows"));
    assert!(fail(&mut d, &format!("INSERT INTO t VALUES ('{}')", "x".repeat(201))).contains("longer than 200"));
}

// ----- numbers -------------------------------------------------------------------

#[test]
fn numbers() {
    let n = |s: &str| Num::parse(s).unwrap();
    assert_eq!(n("0").to_string(), "0");
    assert_eq!(n("-0").to_string(), "0");
    assert_eq!(n("007").to_string(), "7");
    assert_eq!(n("9.99").to_string(), "9.99");
    assert_eq!(n("-0.05").to_string(), "-0.05");
    assert_eq!(n("1.500").to_string(), "1.500");
    assert_eq!(n("100").cmp_num(&n("99.999")), std::cmp::Ordering::Greater);
    assert_eq!(n("1").cmp_num(&n("1.0")), std::cmp::Ordering::Equal);
    assert_eq!(n("-5").cmp_num(&n("-4.9")), std::cmp::Ordering::Less);
    assert_eq!(n("123456789012345678").cmp_num(&n("0.00000000000000001")), std::cmp::Ordering::Greater);
    for bad in ["", "-", ".5", "5.", "1.2.3", "1e5", "abc", "1 2", "+1", "1234567890123456789"] {
        assert!(Num::parse(bad).is_err(), "{bad:?}");
    }
    let mut d = db();
    assert!(fail(&mut d, "SELECT * FROM users WHERE id = 1234567890123456789").contains("more than 18 digits"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE id = 1.2.3").contains("not a number"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE id = .5").contains("not a number"));
    assert!(fail(&mut d, "SELECT * FROM users LIMIT -1").contains("LIMIT"));
    assert!(fail(&mut d, "SELECT * FROM users LIMIT 1.5").contains("LIMIT"));
    assert!(fail(&mut d, "SELECT * FROM users LIMIT x").contains("expected a number"));
    assert_eq!(run(&mut d, "SELECT * FROM users LIMIT 99999999999999999").rows.len(), 5);
}

// ----- errors and robustness -----------------------------------------------------

#[test]
fn errors_name_the_problem() {
    let mut d = db();
    assert_eq!(fail(&mut d, ""), "empty query");
    assert_eq!(fail(&mut d, "   ;"), "a statement can't start with ';'");
    assert!(fail(&mut d, "FROBNICATE").contains("unknown command 'FROBNICATE'"));
    assert!(fail(&mut d, "SELECT * FROM nope").contains("no table 'nope'"));
    assert!(fail(&mut d, "SELECT nope FROM users").contains("no column 'nope' in table 'users'"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE nope = 1").contains("no column 'nope'"));
    assert!(fail(&mut d, "SELECT * FROM users ORDER BY nope").contains("no column 'nope'"));
    assert!(fail(&mut d, "SELECT * users").contains("expected FROM"));
    assert!(fail(&mut d, "SELECT FROM users").contains("expected a column list"));
    assert!(fail(&mut d, "SELECT * FROM").contains("expected a table name"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE").contains("expected a column or value"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE age").contains("incomplete condition"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE age >").contains("expected a column or value"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE users.age = 1").contains("expected a comparison"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE age ~ 1").contains("unexpected character '~'"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE name = 'unterminated").contains("unterminated string"));
    assert!(fail(&mut d, "SELECT \"x FROM users").contains("unterminated quoted name"));
    assert!(fail(&mut d, "SELECT * FROM users; SELECT 1").contains("one statement at a time"));
    assert!(fail(&mut d, "SELECT * FROM users extra").contains("unexpected 'extra'"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE (age > 1").contains("expected ')'"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE age > 1)").contains("unexpected ')'"));
    assert!(fail(&mut d, "SELECT * FROM select").contains("keyword"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE id = 1 AND").contains("expected a column or value"));
    assert!(fail(&mut d, "SELECT * FROM users WHERE id IS 1").contains("expected NULL"));
    assert!(fail(&mut d, "SELECT * FROM users ORDER id").contains("expected BY"));
    assert!(fail(&mut d, "SHOW users").contains("expected TABLES"));
    assert!(fail(&mut d, "DESCRIBE").contains("expected a table name"));
    assert!(fail(&mut d, "DESCRIBE nope").contains("no table"));
    assert!(fail(&mut d, "SELECT 1").contains("expected a column name, found 1"));
    let deep = format!("SELECT * FROM users WHERE {}id = 1{}", "(".repeat(100), ")".repeat(100));
    assert!(fail(&mut d, &deep).contains("nested deeper"));
    let nots = format!("SELECT * FROM users WHERE {}id = 1", "NOT ".repeat(100));
    assert!(fail(&mut d, &nots).contains("nested deeper"));
    // Moderate nesting still works.
    let ok = format!("SELECT id FROM users WHERE {}id = 1{}", "(".repeat(10), ")".repeat(10));
    assert_eq!(run(&mut d, &ok).rows, vec![vec!["1"]]);
}

#[test]
fn describe_and_show() {
    let mut d = db();
    let o = run(&mut d, "DESCRIBE users");
    assert_eq!(o.columns, vec!["column", "type"]);
    assert_eq!(o.rows[3], vec!["age", "INTEGER"]);
    assert_eq!(o.message, "4 columns in users");
    assert_eq!(run(&mut d, "show tables;").message, "3 tables");
}

#[test]
fn never_panics_on_arbitrary_input() {
    let words = [
        "SELECT", "FROM", "WHERE", "users", "products", "name", "age", "id", "*", ",", "(", ")", "'x'", "''", "\"q\"", "1", "-", "2.5",
        "=", "<", ">", "<=", ">=", "<>", "!=", "AND", "OR", "NOT", "NULL", "IS", "LIKE", "ORDER", "BY", "DESC", "LIMIT", "INSERT", "INTO",
        "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "DROP", "COUNT", "SHOW", "TABLES", "DESCRIBE", ";", "'", "\"", "%", "\u{e9}",
        "JOIN", "GROUP", "123456789012345678901234567890", ".",
    ];
    let mut x: u64 = 0xA5A5_5A5A_1234_8765;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut d = db();
    for _ in 0..20_000 {
        let n = (next() % 14) as usize;
        let sql: String = (0..n).map(|_| words[(next() as usize) % words.len()]).collect::<Vec<_>>().join(" ");
        let _ = d.execute(&sql);
        // Keep the database from filling up with junk tables/rows.
        if next() % 50 == 0 {
            d = db();
        }
    }
    // Character-level junk too.
    let chars: Vec<char> = "SELECT*FROMusers WHERE'\"(),;=<>!-.%_\u{e9}\u{0}\n".chars().collect();
    for _ in 0..20_000 {
        let n = (next() % 30) as usize;
        let sql: String = (0..n).map(|_| chars[(next() as usize) % chars.len()]).collect();
        let _ = d.execute(&sql);
    }
}

#[test]
fn truncating_every_sample_never_panics() {
    for sql in SAMPLE_QUERIES {
        let chars: Vec<char> = sql.chars().collect();
        for i in 0..=chars.len() {
            let mut d = db();
            let s: String = chars[..i].iter().collect();
            let _ = d.execute(&s);
        }
    }
}

// ----- table formatting ----------------------------------------------------------

#[test]
fn table_formatting() {
    let cols = vec!["id".to_string(), "name".to_string()];
    let rows = vec![vec!["1".to_string(), "Alice".to_string()], vec!["22".to_string(), "Bo".to_string()]];
    assert_eq!(format_table(&cols, &rows, 12), vec!["id | name", "---+------", "1  | Alice", "22 | Bo"]);
    // Long cells are cut with an ellipsis; control characters can't break a row.
    let rows = vec![vec!["1".to_string(), "x".repeat(40)], vec!["2".to_string(), "a\nb".to_string()]];
    let t = format_table(&cols, &rows, 6);
    assert_eq!(t[2], "1  | xxxxx\u{2026}");
    assert_eq!(t[3], "2  | a b");
    // Empty result: header and rule only; ragged rows don't panic.
    assert_eq!(format_table(&cols, &[], 12).len(), 2);
    let _ = format_table(&cols, &[vec![], vec!["1".into(), "2".into(), "3".into()]], 12);
    let _ = format_table(&[], &[], 12);
}
