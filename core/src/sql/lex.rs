//! Tokenizer: operators are tokens on their own (`age>28` works), quoted
//! strings keep their text, `"quoted"` identifiers are never keywords.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// Bare identifier or keyword (case preserved).
    Word(String),
    /// `"quoted"` identifier.
    Quoted(String),
    Str(String),
    /// Number literal text (sign handled by the parser).
    Num(String),
    LParen,
    RParen,
    Comma,
    Dot,
    Semi,
    Star,
    Minus,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Word(w) => format!("'{w}'"),
            Tok::Quoted(w) => format!("\"{w}\""),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Num(n) => n.clone(),
            Tok::LParen => "'('".into(),
            Tok::RParen => "')'".into(),
            Tok::Comma => "','".into(),
            Tok::Dot => "'.'".into(),
            Tok::Semi => "';'".into(),
            Tok::Star => "'*'".into(),
            Tok::Minus => "'-'".into(),
            Tok::Eq => "'='".into(),
            Tok::Ne => "'!='".into(),
            Tok::Lt => "'<'".into(),
            Tok::Le => "'<='".into(),
            Tok::Gt => "'>'".into(),
            Tok::Ge => "'>='".into(),
        }
    }
}

pub fn tokenize(input: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '.' if !next.is_some_and(|n| n.is_ascii_digit()) => {
                toks.push(Tok::Dot);
                i += 1;
            }
            ';' => {
                toks.push(Tok::Semi);
                i += 1;
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '=' => {
                // `==` is accepted as `=`.
                i += if next == Some('=') { 2 } else { 1 };
                toks.push(Tok::Eq);
            }
            '!' if next == Some('=') => {
                toks.push(Tok::Ne);
                i += 2;
            }
            '<' => match next {
                Some('=') => {
                    toks.push(Tok::Le);
                    i += 2;
                }
                Some('>') => {
                    toks.push(Tok::Ne);
                    i += 2;
                }
                _ => {
                    toks.push(Tok::Lt);
                    i += 1;
                }
            },
            '>' => {
                if next == Some('=') {
                    toks.push(Tok::Ge);
                    i += 2;
                } else {
                    toks.push(Tok::Gt);
                    i += 1;
                }
            }
            '\'' | '"' => {
                let q = c;
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err(format!("unterminated {} starting at character {}", if q == '\'' { "string" } else { "quoted name" }, i + 1)),
                        Some(&ch) if ch == q => {
                            if chars.get(i + 1) == Some(&q) {
                                s.push(q);
                                i += 2;
                            } else {
                                i += 1;
                                break;
                            }
                        }
                        Some(&ch) => {
                            s.push(ch);
                            i += 1;
                        }
                    }
                }
                toks.push(if q == '\'' { Tok::Str(s) } else { Tok::Quoted(s) });
            }
            c if c.is_ascii_digit() || (c == '.' && next.is_some_and(|n| n.is_ascii_digit())) => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                toks.push(Tok::Num(chars[start..i].iter().collect()));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Word(chars[start..i].iter().collect()));
            }
            other => return Err(format!("unexpected character '{other}' at {}", i + 1)),
        }
    }
    Ok(toks)
}
