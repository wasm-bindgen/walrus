//! Handling wasm constant values

use crate::emit::EmitContext;
use crate::ir::Value;
use crate::parse::IndicesToIds;
use crate::RefType;
use crate::{FunctionId, GlobalId, Result};
use anyhow::bail;

/// A constant which is produced in WebAssembly, typically used in global
/// initializers or element/data offsets.
#[derive(Debug, Clone)]
pub enum ConstExpr {
    /// An immediate constant value
    Value(Value),
    /// A constant value referenced by the global specified
    Global(GlobalId),
    /// A null reference
    RefNull(RefType),
    /// A function initializer
    RefFunc(FunctionId),
    /// Extended constant expression (sequence of instructions)
    Extended(Vec<ConstOp>),
}

/// Operations allowed in extended constant expressions
#[derive(Debug, Copy, Clone)]
pub enum ConstOp {
    /// An i32 constant value
    I32Const(i32),
    /// An i64 constant value
    I64Const(i64),
    /// An f32 constant value
    F32Const(f32),
    /// An f64 constant value
    F64Const(f64),
    /// A v128 constant value
    V128Const(u128),
    /// Get the value of a global
    GlobalGet(GlobalId),
    /// A null reference
    RefNull(RefType),
    /// A function reference
    RefFunc(FunctionId),
    /// i32 addition
    I32Add,
    /// i32 subtraction
    I32Sub,
    /// i32 multiplication
    I32Mul,
    /// i64 addition
    I64Add,
    /// i64 subtraction
    I64Sub,
    /// i64 multiplication
    I64Mul,
}

impl ConstExpr {
    pub(crate) fn eval(init: &wasmparser::ConstExpr, ids: &IndicesToIds) -> Result<ConstExpr> {
        use wasmparser::Operator::*;
        let mut reader = init.get_operators_reader();
        let mut ops = Vec::new();

        loop {
            let op = reader.read()?;
            match op {
                End => break,
                I32Const { value } => ops.push(ConstOp::I32Const(value)),
                I64Const { value } => ops.push(ConstOp::I64Const(value)),
                F32Const { value } => ops.push(ConstOp::F32Const(f32::from_bits(value.bits()))),
                F64Const { value } => ops.push(ConstOp::F64Const(f64::from_bits(value.bits()))),
                V128Const { value } => ops.push(ConstOp::V128Const(v128_to_u128(&value))),
                GlobalGet { global_index } => {
                    ops.push(ConstOp::GlobalGet(ids.get_global(global_index)?))
                }
                RefNull { hty } => {
                    let val_type = match hty {
                        wasmparser::HeapType::Abstract { shared: _, ty } => match ty {
                            wasmparser::AbstractHeapType::Func => RefType::FUNCREF,
                            wasmparser::AbstractHeapType::Extern => RefType::EXTERNREF,
                            other => bail!(
                                "unsupported abstract heap type in constant expression: {other:?}"
                            ),
                        },
                        wasmparser::HeapType::Concrete(_) => {
                            bail!("unsupported concrete heap type in constant expression")
                        }
                    };
                    ops.push(ConstOp::RefNull(val_type));
                }
                RefFunc { function_index } => {
                    ops.push(ConstOp::RefFunc(ids.get_func(function_index)?))
                }
                I32Add => ops.push(ConstOp::I32Add),
                I32Sub => ops.push(ConstOp::I32Sub),
                I32Mul => ops.push(ConstOp::I32Mul),
                I64Add => ops.push(ConstOp::I64Add),
                I64Sub => ops.push(ConstOp::I64Sub),
                I64Mul => ops.push(ConstOp::I64Mul),
                _ => bail!("unsupported operation in constant expression: {:?}", op),
            }
        }

        reader.finish()?;

        // Optimize: if there's only one simple operation, use the simple form
        if ops.len() == 1 {
            match &ops[0] {
                ConstOp::I32Const(v) => return Ok(ConstExpr::Value(Value::I32(*v))),
                ConstOp::I64Const(v) => return Ok(ConstExpr::Value(Value::I64(*v))),
                ConstOp::F32Const(v) => return Ok(ConstExpr::Value(Value::F32(*v))),
                ConstOp::F64Const(v) => return Ok(ConstExpr::Value(Value::F64(*v))),
                ConstOp::V128Const(v) => return Ok(ConstExpr::Value(Value::V128(*v))),
                ConstOp::GlobalGet(g) => return Ok(ConstExpr::Global(*g)),
                ConstOp::RefNull(ty) => return Ok(ConstExpr::RefNull(*ty)),
                ConstOp::RefFunc(f) => return Ok(ConstExpr::RefFunc(*f)),
                _ => {}
            }
        }

        Ok(ConstExpr::Extended(ops))
    }

    pub(crate) fn to_wasmencoder_type(&self, cx: &EmitContext) -> wasm_encoder::ConstExpr {
        use wasm_encoder::{Encode, Instruction};
        match self {
            ConstExpr::Value(v) => match v {
                Value::I32(v) => wasm_encoder::ConstExpr::i32_const(*v),
                Value::I64(v) => wasm_encoder::ConstExpr::i64_const(*v),
                Value::F32(v) => wasm_encoder::ConstExpr::f32_const((*v).into()),
                Value::F64(v) => wasm_encoder::ConstExpr::f64_const((*v).into()),
                Value::V128(v) => wasm_encoder::ConstExpr::v128_const(*v as i128),
            },
            ConstExpr::Global(g) => {
                wasm_encoder::ConstExpr::global_get(cx.indices.get_global_index(*g))
            }
            ConstExpr::RefNull(ty) => wasm_encoder::ConstExpr::ref_null(ty.heap_type.into()),
            ConstExpr::RefFunc(f) => {
                wasm_encoder::ConstExpr::ref_func(cx.indices.get_func_index(*f))
            }
            ConstExpr::Extended(ops) => {
                let mut bytes = Vec::new();
                for op in ops {
                    match op {
                        ConstOp::I32Const(v) => Instruction::I32Const(*v).encode(&mut bytes),
                        ConstOp::I64Const(v) => Instruction::I64Const(*v).encode(&mut bytes),
                        ConstOp::F32Const(v) => {
                            Instruction::F32Const((*v).into()).encode(&mut bytes)
                        }
                        ConstOp::F64Const(v) => {
                            Instruction::F64Const((*v).into()).encode(&mut bytes)
                        }
                        ConstOp::V128Const(v) => {
                            Instruction::V128Const(*v as i128).encode(&mut bytes)
                        }
                        ConstOp::GlobalGet(g) => {
                            Instruction::GlobalGet(cx.indices.get_global_index(*g))
                                .encode(&mut bytes)
                        }
                        ConstOp::RefNull(ty) => {
                            Instruction::RefNull(ty.heap_type.into()).encode(&mut bytes)
                        }
                        ConstOp::RefFunc(f) => {
                            Instruction::RefFunc(cx.indices.get_func_index(*f)).encode(&mut bytes)
                        }
                        ConstOp::I32Add => Instruction::I32Add.encode(&mut bytes),
                        ConstOp::I32Sub => Instruction::I32Sub.encode(&mut bytes),
                        ConstOp::I32Mul => Instruction::I32Mul.encode(&mut bytes),
                        ConstOp::I64Add => Instruction::I64Add.encode(&mut bytes),
                        ConstOp::I64Sub => Instruction::I64Sub.encode(&mut bytes),
                        ConstOp::I64Mul => Instruction::I64Mul.encode(&mut bytes),
                    }
                }
                // Don't add End instruction - wasm_encoder::ConstExpr::raw adds it automatically
                wasm_encoder::ConstExpr::raw(bytes)
            }
        }
    }
}

pub(crate) fn v128_to_u128(value: &wasmparser::V128) -> u128 {
    let n = value.bytes();
    (n[0] as u128)
        | ((n[1] as u128) << 8)
        | ((n[2] as u128) << 16)
        | ((n[3] as u128) << 24)
        | ((n[4] as u128) << 32)
        | ((n[5] as u128) << 40)
        | ((n[6] as u128) << 48)
        | ((n[7] as u128) << 56)
        | ((n[8] as u128) << 64)
        | ((n[9] as u128) << 72)
        | ((n[10] as u128) << 80)
        | ((n[11] as u128) << 88)
        | ((n[12] as u128) << 96)
        | ((n[13] as u128) << 104)
        | ((n[14] as u128) << 112)
        | ((n[15] as u128) << 120)
}
