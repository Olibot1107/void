#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum TokenKind {
    Ident(String),
    Number(f64),
    String(String),
    Use,
    As,
    Let,
    Fn,
    Return,
    If,
    Else,
    While,
    Repeat,
    True,
    False,
    Null,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Semicolon,
    Eof,
}

pub fn lex(input: &str) -> Result<Vec<Token>, String> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    let mut line = 1usize;

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' => {
                chars.next();
            }
            '\n' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Semicolon,
                    line,
                });
                line += 1;
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '\n' {
                            tokens.push(Token {
                                kind: TokenKind::Semicolon,
                                line,
                            });
                            line += 1;
                            break;
                        }
                    }
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Slash,
                        line,
                    });
                }
            }
            '(' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    line,
                });
            }
            ')' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    line,
                });
            }
            '{' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    line,
                });
            }
            '}' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    line,
                });
            }
            '[' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::LBracket,
                    line,
                });
            }
            ']' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::RBracket,
                    line,
                });
            }
            ',' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    line,
                });
            }
            ':' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Colon,
                    line,
                });
            }
            '.' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Dot,
                    line,
                });
            }
            '+' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Plus,
                    line,
                });
            }
            '-' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Minus,
                    line,
                });
            }
            '*' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Star,
                    line,
                });
            }
            ';' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Semicolon,
                    line,
                });
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::EqualEqual,
                        line,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Equal,
                        line,
                    });
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::BangEqual,
                        line,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Bang,
                        line,
                    });
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::LessEqual,
                        line,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Less,
                        line,
                    });
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::GreaterEqual,
                        line,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Greater,
                        line,
                    });
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::AndAnd,
                        line,
                    });
                } else {
                    return Err(format!("Unexpected character '&' at line {line}"));
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token {
                        kind: TokenKind::OrOr,
                        line,
                    });
                } else {
                    return Err(format!("Unexpected character '|' at line {line}"));
                }
            }
            '"' | '\'' => {
                let quote = ch;
                chars.next();
                let mut out = String::new();
                let mut closed = false;

                while let Some(c) = chars.next() {
                    if c == quote {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        if let Some(next) = chars.next() {
                            match next {
                                'n' => out.push('\n'),
                                't' => out.push('\t'),
                                'r' => out.push('\r'),
                                '\\' => out.push('\\'),
                                '"' => out.push('"'),
                                '\'' => out.push('\''),
                                other => out.push(other),
                            }
                        }
                    } else {
                        if c == '\n' {
                            line += 1;
                        }
                        out.push(c);
                    }
                }

                if !closed {
                    return Err(format!("Unterminated string at line {line}"));
                }

                tokens.push(Token {
                    kind: TokenKind::String(out),
                    line,
                });
            }
            c if c.is_ascii_digit() => {
                let mut number = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2.is_ascii_digit() || c2 == '.' {
                        number.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let parsed = number
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid number '{number}' at line {line}"))?;

                tokens.push(Token {
                    kind: TokenKind::Number(parsed),
                    line,
                });
            }
            c if is_ident_start(c) => {
                let mut ident = String::new();
                while let Some(&c2) = chars.peek() {
                    if is_ident_part(c2) {
                        ident.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let kind = match ident.as_str() {
                    "use" => TokenKind::Use,
                    "as" => TokenKind::As,
                    "let" => TokenKind::Let,
                    "fn" => TokenKind::Fn,
                    "return" => TokenKind::Return,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "repeat" => TokenKind::Repeat,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "null" => TokenKind::Null,
                    _ => TokenKind::Ident(ident),
                };

                tokens.push(Token { kind, line });
            }
            other => {
                return Err(format!("Unexpected character '{other}' at line {line}"));
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
    });

    Ok(tokens)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
