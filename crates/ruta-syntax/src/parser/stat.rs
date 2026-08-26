//! Statements and blocks.

use crate::ast::{BlockId, StatId, StatKind};
use crate::error::SyntaxError;
use crate::token::TokenKind;

use super::Parser;

impl<'a> Parser<'a> {
    /// The chunk's own block.
    pub(super) fn chunk(&mut self) -> Result<BlockId, SyntaxError> {
        let start = self.current.span.start;
        let mut stats = Vec::new();

        if matches!(self.current.kind, TokenKind::Return) {
            stats.push(self.return_stat()?);
        }

        if !matches!(self.current.kind, TokenKind::Eof) {
            return Err(self.not_implemented());
        }

        let span = self.span_from(start);
        Ok(self.builder.block(stats.into_boxed_slice(), span))
    }

    /// `retstat` -> `return` [explist] [';']`
    fn return_stat(&mut self) -> Result<StatId, SyntaxError> {
        let start = self.current.span.start;
        self.advance()?;

        let values = if matches!(self.current.kind, TokenKind::Eof) || self.at_byte(b';') {
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
}
