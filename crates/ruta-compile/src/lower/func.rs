//! Traversing the tree, one function at a time.

use ruta_syntax::ast::{Ast, StatId, VarId};
use ruta_syntax::scope::{Bindings, Capture};
use ruta_syntax::token::Span;

use crate::ir::{Block, BlockIdx, Function, Instr, Op, Program, Reg, UpvalSource, Vararg};

/// A slot that is in scope. The closing value a generic `for` binds has no name of its own.
#[derive(Debug)]
pub(super) struct Local {
    pub(super) var: Option<VarId>,
    pub(super) reg: Reg,
    /// A to-be-closed slot. A `return f()` inside its scope is not a tail call.
    pub(super) closes: bool,
}

/// One function being built.
/// The stack of these is what lets a nested function be lowered to completion in the middle of its parent.
#[derive(Debug)]
pub(super) struct FuncState {
    /// Which entry of [`Program::funcs`] this fills.
    index: usize,
    regs: u32,
    /// The locals in scope, innermost last. A name resolves to the last match.
    pub(super) vars: Vec<Local>,
    /// Where each enclosing loop sends a `brea`, innermost last.
    pub(super) loops: Vec<BlockIdx>,
    /// The block a label statement starts, made when the label is first named.
    labels: Vec<(StatId, BlockIdx)>,
    blocks: Vec<Block>,
    current: BlockIdx,
}

#[derive(Debug)]
pub(super) struct Lowerer<'a> {
    pub(super) ast: &'a Ast<'a>,
    pub(super) bindings: &'a Bindings,
    program: Program,
    funcs: Vec<FuncState>,
}

impl Lowerer<'_> {
    pub(super) fn state(&mut self) -> &mut FuncState {
        self.funcs.last_mut().expect("inside a function")
    }

    pub(super) fn reg(&mut self) -> Reg {
        let state = self.state();
        state.regs += 1;

        Reg(state.regs - 1)
    }

    pub(super) fn new_block(&mut self) -> BlockIdx {
        let state = self.state();
        state.blocks.push(Block::default());

        BlockIdx(state.blocks.len() as u32 - 1)
    }

    pub(super) fn switch_to(&mut self, block: BlockIdx) {
        self.state().current = block;
    }

    pub(super) fn is_terminated(&mut self) -> bool {
        let state = self.state();
        let current = state.current.0 as usize;

        state.blocks[current]
            .instrs
            .last()
            .is_some_and(|instr| instr.op.is_terminator())
    }

    pub(super) fn emit(&mut self, op: Op, at: u32) {
        debug_assert!(!self.is_terminated());

        let state = self.state();
        let current = state.current.0 as usize;
        state.blocks[current].instrs.push(Instr { op, at });
    }

    /// Leave the block alone when control cannot reach the end of it.
    pub(super) fn jump_to(&mut self, block: BlockIdx, at: u32) {
        if !self.is_terminated() {
            self.emit(Op::Jump { to: block }, at);
        }
    }

    /// The block a label starts. A goto that runs before the label is written makes it.
    pub(super) fn label_block(&mut self, label: StatId) -> BlockIdx {
        if let Some((_, block)) = self.state().labels.iter().find(|(id, _)| *id == label) {
            return *block;
        }

        let block = self.new_block();
        self.state().labels.push((label, block));

        block
    }

    /// Claims this function's entry in the program, so that a child can claim the next one.
    fn enter_function(
        &mut self,
        params: u16,
        vararg: Vararg,
        upvalues: Vec<UpvalSource>,
        span: Span,
    ) -> usize {
        let index = self.program.funcs.len();

        self.program.funcs.push(Function {
            params,
            vararg,
            blocks: Vec::new(),
            regs: 0,
            upvalues,
            span,
        });
        self.funcs.push(FuncState {
            index,
            regs: 0,
            vars: Vec::new(),
            loops: Vec::new(),
            labels: Vec::new(),
            blocks: vec![Block::default()],
            current: BlockIdx(0),
        });

        index
    }

    fn leave_function(&mut self) {
        let state = self.funcs.pop().expect("inside a function");
        let func = &mut self.program.funcs[state.index];

        func.blocks = state.blocks;
        func.regs = state.regs;
    }

    fn main(&mut self) {
        debug_assert!(matches!(
            self.bindings.main().upvalues.as_ref(),
            [Capture::Env]
        ));

        let ast = self.ast;
        let main = ast.main_block();

        self.enter_function(0, Vararg::Anonymous, vec![UpvalSource::Env], main.span);
        self.stats(main);

        if !self.is_terminated() {
            self.emit(
                Op::Return {
                    values: Box::new([]),
                    spread: false,
                },
                main.close_at,
            );
        }

        self.leave_function();
    }
}

pub fn lower(ast: &Ast<'_>, bindings: &Bindings) -> Program {
    let mut lowerer = Lowerer {
        ast,
        bindings,
        program: Program::default(),
        funcs: Vec::new(),
    };

    lowerer.main();
    lowerer.program
}
