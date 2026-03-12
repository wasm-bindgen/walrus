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

/// An identifier for recursive type groups.
pub type RecGroupId = Id<RecGroup>;

/// A recursive type group.
///
/// In the GC proposal, types can be grouped into recursive type groups that
/// allow mutual references between types. Each type in a module belongs to
/// exactly one recursive group (singleton groups for non-recursive types).
#[derive(Debug, Clone)]
pub struct RecGroup {
    /// The types in this recursive group, in definition order.
    pub types: Vec<TypeId>,
}

// ---------------------------------------------------------------------------
// GC type definitions: storage types, field types, aggregate types
// ---------------------------------------------------------------------------

/// A packed storage type for struct and array fields.
///
/// Packed types allow storing smaller integers in fields for memory efficiency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StorageType {
    /// A standard value type.
    Val(ValType),
    /// An 8-bit integer (packed).
    I8,
    /// A 16-bit integer (packed).
    I16,
}

impl StorageType {
    /// Unpack a storage type to its corresponding value type.
    ///
    /// Packed integer types (`I8`, `I16`) unpack to `I32`.
    pub fn unpack(&self) -> ValType {
        match self {
            StorageType::Val(v) => *v,
            StorageType::I8 | StorageType::I16 => ValType::I32,
        }
    }

    /// Convert a wasmparser StorageType to a walrus StorageType, resolving
    /// concrete type indices via `IndicesToIds`.
    pub(crate) fn from_wasmparser(
        st: wasmparser::StorageType,
        ids: &crate::parse::IndicesToIds,
        rec_group_start: u32,
    ) -> Result<StorageType> {
        match st {
            wasmparser::StorageType::I8 => Ok(StorageType::I8),
            wasmparser::StorageType::I16 => Ok(StorageType::I16),
            wasmparser::StorageType::Val(vt) => Ok(StorageType::Val(ValType::from_wasmparser(
                &vt,
                ids,
                rec_group_start,
            )?)),
        }
    }
}

impl fmt::Display for StorageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageType::Val(v) => write!(f, "{v}"),
            StorageType::I8 => write!(f, "i8"),
            StorageType::I16 => write!(f, "i16"),
        }
    }
}

/// A field type for struct and array fields.
///
/// Combines a storage type with a mutability flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldType {
    /// The storage type of this field.
    pub element_type: StorageType,
    /// Whether this field is mutable.
    pub mutable: bool,
}

impl FieldType {
    /// Convert a wasmparser FieldType to a walrus FieldType, resolving
    /// concrete type indices via `IndicesToIds`.
    pub(crate) fn from_wasmparser(
        ft: wasmparser::FieldType,
        ids: &crate::parse::IndicesToIds,
        rec_group_start: u32,
    ) -> Result<FieldType> {
        Ok(FieldType {
            element_type: StorageType::from_wasmparser(ft.element_type, ids, rec_group_start)?,
            mutable: ft.mutable,
        })
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mutable {
            write!(f, "(mut {})", self.element_type)
        } else {
            write!(f, "{}", self.element_type)
        }
    }
}

/// A function type, consisting of parameter and result types.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionType {
    /// The parameter types.
    params: Box<[ValType]>,
    /// The result types.
    results: Box<[ValType]>,
}

impl FunctionType {
    /// Create a new function type.
    pub fn new(params: Box<[ValType]>, results: Box<[ValType]>) -> Self {
        FunctionType { params, results }
    }

    /// Get the parameter types.
    #[inline]
    pub fn params(&self) -> &[ValType] {
        &self.params
    }

    /// Get the result types.
    #[inline]
    pub fn results(&self) -> &[ValType] {
        &self.results
    }
}

impl fmt::Display for FunctionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(func")?;
        if !self.params.is_empty() {
            let params = self
                .params
                .iter()
                .map(|p| format!("{p}"))
                .collect::<Vec<_>>()
                .join(" ");
            write!(f, " (param {params})")?;
        }
        if !self.results.is_empty() {
            let results = self
                .results
                .iter()
                .map(|r| format!("{r}"))
                .collect::<Vec<_>>()
                .join(" ");
            write!(f, " (result {results})")?;
        }
        write!(f, ")")
    }
}

/// A struct type, consisting of a sequence of field types.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StructType {
    /// The fields of this struct type.
    pub fields: Box<[FieldType]>,
}

impl fmt::Display for StructType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = self
            .fields
            .iter()
            .map(|field| format!("(field {field})"))
            .collect::<Vec<_>>()
            .join(" ");
        if fields.is_empty() {
            write!(f, "(struct)")
        } else {
            write!(f, "(struct {fields})")
        }
    }
}

/// An array type, consisting of a single field type for all elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArrayType {
    /// The element type of this array.
    pub field: FieldType,
}

impl fmt::Display for ArrayType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(array {})", self.field)
    }
}

/// A composite type that can be a function, struct, or array.
///
/// This corresponds to the `comptype` production in the WebAssembly GC spec.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompositeType {
    /// A function type.
    Function(FunctionType),
    /// A struct type (GC proposal).
    Struct(StructType),
    /// An array type (GC proposal).
    Array(ArrayType),
}

impl CompositeType {
    /// Returns `Some` if this is a function type.
    pub fn as_function(&self) -> Option<&FunctionType> {
        match self {
            CompositeType::Function(f) => Some(f),
            _ => None,
        }
    }

    /// Returns a mutable reference if this is a function type.
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionType> {
        match self {
            CompositeType::Function(f) => Some(f),
            _ => None,
        }
    }

    /// Returns `Some` if this is a struct type.
    pub fn as_struct(&self) -> Option<&StructType> {
        match self {
            CompositeType::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// Returns `Some` if this is an array type.
    pub fn as_array(&self) -> Option<&ArrayType> {
        match self {
            CompositeType::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Unwrap as a function type, panicking if it's not one.
    pub fn unwrap_function(&self) -> &FunctionType {
        self.as_function().expect("expected a function type")
    }

    /// Unwrap as a struct type, panicking if it's not one.
    pub fn unwrap_struct(&self) -> &StructType {
        self.as_struct().expect("expected a struct type")
    }

    /// Unwrap as an array type, panicking if it's not one.
    pub fn unwrap_array(&self) -> &ArrayType {
        self.as_array().expect("expected an array type")
    }

    /// Returns `true` if this is a function type.
    pub fn is_function(&self) -> bool {
        matches!(self, CompositeType::Function(_))
    }

    /// Returns `true` if this is a struct type.
    pub fn is_struct(&self) -> bool {
        matches!(self, CompositeType::Struct(_))
    }

    /// Returns `true` if this is an array type.
    pub fn is_array(&self) -> bool {
        matches!(self, CompositeType::Array(_))
    }
}

impl fmt::Display for CompositeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositeType::Function(ft) => write!(f, "{ft}"),
            CompositeType::Struct(st) => write!(f, "{st}"),
            CompositeType::Array(at) => write!(f, "{at}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Type: the top-level type definition (subtype wrapper)
// ---------------------------------------------------------------------------

/// A WebAssembly type definition.
///
/// With the GC proposal, types can be function types, struct types, or array
/// types, and can participate in subtyping and recursive type groups.
#[derive(Debug, Clone)]
pub struct Type {
    id: TypeId,
    /// The composite type definition (function, struct, or array).
    comp: CompositeType,
    /// Whether this type is final (cannot be further subtyped).
    /// Defaults to `true` for types without explicit subtype declarations.
    pub is_final: bool,
    /// Optional supertype that this type extends.
    pub supertype: Option<TypeId>,

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
        self.comp == rhs.comp
            && self.is_final == rhs.is_final
            && self.supertype == rhs.supertype
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
        self.comp
            .cmp(&rhs.comp)
            .then_with(|| self.is_final.cmp(&rhs.is_final))
            .then_with(|| self.supertype.cmp(&rhs.supertype))
    }
}

impl hash::Hash for Type {
    #[inline]
    fn hash<H: hash::Hasher>(&self, h: &mut H) {
        // Do not hash id or name.
        self.comp.hash(h);
        self.is_final.hash(h);
        self.supertype.hash(h);
        self.is_for_function_entry.hash(h);
    }
}

impl Tombstone for Type {
    fn on_delete(&mut self) {
        self.comp = CompositeType::Function(FunctionType {
            params: Box::new([]),
            results: Box::new([]),
        });
    }
}

impl Type {
    /// Construct a placeholder type for pre-allocation during parsing.
    ///
    /// The placeholder will be overwritten with the real type once all
    /// forward references within a rec group can be resolved.
    #[inline]
    pub(crate) fn placeholder(id: TypeId) -> Type {
        Type {
            id,
            comp: CompositeType::Struct(Default::default()),
            is_final: true,
            supertype: None,
            is_for_function_entry: false,
            name: None,
        }
    }

    /// Construct a new function type.
    #[inline]
    pub(crate) fn new(id: TypeId, params: Box<[ValType]>, results: Box<[ValType]>) -> Type {
        Type {
            id,
            comp: CompositeType::Function(FunctionType { params, results }),
            is_final: true,
            supertype: None,
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
            comp: CompositeType::Function(FunctionType { params, results }),
            is_final: true,
            supertype: None,
            is_for_function_entry: true,
            name: None,
        }
    }

    /// Construct a new type with a given composite type.
    pub(crate) fn new_composite(
        id: TypeId,
        comp: CompositeType,
        is_final: bool,
        supertype: Option<TypeId>,
    ) -> Type {
        Type {
            id,
            comp,
            is_final,
            supertype,
            is_for_function_entry: false,
            name: None,
        }
    }

    /// Get the id of this type.
    #[inline]
    pub fn id(&self) -> TypeId {
        self.id
    }

    /// Get a reference to the composite type.
    #[inline]
    pub fn kind(&self) -> &CompositeType {
        &self.comp
    }

    /// Get a mutable reference to the composite type.
    #[inline]
    pub fn kind_mut(&mut self) -> &mut CompositeType {
        &mut self.comp
    }

    /// Get the parameters to this function type.
    ///
    /// # Panics
    ///
    /// Panics if this type is not a function type.
    #[inline]
    pub fn params(&self) -> &[ValType] {
        self.comp.unwrap_function().params()
    }

    /// Get the results of this function type.
    ///
    /// # Panics
    ///
    /// Panics if this type is not a function type.
    #[inline]
    pub fn results(&self) -> &[ValType] {
        self.comp.unwrap_function().results()
    }

    /// Returns this type's composite type as a function type, if it is one.
    #[inline]
    pub fn as_function(&self) -> Option<&FunctionType> {
        self.comp.as_function()
    }

    /// Returns this type's composite type as a struct type, if it is one.
    #[inline]
    pub fn as_struct(&self) -> Option<&StructType> {
        self.comp.as_struct()
    }

    /// Returns this type's composite type as an array type, if it is one.
    #[inline]
    pub fn as_array(&self) -> Option<&ArrayType> {
        self.comp.as_array()
    }

    /// Whether this type is a function type.
    #[inline]
    pub fn is_function(&self) -> bool {
        self.comp.is_function()
    }

    /// Whether this type is a struct type.
    #[inline]
    pub fn is_struct(&self) -> bool {
        self.comp.is_struct()
    }

    /// Whether this type is an array type.
    #[inline]
    pub fn is_array(&self) -> bool {
        self.comp.is_array()
    }

    pub(crate) fn is_for_function_entry(&self) -> bool {
        self.is_for_function_entry
    }
}

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Heap types
// ---------------------------------------------------------------------------

/// A heap type for GC reference types.
///
/// This represents the kind of heap object a reference points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum HeapType {
    /// Abstract heap type (abstract types like func, extern, any, etc.)
    Abstract(AbstractHeapType),
    /// Concrete (indexed) heap type, referencing a defined type by its id.
    Concrete(TypeId),
}

impl HeapType {
    /// Convert to wasm_encoder HeapType.
    ///
    /// For concrete heap types, this currently panics. Use
    /// `to_wasmencoder_heap_type_with_indices` for concrete type support.
    pub fn to_wasmencoder_heap_type(self) -> wasm_encoder::HeapType {
        match self {
            HeapType::Abstract(ab_heap_type) => wasm_encoder::HeapType::Abstract {
                shared: false,
                ty: ab_heap_type.into(),
            },
            HeapType::Concrete(id) => wasm_encoder::HeapType::Concrete(id.index() as u32),
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
                bail!("concrete (indexed) heap types require IndicesToIds for resolution; use HeapType::from_wasmparser")
            }
        }
    }
}

impl HeapType {
    /// Convert a wasmparser HeapType to a walrus HeapType, resolving concrete
    /// type indices via `IndicesToIds`.
    ///
    /// `rec_group_start` is the module-level type index of the first type in
    /// the current rec group, used to resolve `RecGroup`-relative indices.
    pub(crate) fn from_wasmparser(
        heap_type: wasmparser::HeapType,
        ids: &crate::parse::IndicesToIds,
        rec_group_start: u32,
    ) -> Result<HeapType> {
        match heap_type {
            wasmparser::HeapType::Abstract { shared: _, ty } => {
                Ok(HeapType::Abstract(ty.try_into()?))
            }
            wasmparser::HeapType::Concrete(unpacked) | wasmparser::HeapType::Exact(unpacked) => {
                let type_id = resolve_unpacked_index(unpacked, ids, rec_group_start)?;
                Ok(HeapType::Concrete(type_id))
            }
        }
    }
}

/// Resolve a wasmparser `UnpackedIndex` to a walrus `TypeId`.
fn resolve_unpacked_index(
    unpacked: wasmparser::UnpackedIndex,
    ids: &crate::parse::IndicesToIds,
    rec_group_start: u32,
) -> Result<TypeId> {
    match unpacked {
        wasmparser::UnpackedIndex::Module(idx) => ids.get_type(idx),
        wasmparser::UnpackedIndex::RecGroup(idx) => ids.get_type(rec_group_start + idx),
        #[allow(unreachable_patterns)]
        _ => bail!("unsupported type index variant"),
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
            HeapType::Concrete(id) => write!(f, "{}", id.index()),
        }
    }
}

// ---------------------------------------------------------------------------
// Abstract heap types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Reference types
// ---------------------------------------------------------------------------

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

impl RefType {
    /// Convert a wasmparser RefType to a walrus RefType, resolving concrete
    /// type indices via `IndicesToIds`.
    pub(crate) fn from_wasmparser(
        ref_type: wasmparser::RefType,
        ids: &crate::parse::IndicesToIds,
        rec_group_start: u32,
    ) -> Result<RefType> {
        Ok(RefType {
            nullable: ref_type.is_nullable(),
            heap_type: HeapType::from_wasmparser(ref_type.heap_type(), ids, rec_group_start)?,
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
                HeapType::Concrete(id) => write!(f, "(ref null {})", id.index()),
            }
        } else {
            write!(f, "(ref {})", self.heap_type)
        }
    }
}

// ---------------------------------------------------------------------------
// ValType conversion impls
// ---------------------------------------------------------------------------

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

    /// Convert a wasmparser ValType to a walrus ValType, resolving concrete
    /// type indices via `IndicesToIds`.
    pub(crate) fn from_wasmparser(
        input: &wasmparser::ValType,
        ids: &crate::parse::IndicesToIds,
        rec_group_start: u32,
    ) -> Result<ValType> {
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
            wasmparser::ValType::Ref(ref_type) => Ok(ValType::Ref(RefType::from_wasmparser(
                *ref_type,
                ids,
                rec_group_start,
            )?)),
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
