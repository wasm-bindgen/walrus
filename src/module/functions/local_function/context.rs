//! Context needed when validating instructions and constructing our `Instr` IR.

use crate::error::{ErrorKind, Result};
use crate::ir::{BlockKind, Instr, InstrLocId, InstrSeq, InstrSeqId, InstrSeqType};
use crate::module::functions::{FunctionId, LocalFunction};
use crate::module::Module;
use crate::parse::IndicesToIds;
use crate::ty::ValType;
use crate::{ModuleTypes, TypeId};
use anyhow::Context;

#[derive(Debug)]
pub(crate) struct ControlFrame {
    /// The parameter types of the block (checked before entering the block).
    pub start_types: Box<[ValType]>,

    /// The result type of the block (used to check its result).
    pub end_types: Box<[ValType]>,

    /// If `true`, then this frame is unreachable. This is used to handle
    /// stack-polymorphic typing after unconditional branches.
    pub unreachable: bool,

    /// The id of this control frame's block.
    pub block: InstrSeqId,

    /// This control frame's kind of block, eg loop vs block vs if/else.
    pub kind: BlockKind,

    /// Where this block's instructions start in `ValidationContext::instrs`.
    pub start: usize,
}

/// The control frame stack.
pub(crate) type ControlStack = Vec<ControlFrame>;

#[derive(Debug)]
pub(crate) struct ValidationContext<'a> {
    /// The module that we're adding a function for.
    pub module: &'a Module,

    /// Mapping of indexes back to ids.
    pub indices: &'a IndicesToIds,

    /// The arena id of `func`.
    pub func_id: FunctionId,

    /// The function being validated/constructed.
    pub func: &'a mut LocalFunction,

    /// The control frames stack.
    pub controls: &'a mut ControlStack,

    /// The instructions of every open block, innermost last. Each frame's
    /// instructions start at its `start`; they are moved into the frame's
    /// `InstrSeq`, at their exact size, when the frame is popped. This avoids
    /// growing every block's `Vec` one push at a time.
    pub instrs: &'a mut Vec<(Instr, InstrLocId)>,

    /// If we're currently parsing an if/else instruction, where we're at
    pub if_else: Vec<IfElseState>,
}

#[derive(Debug)]
pub struct IfElseState {
    pub start: InstrLocId,
    pub consequent: InstrSeqId,
    pub alternative: Option<InstrSeqId>,
}

impl<'a> ValidationContext<'a> {
    /// Create a new function context.
    pub fn new(
        module: &'a Module,
        indices: &'a IndicesToIds,
        func_id: FunctionId,
        func: &'a mut LocalFunction,
        controls: &'a mut ControlStack,
        instrs: &'a mut Vec<(Instr, InstrLocId)>,
    ) -> ValidationContext<'a> {
        debug_assert!(instrs.is_empty());
        ValidationContext {
            module,
            indices,
            func_id,
            func,
            controls,
            instrs,
            if_else: Vec::new(),
        }
    }

    pub fn push_control(
        &mut self,
        kind: BlockKind,
        start_types: Box<[ValType]>,
        end_types: Box<[ValType]>,
    ) -> Result<InstrSeqId> {
        impl_push_control(
            &self.module.types,
            kind,
            self.func,
            self.controls,
            self.instrs.len(),
            start_types,
            end_types,
        )
    }

    pub fn push_control_with_ty(&mut self, kind: BlockKind, ty: TypeId) -> InstrSeqId {
        let (start_types, end_types) = self.module.types.params_results(ty);
        let start_types: Box<[_]> = start_types.into();
        let end_types: Box<[_]> = end_types.into();
        impl_push_control_with_ty(
            &self.module.types,
            kind,
            self.func,
            self.controls,
            self.instrs.len(),
            ty.into(),
            start_types,
            end_types,
        )
    }

    pub fn pop_control(&mut self) -> Result<(ControlFrame, InstrSeqId)> {
        let frame = impl_pop_control(self.controls)?;
        let block = frame.block;
        self.func.block_mut(block).instrs = self.instrs.split_off(frame.start);
        Ok((frame, block))
    }

    pub fn unreachable(&mut self) {
        let frame = self.controls.last_mut().unwrap();
        frame.unreachable = true;
    }

    pub fn control(&self, n: usize) -> Result<&ControlFrame> {
        if n >= self.controls.len() {
            anyhow::bail!("jump to nonexistent control block");
        }
        let idx = self.controls.len() - n - 1;
        Ok(&self.controls[idx])
    }

    pub fn alloc_instr_in_control(
        &mut self,
        control: usize,
        instr: impl Into<Instr>,
        loc: InstrLocId,
    ) -> Result<()> {
        let frame = self.control(control)?;
        if frame.unreachable {
            return Ok(());
        }
        // Frames inside `control` have no instructions yet (this is used to
        // add a block instruction to its parent right after pushing the
        // block's frame), so the parent's instructions end at the top of
        // `instrs`. Push there and move the inner frames' starts past it.
        let inner = self.controls.len() - control..;
        debug_assert!(self.controls[inner.clone()]
            .iter()
            .all(|f| f.start == self.instrs.len()));
        self.instrs.push((instr.into(), loc));
        for f in &mut self.controls[inner] {
            f.start += 1;
        }
        Ok(())
    }

    pub fn alloc_instr(&mut self, instr: impl Into<Instr>, loc: InstrLocId) {
        self.alloc_instr_in_control(0, instr, loc).unwrap();
    }

    pub fn add_legacy_catch(&mut self, catch: crate::ir::LegacyCatch) -> Result<()> {
        // Find the most recent Try instruction in the parent control block
        let frame = self.control(1)?; // Parent block, not the try block itself

        // A `try` in unreachable code was never added to its parent block
        // (`alloc_instr_in_control` drops it), so its clauses have no `Try`
        // to attach to and are dropped along with it.
        if frame.unreachable {
            return Ok(());
        }
        let parent_start = frame.start;

        // The Try instruction should be the last instruction in the parent
        // block, which ends where the innermost frame starts: the try body for
        // `delegate`, or the new (still empty) handler for `catch`.
        let end = self.controls.last().unwrap().start;
        if end > parent_start {
            if let (Instr::Try(ref mut try_instr), _) = self.instrs[end - 1] {
                try_instr.catches.push(catch);
                return Ok(());
            }
        }

        anyhow::bail!("No Try instruction found to add catch clause to");
    }
}

#[allow(clippy::too_many_arguments)]
fn impl_push_control(
    types: &ModuleTypes,
    kind: BlockKind,
    func: &mut LocalFunction,
    controls: &mut ControlStack,
    start: usize,
    start_types: Box<[ValType]>,
    end_types: Box<[ValType]>,
) -> Result<InstrSeqId> {
    let ty = InstrSeqType::existing(types, &start_types, &end_types).ok_or_else(|| {
        anyhow::anyhow!(
            "attempted to push a control frame for an instruction \
             sequence with a type that does not exist"
        )
        .context(format!("type: {:?} -> {:?}", &start_types, &end_types))
    })?;

    Ok(impl_push_control_with_ty(
        types,
        kind,
        func,
        controls,
        start,
        ty,
        start_types,
        end_types,
    ))
}

#[allow(clippy::too_many_arguments)]
fn impl_push_control_with_ty(
    types: &ModuleTypes,
    kind: BlockKind,
    func: &mut LocalFunction,
    controls: &mut ControlStack,
    start: usize,
    ty: InstrSeqType,
    start_types: Box<[ValType]>,
    end_types: Box<[ValType]>,
) -> InstrSeqId {
    if let InstrSeqType::MultiValue(ty) = ty {
        debug_assert_eq!(types.params(ty), &start_types[..]);
        debug_assert_eq!(types.results(ty), &end_types[..]);
    }

    let block = func.add_block(|id| InstrSeq::new(id, ty));

    controls.push(ControlFrame {
        start_types,
        end_types,
        unreachable: false,
        block,
        kind,
        start,
    });

    block
}

fn impl_pop_control(controls: &mut ControlStack) -> Result<ControlFrame> {
    controls
        .last()
        .ok_or(ErrorKind::InvalidWasm)
        .context("attempted to pop a frame from an empty control stack")?;
    let frame = controls.pop().unwrap();
    Ok(frame)
}
