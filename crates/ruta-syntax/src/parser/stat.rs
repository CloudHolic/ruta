//! Statements and blocks.

use crate::ast::{Attribute, BlockId, ExprKind, StatId, StatKind, VarName};
use crate::error::{SyntaxError, SyntaxErrorKind};
use crate::token::TokenKind;

use super::Parser;

impl<'a> Parser<'a> {
    /// The chunk's own block, which has to reach the end of the source.
    pub(super) fn chunk(&mut self) -> Result<BlockId, SyntaxError> {
        let body = self.block()?;

        if !matches!(self.current.kind, TokenKind::Eof) {
            return Err(self.not_implemented());
        }

        Ok(body)
    }

    /// `block -> { stat } [retstat]`
    pub(super) fn block(&mut self) -> Result<BlockId, SyntaxError> {
        let start = self.current.span.start;
        let mut stats = Vec::new();

        while !self.block_follows() {
            // A return statement closes the block it appears in.
            if matches!(self.current.kind, TokenKind::Return) {
                stats.push(self.return_stat()?);
                break;
            }

            if let Some(stat) = self.statement()? {
                stats.push(stat);
            }
        }

        let span = self.span_from(start);
        Ok(self.builder.block(stats.into_boxed_slice(), span))
    }

    /// The tokens that close a block without being part of it.
    fn block_follows(&self) -> bool {
        matches!(
            self.current.kind,
            TokenKind::Eof
                | TokenKind::End
                | TokenKind::Else
                | TokenKind::Elseif
                | TokenKind::Until
        )
    }

    /// One statement, or nothing at all when it was an empty `;`.
    fn statement(&mut self) -> Result<Option<StatId>, SyntaxError> {
        let start = self.current.span.start;

        let kind = if self.at_global_declaration()? {
            self.global_stat()?
        } else {
            match self.current.kind {
                TokenKind::Byte(b';') => {
                    self.advance()?;
                    return Ok(None);
                }
                TokenKind::If => self.if_stat()?,
                TokenKind::While => {
                    self.advance()?;
                    let condition = self.expr()?;
                    self.expect(TokenKind::Do)?;
                    let body = self.block()?;
                    self.expect(TokenKind::End)?;

                    StatKind::While { condition, body }
                }
                TokenKind::Do => {
                    self.advance()?;
                    let body = self.block()?;
                    self.expect(TokenKind::End)?;

                    StatKind::Do(body)
                }
                TokenKind::For => self.for_stat()?,
                TokenKind::Repeat => {
                    self.advance()?;
                    let body = self.block()?;
                    self.expect(TokenKind::Until)?;

                    StatKind::Repeat {
                        body,
                        condition: self.expr()?,
                    }
                }
                TokenKind::Local => self.local_stat()?,
                TokenKind::DbColon => {
                    self.advance()?;
                    let name = self.name()?;
                    self.expect(TokenKind::DbColon)?;

                    StatKind::Label(name)
                }
                TokenKind::Break => {
                    self.advance()?;
                    StatKind::Break
                }
                TokenKind::Goto => {
                    self.advance()?;
                    StatKind::Goto(self.name()?)
                }
                TokenKind::Function => self.func_stat()?,
                _ => self.expr_stat()?,
            }
        };

        let span = self.span_from(start);
        Ok(Some(self.builder.stat(kind, span)))
    }

    /// `retstat -> 'return' [explist] [';']`
    fn return_stat(&mut self) -> Result<StatId, SyntaxError> {
        let start = self.current.span.start;
        self.advance()?;

        let values = if self.block_follows() || self.at_byte(b';') {
            Vec::new()
        } else {
            self.expr_list()?
        };
        self.eat_byte(b';')?;

        let span = self.span_from(start);
        Ok(self
            .builder
            .stat(StatKind::Return(values.into_boxed_slice()), span))
    }

    /// `ifstat -> 'if' expr 'then' block { 'elseif' expr 'then' block } ['else' block] 'end'`
    fn if_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        let mut arms = Vec::new();

        loop {
            // The first pass consumes `if`, later ones `elseif`.
            self.advance()?;
            let condition = self.expr()?;
            self.expect(TokenKind::Then)?;
            arms.push((condition, self.block()?));

            if !matches!(self.current.kind, TokenKind::Elseif) {
                break;
            }
        }

        let otherwise = if self.eat(TokenKind::Else)? {
            Some(self.block()?)
        } else {
            None
        };
        self.expect(TokenKind::End)?;

        Ok(StatKind::If {
            arms: arms.into_boxed_slice(),
            otherwise,
        })
    }

    /// `forstat -> 'for' (NAME '=' exp ',' exp [',' exp] | NAME { ',' NAME } 'in' explist) 'do' block 'end'`
    fn for_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        self.advance()?;
        let first = self.name()?;

        let kind = if self.eat_byte(b'=')? {
            let start = self.expr()?;
            if !self.eat_byte(b',')? {
                return Err(self.not_implemented());
            }
            let limit = self.expr()?;
            let step = if self.eat_byte(b',')? {
                Some(self.expr()?)
            } else {
                None
            };

            self.expect(TokenKind::Do)?;
            StatKind::NumericFor {
                name: first,
                start,
                limit,
                step,
                body: self.block()?,
            }
        } else {
            let mut names = vec![first];
            while self.eat_byte(b',')? {
                names.push(self.name()?);
            }

            self.expect(TokenKind::In)?;
            let exprs = self.expr_list()?;

            self.expect(TokenKind::Do)?;
            StatKind::GenericFor {
                names: names.into_boxed_slice(),
                exprs: exprs.into_boxed_slice(),
                body: self.block()?,
            }
        };

        self.expect(TokenKind::End)?;
        Ok(kind)
    }

    /// `localstat -> 'local' NAME attrib { ',' NAME attrib } ['=' explist]`
    fn local_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        self.advance()?;

        if matches!(self.current.kind, TokenKind::Function) {
            let start = self.current.span.start;
            self.advance()?;
            let name = self.name()?;

            return Ok(StatKind::LocalFunction {
                name,
                func: self.func_body(start, false)?,
            });
        }

        let default = self.attribute(None)?;
        let mut names = Vec::new();
        let mut closing = false;

        loop {
            let name = self.var_name(default)?;

            if name.attribute == Some(Attribute::Close) {
                if closing {
                    return Err(self.semantic(SyntaxErrorKind::MultipleToBeClosed));
                }
                closing = true;
            }
            names.push(name);
            if !self.eat_byte(b',')? {
                break;
            }
        }

        let values = if self.eat_byte(b'=')? {
            self.expr_list()?
        } else {
            Vec::new()
        };

        Ok(StatKind::Local {
            names: names.into_boxed_slice(),
            values: values.into_boxed_slice(),
        })
    }

    /// `exprstat -> suffixedexp { ',' suffixedexp } '=' explist | suffixdexp`
    fn expr_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        let first = self.suffixed_expr()?;

        if self.at_byte(b'=') || self.at_byte(b',') {
            let mut targets = vec![first];
            while self.eat_byte(b',')? {
                targets.push(self.suffixed_expr()?);
            }

            if !self.eat_byte(b'=')? {
                return Err(self.not_implemented());
            }

            let values = self.expr_list()?;
            return Ok(StatKind::Assign {
                targets: targets.into_boxed_slice(),
                values: values.into_boxed_slice(),
            });
        }

        // Everything else that is not a call is an expression with nowhere to go.
        if !matches!(
            self.builder.kind_of(first),
            ExprKind::Call { .. } | ExprKind::Method { .. }
        ) {
            return Err(self.not_implemented());
        }

        Ok(StatKind::Expr(first))
    }

    /// `globalstat -> 'global' ('function' NAME funcbody
    ///                          | attrib ('*' | NAME attrib { ',' NAME attrib } ['=' explist]))`
    fn global_stat(&mut self) -> Result<StatKind<'a>, SyntaxError> {
        self.advance()?;

        if matches!(self.current.kind, TokenKind::Function) {
            let start = self.current.span.start;
            self.advance()?;
            let name = self.name()?;

            return Ok(StatKind::GlobalFunction {
                name,
                func: self.func_body(start, false)?,
            });
        }

        let default = self.global_attribute(None)?;
        if self.eat_byte(b'*')? {
            return Ok(StatKind::GlobalAll { attribute: default });
        }

        let mut names = Vec::new();
        loop {
            let start = self.current.span.start;
            let name = self.name()?;
            let attribute = self.global_attribute(default)?;

            names.push(VarName {
                name,
                attribute,
                span: self.span_from(start),
            });

            if !self.eat_byte(b',')? {
                break;
            }
        }

        let values = if self.eat_byte(b'=')? {
            self.expr_list()?
        } else {
            Vec::new()
        };

        Ok(StatKind::Global {
            names: names.into_boxed_slice(),
            values: values.into_boxed_slice(),
        })
    }

    /// `global` is an ordinary name until the token after it says otherwise.
    fn at_global_declaration(&mut self) -> Result<bool, SyntaxError> {
        if !matches!(self.current_name(), Some(b"global")) {
            return Ok(false);
        }

        Ok(matches!(
            self.peek()?.kind,
            TokenKind::Name(_)
                | TokenKind::Function
                | TokenKind::Byte(b'<')
                | TokenKind::Byte(b'*')
        ))
    }

    fn var_name(&mut self, default: Option<Attribute>) -> Result<VarName<'a>, SyntaxError> {
        let start = self.current.span.start;
        let name = self.name()?;
        let attribute = self.attribute(default)?;

        Ok(VarName {
            name,
            attribute,
            span: self.span_from(start),
        })
    }

    /// `attrib -> ['<' NAME '>']`.
    fn attribute(&mut self, default: Option<Attribute>) -> Result<Option<Attribute>, SyntaxError> {
        if !self.eat_byte(b'<')? {
            return Ok(default);
        }

        let name = self.name()?;
        if !self.eat_byte(b'>')? {
            return Err(self.not_implemented());
        }

        match name {
            b"const" => Ok(Some(Attribute::Const)),
            b"close" => Ok(Some(Attribute::Close)),
            _ => Err(self.semantic(SyntaxErrorKind::UnknownAttribute(name.into()))),
        }
    }

    /// The same, except that a global can never be to-be-closed.
    fn global_attribute(
        &mut self,
        default: Option<Attribute>,
    ) -> Result<Option<Attribute>, SyntaxError> {
        let attribute = self.attribute(default)?;

        if attribute == Some(Attribute::Close) {
            return Err(self.semantic(SyntaxErrorKind::GlobalToBeClosed));
        }

        Ok(attribute)
    }
}
