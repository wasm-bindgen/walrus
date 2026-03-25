//! The `walrus` WebAssembly transformations library.
//!
//! # GC Proposal Support
//!
//! Walrus supports the [WebAssembly GC proposal](https://github.com/WebAssembly/gc),
//! including struct types, array types, subtyping, recursive type groups, and the
//! full set of GC instructions.
//!
//! ## Creating GC types
//!
//! Use [`ModuleTypes::add_struct`], [`ModuleTypes::add_array`], and
//! [`ModuleTypes::add_composite`] to create GC types programmatically. For
//! mutually-recursive types, use [`ModuleTypes::add_rec_group`] which
//! pre-allocates type IDs for forward references.
//!
//! ## GC instructions
//!
//! All GC instructions (`struct.new`, `array.get`, `ref.cast`, etc.) are
//! available as builder methods on [`InstrSeqBuilder`]. See the [`ir`] module
//! for the full list of instruction variants.

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

#[cfg(feature = "parallel")]
macro_rules! maybe_parallel {
    ($e:ident.($serial:ident | $parallel:ident)) => {
        $e.$parallel()
    };
}

#[cfg(not(feature = "parallel"))]
macro_rules! maybe_parallel {
    ($e:ident.($serial:ident | $parallel:ident)) => {
        $e.$serial()
    };
}

mod arena_set;
mod const_expr;
pub mod dot;
mod emit;
mod error;
mod function_builder;
pub mod ir;
mod map;
mod module;
mod parse;
pub mod passes;
mod tombstone_arena;
mod ty;

pub use crate::const_expr::{ConstExpr, ConstOp};
pub use crate::emit::IdsToIndices;
pub use crate::error::{ErrorKind, Result};
pub use crate::function_builder::{FunctionBuilder, InstrSeqBuilder};
pub use crate::ir::{Local, LocalId};
pub use crate::module::*;
pub use crate::parse::IndicesToIds;
pub use crate::ty::{
    AbstractHeapType, ArrayType, CompositeType, FieldType, FunctionType, HeapType, RecGroup,
    RecGroupId, RefType, StorageType, StructType, Type, TypeId, ValType,
};
