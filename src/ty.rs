//! WebAssembly function and value types.

use crate::error::Result;
use crate::tombstone_arena::Tombstone;
use anyhow::bail;
use id_arena::Id;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash;

/// An identifier for types.
pub type TypeId = Id<Type>;

/// A function type.
#[derive(Debug, Clone)]
pub struct Type {
    id: TypeId,
    params: Box<[ValType]>,
    results: Box<[ValType]>,

    // Whether or not this type is for a multi-value function entry block, and
    // therefore is for internal use only and shouldn't be emitted when we
    // serialize the Type section.
    is_for_function_entry: bool,

    /// An optional name for debugging.
    ///
    /// This is not really used by anything currently, but a theoretical WAT to
    /// walrus parser could keep track of the original name in the WAT.
    pub name: Option<String>,
}

impl PartialEq for Type {
    #[inline]
    fn eq(&self, rhs: &Type) -> bool {
        // NB: do not compare id or name.
        self.params == rhs.params
            && self.results == rhs.results
            && self.is_for_function_entry == rhs.is_for_function_entry
    }
}

impl Eq for Type {}

impl PartialOrd for Type {
    fn partial_cmp(&self, rhs: &Type) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Type {
    fn cmp(&self, rhs: &Type) -> Ordering {
        self.params()
            .cmp(rhs.params())
            .then_with(|| self.results().cmp(rhs.results()))
    }
}

impl hash::Hash for Type {
    #[inline]
    fn hash<H: hash::Hasher>(&self, h: &mut H) {
        // Do not hash id or name.
        self.params.hash(h);
        self.results.hash(h);
        self.is_for_function_entry.hash(h);
    }
}

impl Tombstone for Type {
    fn on_delete(&mut self) {
        self.params = Box::new([]);
        self.results = Box::new([]);
    }
}

impl Type {
    /// Construct a new function type.
    #[inline]
    pub(crate) fn new(id: TypeId, params: Box<[ValType]>, results: Box<[ValType]>) -> Type {
        Type {
            id,
            params,
            results,
            is_for_function_entry: false,
            name: None,
        }
    }

    /// Construct a new type for function entry blocks.
    #[inline]
    pub(crate) fn for_function_entry(id: TypeId, results: Box<[ValType]>) -> Type {
        let params = vec![].into();
        Type {
            id,
            params,
            results,
            is_for_function_entry: true,
            name: None,
        }
    }

    /// Get the id of this type.
    #[inline]
    pub fn id(&self) -> TypeId {
        self.id
    }

    /// Get the parameters to this function type.
    #[inline]
    pub fn params(&self) -> &[ValType] {
        &self.params
    }

    /// Get the results of this function type.
    #[inline]
    pub fn results(&self) -> &[ValType] {
        &self.results
    }

    pub(crate) fn is_for_function_entry(&self) -> bool {
        self.is_for_function_entry
    }
}

/// A value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValType {
    /// 32-bit integer.
    I32,
    /// 64-bit integer.
    I64,
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 128-bit vector.
    V128,
    /// Reference.
    Ref(RefType),
}

/// A reference type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RefType {
    /// Whether this reference type is nullable.
    pub nullable: bool,
    /// The heap type that this reference points to.
    pub heap_type: HeapType,
}

impl RefType {
    /// Alias for the `anyref` type in WebAssembly.
    pub const ANYREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Any),
    };

    /// Alias for the `anyref` type in WebAssembly.
    pub const EQREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Eq),
    };

    /// Alias for the `funcref` type in WebAssembly.
    pub const FUNCREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Func),
    };

    /// Alias for the `externref` type in WebAssembly.
    pub const EXTERNREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Extern),
    };

    /// Alias for the `i31ref` type in WebAssembly.
    pub const I31REF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::I31),
    };

    /// Alias for the `arrayref` type in WebAssembly.
    pub const ARRAYREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Array),
    };

    /// Alias for the `exnref` type in WebAssembly.
    pub const EXNREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Exn),
    };
}

#[allow(clippy::from_over_into)]
impl Into<wasm_encoder::RefType> for RefType {
    fn into(self) -> wasm_encoder::RefType {
        wasm_encoder::RefType {
            nullable: self.nullable,
            heap_type: self.heap_type.into(),
        }
    }
}

impl TryFrom<wasmparser::RefType> for RefType {
    type Error = anyhow::Error;

    fn try_from(ref_type: wasmparser::RefType) -> Result<RefType> {
        Ok(RefType {
            nullable: ref_type.is_nullable(),
            heap_type: ref_type.heap_type().try_into()?,
        })
    }
}

impl ValType {
    pub(crate) fn from_wasmparser_type(ty: wasmparser::ValType) -> Result<Box<[ValType]>> {
        let v = vec![ValType::parse(&ty)?];
        Ok(v.into_boxed_slice())
    }

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_wasmencoder_type(&self) -> wasm_encoder::ValType {
        match self {
            ValType::I32 => wasm_encoder::ValType::I32,
            ValType::I64 => wasm_encoder::ValType::I64,
            ValType::F32 => wasm_encoder::ValType::F32,
            ValType::F64 => wasm_encoder::ValType::F64,
            ValType::V128 => wasm_encoder::ValType::V128,
            ValType::Ref(RefType {
                nullable,
                heap_type,
            }) => wasm_encoder::ValType::Ref(wasm_encoder::RefType {
                nullable: *nullable,
                heap_type: (*heap_type).into(),
            }),
        }
    }

    pub(crate) fn parse(input: &wasmparser::ValType) -> Result<ValType> {
        match input {
            wasmparser::ValType::I32 => Ok(ValType::I32),
            wasmparser::ValType::I64 => Ok(ValType::I64),
            wasmparser::ValType::F32 => Ok(ValType::F32),
            wasmparser::ValType::F64 => Ok(ValType::F64),
            wasmparser::ValType::V128 => Ok(ValType::V128),
            wasmparser::ValType::Ref(wasmparser::RefType::CONT)
            | wasmparser::ValType::Ref(wasmparser::RefType::CONTREF)
            | wasmparser::ValType::Ref(wasmparser::RefType::NULLCONTREF)
            | wasmparser::ValType::Ref(wasmparser::RefType::NOCONT) => {
                bail!("The stack switching proposal is not supported")
            }
            wasmparser::ValType::Ref(ref_type) => Ok(ValType::Ref((*ref_type).try_into()?)),
        }
    }
}

impl fmt::Display for ValType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValType::I32 => "i32",
                ValType::I64 => "i64",
                ValType::F32 => "f32",
                ValType::F64 => "f64",
                ValType::V128 => "v128",
                ValType::Ref(RefType {
                    nullable: false,
                    heap_type,
                }) => return write!(f, "ref null {heap_type}"),
                ValType::Ref(RefType {
                    nullable: true,
                    heap_type,
                }) => return write!(f, "ref {heap_type}"),
            }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeapType {
    Abstract(AbstractHeapType),
    Concrete(u32),
}

impl std::fmt::Display for HeapType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeapType::Abstract(ab_heap_type) => write!(
                f,
                "{}",
                match ab_heap_type {
                    AbstractHeapType::Func => "func",
                    AbstractHeapType::Extern => "extern",
                    AbstractHeapType::Any => "any",
                    AbstractHeapType::None => "none",
                    AbstractHeapType::NoExtern => "noextern",
                    AbstractHeapType::NoFunc => "nofunc",
                    AbstractHeapType::Eq => "eq",
                    AbstractHeapType::Struct => "struct",
                    AbstractHeapType::Array => "array",
                    AbstractHeapType::I31 => "i31",
                    AbstractHeapType::Exn => "exn",
                    AbstractHeapType::NoExn => "noexn",
                }
            ),
            HeapType::Concrete(id) => write!(f, "{id}"),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<wasm_encoder::HeapType> for HeapType {
    fn into(self) -> wasm_encoder::HeapType {
        match self {
            HeapType::Abstract(ab_heap_type) => wasm_encoder::HeapType::Abstract {
                shared: false,
                ty: ab_heap_type.into(),
            },
            HeapType::Concrete(_) => todo!(),
        }
    }
}

impl TryFrom<wasmparser::HeapType> for HeapType {
    type Error = anyhow::Error;

    fn try_from(heap_type: wasmparser::HeapType) -> std::result::Result<Self, Self::Error> {
        Ok(match heap_type {
            wasmparser::HeapType::Abstract { shared: _, ty } => HeapType::Abstract(ty.try_into()?),
            wasmparser::HeapType::Concrete(_) => todo!(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AbstractHeapType {
    Func,
    Extern,
    Any,
    None,
    NoExtern,
    NoFunc,
    Eq,
    Struct,
    Array,
    I31,
    Exn,
    NoExn,
}

#[allow(clippy::from_over_into)]
impl Into<wasm_encoder::AbstractHeapType> for AbstractHeapType {
    fn into(self) -> wasm_encoder::AbstractHeapType {
        match self {
            AbstractHeapType::Func => wasm_encoder::AbstractHeapType::Func,
            AbstractHeapType::Extern => wasm_encoder::AbstractHeapType::Extern,
            AbstractHeapType::Any => wasm_encoder::AbstractHeapType::Any,
            AbstractHeapType::None => wasm_encoder::AbstractHeapType::None,
            AbstractHeapType::NoExtern => wasm_encoder::AbstractHeapType::NoExtern,
            AbstractHeapType::NoFunc => wasm_encoder::AbstractHeapType::NoFunc,
            AbstractHeapType::Eq => wasm_encoder::AbstractHeapType::Eq,
            AbstractHeapType::Struct => wasm_encoder::AbstractHeapType::Struct,
            AbstractHeapType::Array => wasm_encoder::AbstractHeapType::Array,
            AbstractHeapType::I31 => wasm_encoder::AbstractHeapType::I31,
            AbstractHeapType::Exn => wasm_encoder::AbstractHeapType::Exn,
            AbstractHeapType::NoExn => wasm_encoder::AbstractHeapType::NoExn,
        }
    }
}

impl TryFrom<wasmparser::AbstractHeapType> for AbstractHeapType {
    type Error = anyhow::Error;

    fn try_from(
        ab_heap_type: wasmparser::AbstractHeapType,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(match ab_heap_type {
            wasmparser::AbstractHeapType::Func => AbstractHeapType::Func,
            wasmparser::AbstractHeapType::Extern => AbstractHeapType::Extern,
            wasmparser::AbstractHeapType::Any => AbstractHeapType::Any,
            wasmparser::AbstractHeapType::None => AbstractHeapType::None,
            wasmparser::AbstractHeapType::NoExtern => AbstractHeapType::NoExtern,
            wasmparser::AbstractHeapType::NoFunc => AbstractHeapType::NoFunc,
            wasmparser::AbstractHeapType::Eq => AbstractHeapType::Eq,
            wasmparser::AbstractHeapType::Struct => AbstractHeapType::Struct,
            wasmparser::AbstractHeapType::Array => AbstractHeapType::Array,
            wasmparser::AbstractHeapType::I31 => AbstractHeapType::I31,
            wasmparser::AbstractHeapType::Exn => AbstractHeapType::Exn,
            wasmparser::AbstractHeapType::NoExn => AbstractHeapType::NoExn,
            wasmparser::AbstractHeapType::Cont | wasmparser::AbstractHeapType::NoCont => {
                bail!("Stack switching proposal is not supported")
            }
        })
    }
}
