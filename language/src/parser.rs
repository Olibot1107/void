use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut out = Vec::new();

        while !self.is_eof() {
            self.skip_semicolons();
            if self.is_eof() {
                break;
            }
            out.push(self.parse_stmt()?);
            self.skip_semicolons();
        }

        Ok(out)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.matches_keyword_use() {
            return self.parse_use();
        }
        if self.matches_keyword_let() {
            return self.parse_let();
        }
        if self.matches_keyword_if() {
            return self.parse_if();
        }
        if self.matches_keyword_while() {
            return self.parse_while();
        }
        if self.matches_keyword_repeat() {
            return self.parse_repeat();
        }
        if self.matches_keyword_return() {
            return self.parse_return();
        }
        if self.looks_like_assignment() {
            return self.parse_assign();
        }

        Ok(Stmt::Expr(self.parse_expr()?))
    }

    fn parse_use(&mut self) -> Result<Stmt, String> {
        self.consume_use()?;
        let specifier = self.consume_string()?;
        self.consume_as()?;
        let alias = self.consume_ident()?;
        Ok(Stmt::Use { specifier, alias })
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        self.consume_let()?;
        let name = self.consume_ident()?;
        self.consume_equal()?;
        let expr = self.parse_expr()?;
        Ok(Stmt::Let { name, expr })
    }

    fn parse_assign(&mut self) -> Result<Stmt, String> {
        let mut target = vec![self.consume_ident()?];
        while self.matches_dot() {
            self.consume_dot()?;
            target.push(self.consume_ident()?);
        }
        self.consume_equal()?;
        let expr = self.parse_expr()?;
        Ok(Stmt::Assign { target, expr })
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.consume_if()?;
        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.matches_keyword_else() {
            self.consume_else()?;
            if self.matches_keyword_if() {
                vec![self.parse_if()?]
            } else {
                self.parse_block()?
            }
        } else {
            Vec::new()
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.consume_while()?;
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body })
    }

    fn parse_repeat(&mut self) -> Result<Stmt, String> {
        self.consume_repeat()?;
        let count = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::Repeat { count, body })
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        self.consume_return()?;
        if self.matches_semicolon() || self.matches_rbrace() {
            return Ok(Stmt::Return(Expr::Null));
        }
        let expr = self.parse_expr()?;
        Ok(Stmt::Return(expr))
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.consume_lbrace()?;
        let mut body = Vec::new();

        loop {
            self.skip_semicolons();
            if self.matches_rbrace() {
                break;
            }
            if self.is_eof() {
                return Err(self.error_here("Unterminated block"));
            }
            body.push(self.parse_stmt()?);
            self.skip_semicolons();
        }

        self.consume_rbrace()?;
        Ok(body)
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_and()?;

        while self.matches_or_or() {
            self.consume_or_or()?;
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_equality()?;

        while self.matches_and_and() {
            self.consume_and_and()?;
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;

        loop {
            if self.matches_equal_equal() {
                self.consume_equal_equal()?;
                let right = self.parse_comparison()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Eq,
                    right: Box::new(right),
                };
            } else if self.matches_bang_equal() {
                self.consume_bang_equal()?;
                let right = self.parse_comparison()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Ne,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_additive()?;

        loop {
            if self.matches_less() {
                self.consume_less()?;
                let right = self.parse_additive()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Lt,
                    right: Box::new(right),
                };
            } else if self.matches_less_equal() {
                self.consume_less_equal()?;
                let right = self.parse_additive()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Lte,
                    right: Box::new(right),
                };
            } else if self.matches_greater() {
                self.consume_greater()?;
                let right = self.parse_additive()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Gt,
                    right: Box::new(right),
                };
            } else if self.matches_greater_equal() {
                self.consume_greater_equal()?;
                let right = self.parse_additive()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Gte,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_multiplicative()?;

        loop {
            if self.matches_plus() {
                self.consume_plus()?;
                let right = self.parse_multiplicative()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Add,
                    right: Box::new(right),
                };
            } else if self.matches_minus() {
                self.consume_minus()?;
                let right = self.parse_multiplicative()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Sub,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_unary()?;

        loop {
            if self.matches_star() {
                self.consume_star()?;
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Mul,
                    right: Box::new(right),
                };
            } else if self.matches_slash() {
                self.consume_slash()?;
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Div,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.matches_minus() {
            self.consume_minus()?;
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }

        if self.matches_bang() {
            self.consume_bang()?;
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.matches_lparen() {
                self.consume_lparen()?;
                let mut args = Vec::new();
                if !self.matches_rparen() {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.matches_comma() {
                            self.consume_comma()?;
                            continue;
                        }
                        break;
                    }
                }
                self.consume_rparen()?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }

            if self.matches_dot() {
                self.consume_dot()?;
                let property = self.consume_ident()?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
                continue;
            }

            break;
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        if self.matches_number() {
            let n = self.consume_number()?;
            return Ok(Expr::Number(n));
        }

        if self.matches_string() {
            let s = self.consume_string()?;
            return Ok(Expr::String(s));
        }

        if self.matches_keyword_true() {
            self.consume_true()?;
            return Ok(Expr::Bool(true));
        }

        if self.matches_keyword_false() {
            self.consume_false()?;
            return Ok(Expr::Bool(false));
        }

        if self.matches_keyword_null() {
            self.consume_null()?;
            return Ok(Expr::Null);
        }

        if self.matches_ident() {
            let name = self.consume_ident()?;
            return Ok(Expr::Var(name));
        }

        if self.matches_keyword_fn() {
            return self.parse_fn_literal();
        }

        if self.matches_lparen() {
            self.consume_lparen()?;
            let expr = self.parse_expr()?;
            self.consume_rparen()?;
            return Ok(expr);
        }

        Err(self.error_here("Expected expression"))
    }

    fn parse_fn_literal(&mut self) -> Result<Expr, String> {
        self.consume_fn()?;
        self.consume_lparen()?;

        let mut params = Vec::new();
        if !self.matches_rparen() {
            loop {
                params.push(self.consume_ident()?);
                if self.matches_comma() {
                    self.consume_comma()?;
                    continue;
                }
                break;
            }
        }

        self.consume_rparen()?;
        let body = self.parse_block()?;

        Ok(Expr::FnLiteral { params, body })
    }

    fn looks_like_assignment(&self) -> bool {
        let mut i = self.pos;

        let Some(Token {
            kind: TokenKind::Ident(_),
            ..
        }) = self.tokens.get(i)
        else {
            return false;
        };

        i += 1;
        loop {
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::Dot) => {
                    i += 1;
                    if !matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Ident(_))) {
                        return false;
                    }
                    i += 1;
                }
                Some(TokenKind::Equal) => return true,
                _ => return false,
            }
        }
    }

    fn skip_semicolons(&mut self) {
        while matches!(self.current_kind(), Some(TokenKind::Semicolon)) {
            self.pos += 1;
        }
    }

    fn is_eof(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Eof) | None)
    }

    fn current_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn current_line(&self) -> usize {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .map(|t| t.line)
            .unwrap_or(0)
    }

    fn error_here(&self, message: &str) -> String {
        format!("{message} at line {}", self.current_line())
    }

    fn matches_keyword_use(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Use))
    }
    fn matches_keyword_as(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::As))
    }
    fn matches_keyword_let(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Let))
    }
    fn matches_keyword_fn(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Fn))
    }
    fn matches_keyword_return(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Return))
    }
    fn matches_keyword_if(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::If))
    }
    fn matches_keyword_else(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Else))
    }
    fn matches_keyword_while(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::While))
    }
    fn matches_keyword_repeat(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Repeat))
    }
    fn matches_keyword_true(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::True))
    }
    fn matches_keyword_false(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::False))
    }
    fn matches_keyword_null(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Null))
    }

    fn matches_ident(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Ident(_)))
    }
    fn matches_number(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Number(_)))
    }
    fn matches_string(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::String(_)))
    }
    fn matches_semicolon(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Semicolon))
    }
    fn matches_lparen(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::LParen))
    }
    fn matches_rparen(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::RParen))
    }
    fn matches_lbrace(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::LBrace))
    }
    fn matches_rbrace(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::RBrace))
    }
    fn matches_comma(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Comma))
    }
    fn matches_dot(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Dot))
    }
    fn matches_plus(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Plus))
    }
    fn matches_minus(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Minus))
    }
    fn matches_star(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Star))
    }
    fn matches_slash(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Slash))
    }
    fn matches_bang(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Bang))
    }
    fn matches_equal(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Equal))
    }
    fn matches_equal_equal(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::EqualEqual))
    }
    fn matches_bang_equal(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::BangEqual))
    }
    fn matches_less(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Less))
    }
    fn matches_less_equal(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::LessEqual))
    }
    fn matches_greater(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Greater))
    }
    fn matches_greater_equal(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::GreaterEqual))
    }
    fn matches_and_and(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::AndAnd))
    }
    fn matches_or_or(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::OrOr))
    }

    fn consume_use(&mut self) -> Result<(), String> {
        if self.matches_keyword_use() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'use'"))
        }
    }
    fn consume_as(&mut self) -> Result<(), String> {
        if self.matches_keyword_as() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'as'"))
        }
    }
    fn consume_let(&mut self) -> Result<(), String> {
        if self.matches_keyword_let() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'let'"))
        }
    }
    fn consume_fn(&mut self) -> Result<(), String> {
        if self.matches_keyword_fn() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'fn'"))
        }
    }
    fn consume_return(&mut self) -> Result<(), String> {
        if self.matches_keyword_return() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'return'"))
        }
    }
    fn consume_if(&mut self) -> Result<(), String> {
        if self.matches_keyword_if() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'if'"))
        }
    }
    fn consume_else(&mut self) -> Result<(), String> {
        if self.matches_keyword_else() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'else'"))
        }
    }
    fn consume_while(&mut self) -> Result<(), String> {
        if self.matches_keyword_while() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'while'"))
        }
    }
    fn consume_repeat(&mut self) -> Result<(), String> {
        if self.matches_keyword_repeat() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'repeat'"))
        }
    }
    fn consume_true(&mut self) -> Result<(), String> {
        if self.matches_keyword_true() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'true'"))
        }
    }
    fn consume_false(&mut self) -> Result<(), String> {
        if self.matches_keyword_false() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'false'"))
        }
    }
    fn consume_null(&mut self) -> Result<(), String> {
        if self.matches_keyword_null() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected 'null'"))
        }
    }

    fn consume_ident(&mut self) -> Result<String, String> {
        match self.current_kind() {
            Some(TokenKind::Ident(name)) => {
                let out = name.clone();
                self.pos += 1;
                Ok(out)
            }
            _ => Err(self.error_here("Expected identifier")),
        }
    }
    fn consume_number(&mut self) -> Result<f64, String> {
        match self.current_kind() {
            Some(TokenKind::Number(value)) => {
                let out = *value;
                self.pos += 1;
                Ok(out)
            }
            _ => Err(self.error_here("Expected number")),
        }
    }
    fn consume_string(&mut self) -> Result<String, String> {
        match self.current_kind() {
            Some(TokenKind::String(value)) => {
                let out = value.clone();
                self.pos += 1;
                Ok(out)
            }
            _ => Err(self.error_here("Expected string")),
        }
    }
    fn consume_lparen(&mut self) -> Result<(), String> {
        if self.matches_lparen() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '('"))
        }
    }
    fn consume_rparen(&mut self) -> Result<(), String> {
        if self.matches_rparen() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected ')'"))
        }
    }
    fn consume_lbrace(&mut self) -> Result<(), String> {
        if self.matches_lbrace() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '{'"))
        }
    }
    fn consume_rbrace(&mut self) -> Result<(), String> {
        if self.matches_rbrace() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '}'"))
        }
    }
    fn consume_comma(&mut self) -> Result<(), String> {
        if self.matches_comma() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected ','"))
        }
    }
    fn consume_dot(&mut self) -> Result<(), String> {
        if self.matches_dot() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '.'"))
        }
    }
    fn consume_plus(&mut self) -> Result<(), String> {
        if self.matches_plus() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '+'"))
        }
    }
    fn consume_minus(&mut self) -> Result<(), String> {
        if self.matches_minus() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '-'"))
        }
    }
    fn consume_star(&mut self) -> Result<(), String> {
        if self.matches_star() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '*'"))
        }
    }
    fn consume_slash(&mut self) -> Result<(), String> {
        if self.matches_slash() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '/'"))
        }
    }
    fn consume_bang(&mut self) -> Result<(), String> {
        if self.matches_bang() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '!'"))
        }
    }
    fn consume_equal(&mut self) -> Result<(), String> {
        if self.matches_equal() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '='"))
        }
    }
    fn consume_equal_equal(&mut self) -> Result<(), String> {
        if self.matches_equal_equal() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '=='"))
        }
    }
    fn consume_bang_equal(&mut self) -> Result<(), String> {
        if self.matches_bang_equal() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '!='"))
        }
    }
    fn consume_less(&mut self) -> Result<(), String> {
        if self.matches_less() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '<'"))
        }
    }
    fn consume_less_equal(&mut self) -> Result<(), String> {
        if self.matches_less_equal() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '<='"))
        }
    }
    fn consume_greater(&mut self) -> Result<(), String> {
        if self.matches_greater() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '>'"))
        }
    }
    fn consume_greater_equal(&mut self) -> Result<(), String> {
        if self.matches_greater_equal() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '>='"))
        }
    }
    fn consume_and_and(&mut self) -> Result<(), String> {
        if self.matches_and_and() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '&&'"))
        }
    }
    fn consume_or_or(&mut self) -> Result<(), String> {
        if self.matches_or_or() {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_here("Expected '||'"))
        }
    }
}
