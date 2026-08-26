//! Expressions, and the priority ladder that shapes them.

use crate::ast::{BinOp, ExprId, ExprKind, Field, UnOp};
use crate::error::SyntaxError;
use crate::token::TokenKind;

use super::Parser;

/// Every unary operator binds at the same strength, tighter than any binary one but looser than `^`.
const UNARY_PRIORITY: u8 = 12;

impl<'a> Parser<'a> {
    pub(super) fn expr(&mut self) -> Result<ExprId, SyntaxError> {
        self.subexpr(0)
    }

    /// `explist -> expr { ',' expr }`
    pub(super) fn expr_list(&mut self) -> Result<Vec<ExprId>, SyntaxError> {
        let mut list = vec![self.expr()?];

        while self.eat_byte(b',')? {
            list.push(self.expr()?);
        }

        Ok(list)
    }

    /// Takes operators binidng tighter than `limit`.
    fn subexpr(&mut self, limit: u8) -> Result<ExprId, SyntaxError> {
        let start = self.current.span.start;

        let mut left = match unary_op(&self.current.kind) {
            Some(op) => {
                self.advance()?;
                let operand = self.subexpr(UNARY_PRIORITY)?;
                self.builder
                    .expr(ExprKind::Unary { op, operand }, self.span_from(start))
            }
            None => self.simple_expr()?,
        };

        while let Some(op) = binary_op(&self.current.kind) {
            let (left_priority, right_priority) = binary_priority(op);
            if left_priority <= limit {
                break;
            }

            self.advance()?;
            let right = self.subexpr(right_priority)?;
            left = self
                .builder
                .expr(ExprKind::Binary { op, left, right }, self.span_from(start));
        }

        Ok(left)
    }

    fn simple_expr(&mut self) -> Result<ExprId, SyntaxError> {
        let start = self.current.span.start;

        let kind = match &self.current.kind {
            TokenKind::Nil => ExprKind::Nil,
            TokenKind::True => ExprKind::True,
            TokenKind::False => ExprKind::False,
            TokenKind::Int(value) => ExprKind::Int(*value),
            TokenKind::Float(value) => ExprKind::Float(*value),
            TokenKind::Str(bytes) => ExprKind::Str(bytes.clone()),
            TokenKind::Dots => ExprKind::Vararg,
            TokenKind::Byte(b'{') => return self.table(),
            _ => return self.suffixed_expr(),
        };

        self.advance()?;
        Ok(self.builder.expr(kind, self.span_from(start)))
    }

    /// `primaryexp -> NAME | '(' expr ')'`
    fn primary_expr(&mut self) -> Result<ExprId, SyntaxError> {
        let start = self.current.span.start;

        if let Some(name) = self.current_name() {
            self.advance()?;
            return Ok(self
                .builder
                .expr(ExprKind::Name(name), self.span_from(start)));
        }

        if self.eat_byte(b'(')? {
            let inner = self.expr()?;
            if !self.eat_byte(b')')? {
                return Err(self.not_implemented());
            }

            return Ok(self
                .builder
                .expr(ExprKind::Paren(inner), self.span_from(start)));
        }

        Err(self.not_implemented())
    }

    /// `suffixedexp -> primaryexp { '.' NAME | '[' expr ']' | ':' NAME args | args }`
    fn suffixed_expr(&mut self) -> Result<ExprId, SyntaxError> {
        let start = self.current.span.start;
        let mut left = self.primary_expr()?;

        loop {
            left = match self.current.kind {
                TokenKind::Byte(b'.') => {
                    self.advance()?;
                    let key = self.name_as_string()?;
                    self.builder
                        .expr(ExprKind::Index { object: left, key }, self.span_from(start))
                }
                TokenKind::Byte(b'[') => {
                    self.advance()?;
                    let key = self.expr()?;
                    if !self.eat_byte(b']')? {
                        return Err(self.not_implemented());
                    }

                    self.builder
                        .expr(ExprKind::Index { object: left, key }, self.span_from(start))
                }
                TokenKind::Byte(b':') => {
                    self.advance()?;
                    let Some(name) = self.current_name() else {
                        return Err(self.not_implemented());
                    };
                    self.advance()?;
                    let args = self.args()?;

                    self.builder.expr(
                        ExprKind::Method {
                            object: left,
                            name,
                            args,
                        },
                        self.span_from(start),
                    )
                }
                TokenKind::Byte(b'(') | TokenKind::Byte(b'{') | TokenKind::Str(_) => {
                    let args = self.args()?;

                    self.builder
                        .expr(ExprKind::Call { callee: left, args }, self.span_from(start))
                }
                _ => return Ok(left),
            };
        }
    }

    /// `args -> '(' [explist] ')' | tablector | STRING`
    fn args(&mut self) -> Result<Box<[ExprId]>, SyntaxError> {
        if self.eat_byte(b'(')? {
            let list = if self.at_byte(b')') {
                Vec::new()
            } else {
                self.expr_list()?
            };

            if !self.eat_byte(b')')? {
                return Err(self.not_implemented());
            }

            return Ok(list.into_boxed_slice());
        }

        if self.at_byte(b'{') {
            let table = self.table()?;
            return Ok(vec![table].into_boxed_slice());
        }

        let literal = match &self.current.kind {
            TokenKind::Str(bytes) => bytes.clone(),
            _ => return Err(self.not_implemented()),
        };
        let span = self.current.span;
        self.advance()?;

        Ok(vec![self.builder.expr(ExprKind::Str(literal), span)].into_boxed_slice())
    }

    /// `tablector -> '{' [field { sep field } [sep]] '}' where `sep` is `,` or `;`
    fn table(&mut self) -> Result<ExprId, SyntaxError> {
        let start = self.current.span.start;
        self.advance()?;

        let mut fields = Vec::new();
        while !self.at_byte(b'}') {
            fields.push(self.field()?);

            if !self.eat_byte(b',')? && !self.eat_byte(b';')? {
                break;
            }
        }

        if !self.eat_byte(b'}')? {
            return Err(self.not_implemented());
        }

        Ok(self.builder.expr(
            ExprKind::Table(fields.into_boxed_slice()),
            self.span_from(start),
        ))
    }

    fn field(&mut self) -> Result<Field<'a>, SyntaxError> {
        if self.eat_byte(b'[')? {
            let key = self.expr()?;
            if !self.eat_byte(b']')? || !self.eat_byte(b'=')? {
                return Err(self.not_implemented());
            }

            return Ok(Field::Keyed {
                key,
                value: self.expr()?,
            });
        }

        // A name is a key only when `=` follows it; otherwise it starts an ordinary expression.
        if let Some(name) = self.current_name()
            && self.peek()?.kind == TokenKind::Byte(b'=')
        {
            self.advance()?;
            self.advance()?;

            return Ok(Field::Named {
                name,
                value: self.expr()?,
            });
        }

        Ok(Field::Positional(self.expr()?))
    }

    /// The name after `.` or in `function a.b`, as the string key it stands for.
    fn name_as_string(&mut self) -> Result<ExprId, SyntaxError> {
        let Some(name) = self.current_name() else {
            return Err(self.not_implemented());
        };
        let span = self.current.span;
        self.advance()?;

        Ok(self
            .builder
            .expr(ExprKind::Str(name.to_vec().into_boxed_slice()), span))
    }
}

fn unary_op(kind: &TokenKind<'_>) -> Option<UnOp> {
    Some(match kind {
        TokenKind::Not => UnOp::Not,
        TokenKind::Byte(b'-') => UnOp::Neg,
        TokenKind::Byte(b'#') => UnOp::Len,
        TokenKind::Byte(b'~') => UnOp::BNot,
        _ => return None,
    })
}

fn binary_op(kind: &TokenKind<'_>) -> Option<BinOp> {
    Some(match kind {
        TokenKind::Byte(b'+') => BinOp::Add,
        TokenKind::Byte(b'-') => BinOp::Sub,
        TokenKind::Byte(b'*') => BinOp::Mul,
        TokenKind::Byte(b'/') => BinOp::Div,
        TokenKind::IDiv => BinOp::IDiv,
        TokenKind::Byte(b'%') => BinOp::Mod,
        TokenKind::Byte(b'^') => BinOp::Pow,
        TokenKind::Concat => BinOp::Concat,
        TokenKind::Eq => BinOp::Eq,
        TokenKind::Ne => BinOp::Ne,
        TokenKind::Byte(b'<') => BinOp::Lt,
        TokenKind::Le => BinOp::Le,
        TokenKind::Byte(b'>') => BinOp::Gt,
        TokenKind::Ge => BinOp::Ge,
        TokenKind::And => BinOp::And,
        TokenKind::Or => BinOp::Or,
        TokenKind::Byte(b'&') => BinOp::BAnd,
        TokenKind::Byte(b'|') => BinOp::BOr,
        TokenKind::Byte(b'~') => BinOp::BXor,
        TokenKind::Shl => BinOp::Shl,
        TokenKind::Shr => BinOp::Shr,
        _ => return None,
    })
}

/// Left and right binding priority. Only `..` and `^` differ between the two.
fn binary_priority(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (1, 1),
        BinOp::And => (2, 2),
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::Ne | BinOp::Eq => (3, 3),
        BinOp::BOr => (4, 4),
        BinOp::BXor => (5, 5),
        BinOp::BAnd => (6, 6),
        BinOp::Shl | BinOp::Shr => (7, 7),
        BinOp::Concat => (9, 8),
        BinOp::Add | BinOp::Sub => (10, 10),
        BinOp::Mul | BinOp::Div | BinOp::IDiv | BinOp::Mod => (11, 11),
        BinOp::Pow => (14, 13),
    }
}
