//! Context needed when validating instructions and constructing our `Instr` IR.

use crate::error::{ErrorKind, Result};
use crate::ir::{BlockKind, Instr, InstrLocId, InstrSeq, InstrSeqId, InstrSeqType};
use crate::module::functions::{FunctionId, LocalFunction};
use crate::module::Module;
use crate::parse::IndicesToIds;
use crate::ty::ValType;
use crate::ModuleTypes;
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

    /// If we're currently parsing an if/else instruction, where we're at
    pub if_else: Vec<IfElseState>,

    /// Multi-value legacy handler blocks with a placeholder type, with their
    /// params and results, in operator order. See `push_legacy_handler`.
    pub legacy_handler_tys: Vec<LegacyHandlerTy>,
}

/// A legacy handler block whose `MultiValue` type is resolved after parsing:
/// the block, its params and its results.
pub(crate) type LegacyHandlerTy = (InstrSeqId, Box<[ValType]>, Box<[ValType]>);

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
    ) -> ValidationContext<'a> {
        ValidationContext {
            module,
            indices,
            func_id,
            func,
            controls,
            if_else: Vec::new(),
            legacy_handler_tys: Vec::new(),
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
            start_types,
            end_types,
        )
    }

    /// Push the function entry frame.
    ///
    /// The entry block's type is a `MultiValue` entry `Type` that may not
    /// exist yet, because bodies are parsed against a shared `&Module`. The
    /// block gets a placeholder type that `parse_local_functions` replaces
    /// after parsing.
    pub fn push_entry_control(&mut self, results: Box<[ValType]>) -> InstrSeqId {
        impl_push_control_with_ty(
            &self.module.types,
            BlockKind::FunctionEntry,
            self.func,
            self.controls,
            InstrSeqType::Simple(None),
            Box::new([]),
            results,
        )
    }

    /// Push the frame of a legacy `catch` / `catch_all` handler.
    ///
    /// A handler's `[tag params] -> [try results]` signature is implicit in
    /// the encoding, so when it's multi-value its `Type` may not exist yet.
    /// Such a block gets a placeholder type and is recorded in
    /// `legacy_handler_tys`, for `parse_local_functions` to register and
    /// resolve after parsing.
    pub fn push_legacy_handler(
        &mut self,
        kind: BlockKind,
        start_types: Box<[ValType]>,
        end_types: Box<[ValType]>,
    ) -> InstrSeqId {
        let simple = matches!((start_types.len(), end_types.len()), (0, 0) | (0, 1));
        if simple {
            return self.push_control(kind, start_types, end_types).unwrap();
        }
        let params = start_types.clone();
        let results = end_types.clone();
        let block = impl_push_control_with_ty(
            &self.module.types,
            kind,
            self.func,
            self.controls,
            InstrSeqType::Simple(None),
            start_types,
            end_types,
        );
        self.legacy_handler_tys.push((block, params, results));
        block
    }

    pub fn pop_control(&mut self) -> Result<(ControlFrame, InstrSeqId)> {
        let frame = impl_pop_control(self.controls)?;
        let block = frame.block;
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

    pub fn alloc_instr_in_block(
        &mut self,
        block: InstrSeqId,
        instr: impl Into<Instr>,
        loc: InstrLocId,
    ) {
        self.func.block_mut(block).instrs.push((instr.into(), loc));
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
        let block = frame.block;
        self.alloc_instr_in_block(block, instr, loc);
        Ok(())
    }

    pub fn alloc_instr(&mut self, instr: impl Into<Instr>, loc: InstrLocId) {
        self.alloc_instr_in_control(0, instr, loc).unwrap();
    }

    pub fn add_legacy_catch(&mut self, catch: crate::ir::LegacyCatch) -> Result<()> {
        // Find the most recent Try instruction in the parent control block
        let frame = self.control(1)?; // Parent block, not the try block itself
        let block = frame.block;
        let seq = self.func.block_mut(block);

        // The Try instruction should be the last instruction in the parent block
        if let Some((Instr::Try(ref mut try_instr), _)) = seq.instrs.last_mut() {
            try_instr.catches.push(catch);
            return Ok(());
        }

        anyhow::bail!("No Try instruction found to add catch clause to");
    }
}

fn impl_push_control(
    types: &ModuleTypes,
    kind: BlockKind,
    func: &mut LocalFunction,
    controls: &mut ControlStack,
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
        ty,
        start_types,
        end_types,
    ))
}

fn impl_push_control_with_ty(
    types: &ModuleTypes,
    kind: BlockKind,
    func: &mut LocalFunction,
    controls: &mut ControlStack,
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
