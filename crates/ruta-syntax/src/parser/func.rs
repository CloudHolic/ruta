//! Function bodies, and the three forms that carry one.

use crate::ast::{ExprId, ExprKind, Func, FuncId, StatKind, Vararg};
use crate::error::SyntaxError;
use crate::token::TokenKind;

use super::Parser;

impl<'a> Parser<'a> {
    /// `funcstat -> 'function' funcname funcbody`
    /// `funcname -> NAME { '.' NAME } [':' NAME]`
    pub(super) fn func_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        let start = self.current.span.start;
        self.advance()?;

        let name_start = self.current.span.start;
        let name = self.name()?;
        let mut target = self
            .builder
            .expr(ExprKind::Name(name), self.span_from(name_start));
        let mut is_method = false;

        loop {
            let key = if self.eat_byte(b'.')? {
                self.name_as_string()?
            } else if self.eat_byte(b':')? {
                is_method = true;
                self.name_as_string()?
            } else {
                break;
            };

            target = self.builder.expr(
                ExprKind::Index {
                    object: target,
                    key,
                },
                self.span_from(name_start),
            );

            // A colon ends the name: a method cannot be followed by more fields.
            if is_method {
                break;
            }
        }

        Ok(StatKind::Function {
            target,
            func: self.func_body(start, is_method)?,
        })
    }

    /// The `function() ... end` that appears where a value is expected.
    pub(super) fn func_expr(&mut self, start: u32) -> Result<ExprId, SyntaxError> {
        self.advance()?;
        let func = self.func_body(start, false)?;

        Ok(self
            .builder
            .expr(ExprKind::Function(func), self.span_from(start)))
    }

    /// `funcbody -> '(' [parlist] ')' block 'end'`, with `start` at the `function` keyword.
    pub(super) fn func_body(&mut self, start: u32, is_method: bool) -> Result<FuncId, SyntaxError> {
        if !self.eat_byte(b'(')? {
            return Err(self.not_implemented());
        }

        let mut params = Vec::new();
        let mut vararg = None;

        if !self.at_byte(b')') {
            loop {
                // `...` ends the list wherever it appears, and it may be bind a name of its own.
                if self.eat(TokenKind::Dots)? {
                    vararg = Some(match self.current_name() {
                        Some(name) => {
                            self.advance()?;
                            Vararg::Named(name)
                        }
                        None => Vararg::Anonymous,
                    });
                    break;
                }

                params.push(self.name()?);
                if !self.eat_byte(b',')? {
                    break;
                }
            }
        }

        if !self.eat_byte(b')')? {
            return Err(self.not_implemented());
        }

        let body = self.block()?;
        self.expect(TokenKind::End)?;

        let span = self.span_from(start);
        Ok(self.builder.func(Func {
            params: params.into_boxed_slice(),
            vararg,
            is_method,
            body,
            span,
        }))
    }
}
