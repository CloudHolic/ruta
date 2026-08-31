//! Block and function frames, and the labels they hold.

use crate::error::{Error, ErrorKind};

use super::{Resolver, error};

#[derive(Debug)]
pub(super) struct BlockFrame<'src> {
    /// How tail the declaration stack was when the block opened.
    pub(super) declarations: usize,
    pub(super) labels: Vec<Label<'src>>,
    /// Gotos that have not found a label yet. Settled when the block closes.
    pub(super) gotos: Vec<Goto<'src>>,
}

#[derive(Debug)]
pub(super) struct Label<'src> {
    name: &'src [u8],
    /// The offset a later collision names by line.
    at: u32,
    /// How many declarations are alive where the label sits.
    declarations: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Goto<'src> {
    pub(super) name: &'src [u8],
    /// The offset the message names by line.
    pub(super) at: u32,
    /// How many declarations were alive where the goto was written.
    pub(super) declarations: usize,
}

/// What is closing.
#[derive(Debug, Clone, Copy)]
pub(super) enum Exit {
    Block,
    Function,
}

impl<'src> Resolver<'_, 'src> {
    pub(super) fn enter_block(&mut self) {
        self.blocks.push(BlockFrame {
            declarations: self.declarations.len(),
            labels: Vec::new(),
            gotos: Vec::new(),
        })
    }

    pub(super) fn leave_block(&mut self, close_at: u32, exit: Exit) -> Result<(), Error> {
        let frame = self.blocks.pop().expect("inside a block");

        // A jump into a declaration's scope beats a goto that found no label at all.
        for goto in frame.gotos.iter() {
            let Some(label) = frame.labels.iter().find(|label| label.name == goto.name) else {
                continue;
            };

            if label.declarations > goto.declarations {
                let variable = self.declarations[goto.declarations].name.unwrap_or(b"*");

                return Err(error(
                    ErrorKind::JumpIntoScope {
                        label: goto.name.into(),
                        goto_at: goto.at,
                        variable: variable.into(),
                    },
                    close_at,
                ));
            }
        }

        let mut unresolved = frame
            .gotos
            .iter()
            .filter(|goto| !frame.labels.iter().any(|label| label.name == goto.name));

        match exit {
            Exit::Function => {
                if let Some(goto) = unresolved.next() {
                    return Err(error(
                        ErrorKind::NoVisibleLabel {
                            name: goto.name.into(),
                            goto_at: goto.at,
                        },
                        close_at,
                    ));
                }
            }
            Exit::Block => {
                let escaping: Vec<_> = unresolved
                    .map(|goto| Goto {
                        declarations: goto.declarations.min(frame.declarations),
                        ..*goto
                    })
                    .collect();

                self.blocks
                    .last_mut()
                    .expect("a block outside a function boundary")
                    .gotos
                    .extend(escaping);
            }
        }

        self.declarations.truncate(frame.declarations);

        Ok(())
    }

    pub(super) fn enter_function(&mut self) {
        self.functions.push(self.blocks.len());
        self.enter_block();
    }

    pub(super) fn leave_function(&mut self, close_at: u32) -> Result<(), Error> {
        self.leave_block(close_at, Exit::Function)?;
        self.functions.pop();

        Ok(())
    }

    pub(super) fn declare_label(
        &mut self,
        name: &'src [u8],
        at: u32,
        report_at: u32,
        tail: bool,
    ) -> Result<(), Error> {
        let floor = self.functions.last().copied().unwrap_or(0);

        for frame in self.blocks[floor..].iter() {
            if let Some(first) = frame.labels.iter().find(|label| label.name == name) {
                return Err(error(
                    ErrorKind::LabelAlreadyDefined {
                        name: name.into(),
                        first_at: first.at,
                    },
                    report_at,
                ));
            }
        }

        let height = self.declarations.len();
        let frame = self.blocks.last_mut().expect("inside a block");
        let declarations = if tail { frame.declarations } else { height };

        frame.labels.push(Label {
            name,
            at,
            declarations,
        });

        Ok(())
    }
}
