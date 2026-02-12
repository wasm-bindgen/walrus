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

/// A heap type for GC reference types.
///
/// This represents the kind of heap object a reference points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum HeapType {
    /// Abstract heap type (abstract types like func, extern, any, etc.)
    Abstract(AbstractHeapType),
    /// Concrete (indexed) heap type - currently not supported
    Concrete(u32),
}

impl HeapType {
    /// Convert to wasm_encoder HeapType.
    pub fn to_wasmencoder_heap_type(self) -> wasm_encoder::HeapType {
        match self {
            HeapType::Abstract(ab_heap_type) => wasm_encoder::HeapType::Abstract {
                shared: false,
                ty: ab_heap_type.into(),
            },
            HeapType::Concrete(_) => todo!("concrete heap types not yet supported"),
        }
    }
}

impl TryFrom<wasmparser::HeapType> for HeapType {
    type Error = anyhow::Error;

    fn try_from(heap_type: wasmparser::HeapType) -> Result<HeapType> {
        match heap_type {
            wasmparser::HeapType::Abstract { shared: _, ty } => {
                Ok(HeapType::Abstract(ty.try_into()?))
            }
            wasmparser::HeapType::Concrete(_) | wasmparser::HeapType::Exact(_) => {
                bail!("concrete (indexed) heap types are not yet supported")
            }
        }
    }
}

impl fmt::Display for HeapType {
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

/// Abstract heap types for GC reference types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AbstractHeapType {
    /// The abstract `func` heap type (any function).
    Func,
    /// The abstract `extern` heap type (external/host references).
    Extern,
    /// The abstract `any` heap type (any internal reference).
    Any,
    /// The abstract `none` heap type (bottom type for internal refs).
    None,
    /// The abstract `noextern` heap type (bottom type for external refs).
    NoExtern,
    /// The abstract `nofunc` heap type (bottom type for function refs).
    NoFunc,
    /// The abstract `eq` heap type (comparable references: i31, struct, array).
    Eq,
    /// The abstract `struct` heap type.
    Struct,
    /// The abstract `array` heap type.
    Array,
    /// The abstract `i31` heap type (31-bit integers).
    I31,
    /// The abstract `exn` heap type (exceptions).
    Exn,
    /// The abstract `noexn` heap type (bottom type for exception refs).
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

/// A reference type.
///
/// Reference types include function references, external references, and
/// the GC proposal types (anyref, eqref, i31ref, etc.).
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

    /// Alias for the `eqref` type in WebAssembly.
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

    /// Alias for the `structref` type in WebAssembly.
    pub const STRUCTREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Struct),
    };

    /// Alias for the `exnref` type in WebAssembly.
    pub const EXNREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::Exn),
    };

    /// Alias for the `nullref` type in WebAssembly.
    pub const NULLREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::None),
    };

    /// Alias for the `nullexternref` type in WebAssembly.
    pub const NULLEXTERNREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::NoExtern),
    };

    /// Alias for the `nullfuncref` type in WebAssembly.
    pub const NULLFUNCREF: RefType = RefType {
        nullable: true,
        heap_type: HeapType::Abstract(AbstractHeapType::NoFunc),
    };

    /// Returns whether this reference type is nullable.
    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    /// Convert to wasm_encoder RefType.
    pub fn to_wasmencoder_ref_type(self) -> wasm_encoder::RefType {
        wasm_encoder::RefType {
            nullable: self.nullable,
            heap_type: self.heap_type.to_wasmencoder_heap_type(),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<wasm_encoder::RefType> for RefType {
    fn into(self) -> wasm_encoder::RefType {
        self.to_wasmencoder_ref_type()
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

impl fmt::Display for RefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.nullable {
            // Use shorthand names for common nullable types
            match self.heap_type {
                HeapType::Abstract(AbstractHeapType::Func) => write!(f, "funcref"),
                HeapType::Abstract(AbstractHeapType::Extern) => write!(f, "externref"),
                HeapType::Abstract(AbstractHeapType::Exn) => write!(f, "exnref"),
                HeapType::Abstract(AbstractHeapType::Any) => write!(f, "anyref"),
                HeapType::Abstract(AbstractHeapType::Eq) => write!(f, "eqref"),
                HeapType::Abstract(AbstractHeapType::I31) => write!(f, "i31ref"),
                HeapType::Abstract(AbstractHeapType::Struct) => write!(f, "structref"),
                HeapType::Abstract(AbstractHeapType::Array) => write!(f, "arrayref"),
                HeapType::Abstract(AbstractHeapType::None) => write!(f, "nullref"),
                HeapType::Abstract(AbstractHeapType::NoExtern) => write!(f, "nullexternref"),
                HeapType::Abstract(AbstractHeapType::NoFunc) => write!(f, "nullfuncref"),
                HeapType::Abstract(AbstractHeapType::NoExn) => write!(f, "nullexnref"),
                HeapType::Concrete(idx) => write!(f, "(ref null {idx})"),
            }
        } else {
            write!(f, "(ref {})", self.heap_type)
        }
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
            ValType::Ref(ref_type) => {
                wasm_encoder::ValType::Ref(ref_type.to_wasmencoder_ref_type())
            }
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
        match self {
            ValType::I32 => write!(f, "i32"),
            ValType::I64 => write!(f, "i64"),
            ValType::F32 => write!(f, "f32"),
            ValType::F64 => write!(f, "f64"),
            ValType::V128 => write!(f, "v128"),
            ValType::Ref(r) => write!(f, "{}", r),
        }
    }
}
