//! Traversing the tree, one function at a time.

use std::mem;

use ruta_syntax::ast::{Ast, FuncId, StatId, VarId, Vararg as SyntaxVararg};
use ruta_syntax::error::Error;
use ruta_syntax::scope::{Bindings, Capture};
use ruta_syntax::token::Span;

use crate::ir::{
    Block, BlockIdx, FuncIdx, Function, Instr, Op, Position, Program, Reg, Slot, UpvalSource,
    Vararg,
};

/// A slot that is in scope. The closing value a generic `for` binds has no name of its own.
#[derive(Debug)]
pub(super) struct Local {
    pub(super) var: Option<VarId>,
    pub(super) reg: Reg,
    /// A to-be-closed slot. A `return f()` inside its scope is not a tail call.
    pub(super) closes: bool,
    /// Which entry of the function's slot list this fills.
    slot: usize,
}

/// A goto whose label has not been lowered yet, so what dies on the way out is not known.
#[derive(Debug)]
struct Pending {
    label: StatId,
    /// The block that will hold the close and the jump.
    block: BlockIdx,
    captured: Box<[(usize, Reg)]>,
    at: u32,
}

#[derive(Debug)]
struct LabelSite {
    stat: StatId,
    block: BlockIdx,
    /// How many locals are in scope where the label is written. Known once it is reached.
    depth: Option<usize>,
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
    /// Every declaration the body has made, the ones already out of scope included.
    slots: Vec<Slot>,
    /// Where each enclosing loop sends a `break`, and the scope depth it leaves behind.
    pub(super) loops: Vec<(BlockIdx, usize)>,
    /// The block a label statement starts, made when the label is first named.
    labels: Vec<LabelSite>,
    /// Forward gotos still waiting for their label.
    pending: Vec<Pending>,
    blocks: Vec<Block>,
    current: BlockIdx,
}

#[derive(Debug)]
pub(super) struct Lowerer<'a> {
    pub(super) ast: &'a Ast<'a>,
    pub(super) bindings: &'a Bindings,
    program: Program,
    funcs: Vec<FuncState>,
    /// Every local a nested function captures, so that leaving its scope closes it.
    captured: Vec<VarId>,
    /// The first limit the chunk crossed.
    refusal: Option<Error>,
}

impl Lowerer<'_> {
    pub(super) fn state(&mut self) -> &mut FuncState {
        self.funcs.last_mut().expect("inside a function")
    }

    /// Where the function being lowered begins, or `None` for the chunk itself.
    pub(super) fn enclosing(&self) -> Option<u32> {
        let index = self.funcs.last().expect("inside a function").index;

        (index != 0).then(|| self.program.funcs[index].span.start)
    }

    /// Keeps the first limit crossed. Lowering runs on so that the rest of the tree is still walked,
    /// and nothing reads what it produces.
    pub(super) fn refuse(&mut self, error: Error) {
        if self.refusal.is_none() {
            self.refusal = Some(error);
        }
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
        if let Some(site) = self.state().labels.iter().find(|site| site.stat == label) {
            return site.block;
        }

        let block = self.new_block();
        self.state().labels.push(LabelSite {
            stat: label,
            block,
            depth: None,
        });

        block
    }

    pub(super) fn reach_label(&mut self, label: StatId, block: BlockIdx) {
        let depth = self.state().vars.len();
        let state = self.state();

        if let Some(site) = state.labels.iter_mut().find(|site| site.stat == label) {
            site.depth = Some(depth);
        }

        let waiting = mem::take(&mut state.pending);
        let (arriving, rest): (Vec<Pending>, Vec<Pending>) =
            waiting.into_iter().partition(|goto| goto.label == label);
        state.pending = rest;

        for goto in arriving {
            let instrs = &mut state.blocks[goto.block.0 as usize].instrs;

            if let Some(from) = close_from(&goto.captured, depth) {
                instrs.push(Instr {
                    op: Op::CloseUpvals { from },
                    at: goto.at,
                });
            }

            instrs.push(Instr {
                op: Op::Jump { to: block },
                at: goto.at,
            })
        }
    }

    /// A goto closes what dies on the way out.
    /// A forward jump does not know that yet, so it waits in a block of its own.
    pub(super) fn goto(&mut self, label: StatId, at: u32) {
        let captured = self.captured_in_scope();

        if let Some(depth) = self.label_depth(label) {
            if let Some(from) = close_from(&captured, depth) {
                self.emit(Op::CloseUpvals { from }, at);
            }

            let block = self.label_block(label);
            self.emit(Op::Jump { to: block }, at);
        } else if captured.is_empty() {
            let block = self.label_block(label);
            self.emit(Op::Jump { to: block }, at);
        } else {
            let block = self.new_block();
            self.emit(Op::Jump { to: block }, at);
            self.state().pending.push(Pending {
                label,
                block,
                captured,
                at,
            });
        }
    }

    /// Closes the upvalues held by the scopes above `depth`, which are the ones being left.
    pub(super) fn close_upvals(&mut self, depth: usize, at: u32) {
        if self.is_terminated() {
            return;
        }

        if let Some(from) = close_from(&self.captured_in_scope(), depth) {
            self.emit(Op::CloseUpvals { from }, at);
        }
    }

    /// Gives a declaration its register and its place in the scope stack.
    pub(super) fn declare(
        &mut self,
        name: Option<&[u8]>,
        var: Option<VarId>,
        reg: Reg,
        closes: bool,
    ) {
        let enters = self.position();
        let state = self.state();
        let number = state.vars.len() as u32;
        let slot = state.slots.len();

        state.slots.push(Slot {
            name: name.map(Box::from),
            reg,
            number,
            enters,
            leaves: enters,
        });
        state.vars.push(Local {
            var,
            reg,
            closes,
            slot,
        });
    }

    /// Leaves every scope above `depth`.
    pub(super) fn leave(&mut self, depth: usize) {
        let leaves = self.position();
        let state = self.state();
        let leaving: Vec<usize> = state.vars.drain(depth..).map(|local| local.slot).collect();

        for slot in leaving {
            state.slots[slot].leaves = leaves;
        }
    }

    pub(super) fn closure(&mut self, id: FuncId, dest: Reg, at: u32) {
        let ast = self.ast;
        let func = ast.func(id);
        let body = ast.block(func.body);

        let upvalues = self.upvalues(id);
        let params = u16::from(func.self_var.is_some()) + func.params.len() as u16;
        let vararg = match func.vararg {
            None => Vararg::None,
            Some(SyntaxVararg::Anonymous) => Vararg::Anonymous,
            Some(SyntaxVararg::Named(_)) => Vararg::Table,
        };

        let index = self.enter_function(params, vararg, upvalues, func.span);

        if let Some(var) = func.self_var {
            self.bind(b"self", var);
        }

        for param in func.params.iter() {
            self.bind(param.name, param.id);
        }

        if let Some(SyntaxVararg::Named(var)) = func.vararg {
            self.bind(var.name, var.id);
        }

        self.stats(body);

        if !self.is_terminated() {
            self.emit(
                Op::Return {
                    values: Box::new([]),
                    spread: false,
                },
                body.close_at,
            );
        }

        self.leave_function();
        self.emit(
            Op::Closure {
                dest,
                func: FuncIdx(index as u32),
            },
            at,
        );
    }

    fn label_depth(&mut self, label: StatId) -> Option<usize> {
        self.state()
            .labels
            .iter()
            .find(|site| site.stat == label)?
            .depth
    }

    fn position(&mut self) -> Position {
        let state = self.state();
        let block = state.current;

        Position {
            block,
            instr: state.blocks[block.0 as usize].instrs.len() as u32,
        }
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
            slots: Vec::new(),
            span,
        });
        self.funcs.push(FuncState {
            index,
            regs: 0,
            vars: Vec::new(),
            slots: Vec::new(),
            loops: Vec::new(),
            labels: Vec::new(),
            pending: Vec::new(),
            blocks: vec![Block::default()],
            current: BlockIdx(0),
        });

        index
    }

    fn leave_function(&mut self) {
        self.leave(0);

        let state = self.funcs.pop().expect("inside a function");
        debug_assert!(state.pending.is_empty());

        let func = &mut self.program.funcs[state.index];

        func.blocks = state.blocks;
        func.regs = state.regs;
        func.slots = state.slots;
    }

    /// The captured locals in scope, each with how deep it sits in the scope stack.
    fn captured_in_scope(&self) -> Box<[(usize, Reg)]> {
        let captured = &self.captured;

        self.funcs
            .last()
            .expect("inside a function")
            .vars
            .iter()
            .enumerate()
            .filter(|(_, local)| local.var.is_some_and(|var| captured.contains(&var)))
            .map(|(scope, local)| (scope, local.reg))
            .collect()
    }

    fn main(&mut self) {
        debug_assert!(matches!(
            self.bindings.main().upvalues.as_ref(),
            [Capture::Env]
        ));

        let ast = self.ast;
        let main = ast.main_block();

        self.enter_function(
            0,
            Vararg::Anonymous,
            vec![UpvalSource::Env],
            Span::new(main.span.start, ast.ends()),
        );
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

    /// Gives a parameter its register, in the order the frame receives them.
    fn bind(&mut self, name: &[u8], var: VarId) {
        let reg = self.reg();
        self.declare(Some(name), Some(var), reg, false);
    }

    /// Reads the captures in the enclosing function's terms, which is where this runs.
    fn upvalues(&mut self, id: FuncId) -> Vec<UpvalSource> {
        let bindings = self.bindings;

        bindings
            .function(id)
            .upvalues
            .iter()
            .map(|capture| match capture {
                Capture::ParentLocal(var) => UpvalSource::ParentLocal(self.lookup(*var)),
                Capture::ParentUpvalue(index) => UpvalSource::ParentUpval(*index),
                Capture::Env => UpvalSource::Env,
            })
            .collect()
    }
}

pub fn lower(ast: &Ast<'_>, bindings: &Bindings) -> Result<Program, Error> {
    let mut lowerer = Lowerer {
        ast,
        bindings,
        captured: bindings.captured().collect(),
        program: Program::default(),
        funcs: Vec::new(),
        refusal: None,
    };

    lowerer.main();

    match lowerer.refusal {
        Some(error) => Err(error),
        None => Ok(lowerer.program),
    }
}

/// The lowest register a captured local occupies above `depth`.
/// Registers grow with the scope stack, so closing form there leaves the outer scopes untouched.
fn close_from(captured: &[(usize, Reg)], depth: usize) -> Option<Reg> {
    captured
        .iter()
        .find(|(scope, _)| *scope >= depth)
        .map(|(_, reg)| *reg)
}
