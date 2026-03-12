//! Types in a wasm module.

use crate::arena_set::ArenaSet;
use crate::emit::{Emit, EmitContext};
use crate::error::Result;
use crate::module::Module;
use crate::parse::IndicesToIds;
use crate::ty::{
    ArrayType, CompositeType, FieldType, FunctionType, RecGroup, StructType, Type, TypeId, ValType,
};
use anyhow::bail;

/// The set of de-duplicated types within a module.
#[derive(Debug, Default)]
pub struct ModuleTypes {
    arena: ArenaSet<Type>,
    /// Recursive type groups in definition order.
    ///
    /// Every type belongs to exactly one rec group. Types created without an
    /// explicit rec group (e.g., via `add()`) get their own singleton group.
    rec_groups: Vec<RecGroup>,
}

impl ModuleTypes {
    /// Get a type associated with an ID
    pub fn get(&self, id: TypeId) -> &Type {
        &self.arena[id]
    }

    /// Get a type associated with an ID
    pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
        &mut self.arena[id]
    }

    /// Get the parameters and results for the given type.
    pub fn params_results(&self, id: TypeId) -> (&[ValType], &[ValType]) {
        let ty = self.get(id);
        (ty.params(), ty.results())
    }

    /// Get the parameters for the given type.
    pub fn params(&self, id: TypeId) -> &[ValType] {
        self.get(id).params()
    }

    /// Get the results for the given type.
    pub fn results(&self, id: TypeId) -> &[ValType] {
        self.get(id).results()
    }

    /// Get a type ID by its name.
    ///
    /// This is currently only intended for in-memory modifications, and by
    /// default will always return `None` for a newly parsed module. A
    /// hypothetical future WAT text format to `walrus::Module` parser could
    /// preserve type names from the WAT.
    pub fn by_name(&self, name: &str) -> Option<TypeId> {
        self.arena.iter().find_map(|(id, ty)| {
            if ty.name.as_deref() == Some(name) {
                Some(id)
            } else {
                None
            }
        })
    }

    /// Get a shared reference to this module's types.
    pub fn iter(&self) -> impl Iterator<Item = &Type> {
        self.arena.iter().map(|(_, f)| f)
    }

    /// Removes a type from this module.
    ///
    /// It is up to you to ensure that any potential references to the deleted
    /// type are also removed, eg `call_indirect` expressions, function types,
    /// etc.
    pub fn delete(&mut self, ty: TypeId) {
        self.arena.remove(ty);
    }

    /// Add a new type to this module, and return its `Id`
    pub fn add(&mut self, params: &[ValType], results: &[ValType]) -> TypeId {
        let next_id = self.arena.next_id();
        let id = self.arena.insert(Type::new(
            next_id,
            params.to_vec().into_boxed_slice(),
            results.to_vec().into_boxed_slice(),
        ));
        // Create a singleton rec group if this is a genuinely new type
        // (not deduplicated to an existing one that already has a group).
        if id == next_id {
            self.rec_groups.push(RecGroup { types: vec![id] });
        }
        id
    }

    pub(crate) fn add_entry_ty(&mut self, results: &[ValType]) -> TypeId {
        let next_id = self.arena.next_id();
        let id = self.arena.insert(Type::for_function_entry(
            next_id,
            results.to_vec().into_boxed_slice(),
        ));
        if id == next_id {
            self.rec_groups.push(RecGroup { types: vec![id] });
        }
        id
    }

    /// Find the existing type for the given parameters and results.
    pub fn find(&self, params: &[ValType], results: &[ValType]) -> Option<TypeId> {
        self.arena.iter().find_map(|(id, ty)| {
            if !ty.is_for_function_entry() && ty.params() == params && ty.results() == results {
                Some(id)
            } else {
                None
            }
        })
    }

    pub(crate) fn find_for_function_entry(&self, results: &[ValType]) -> Option<TypeId> {
        self.arena.iter().find_map(|(id, ty)| {
            if ty.is_for_function_entry() && ty.params().is_empty() && ty.results() == results {
                Some(id)
            } else {
                None
            }
        })
    }

    /// Get the rec groups in this module, in definition order.
    pub fn rec_groups(&self) -> &[RecGroup] {
        &self.rec_groups
    }

    /// Find the rec group that contains the given type.
    pub fn rec_group_for_type(&self, id: TypeId) -> Option<&RecGroup> {
        self.rec_groups.iter().find(|rg| rg.types.contains(&id))
    }
}

impl Module {
    /// Construct the set of types within a module.
    pub(crate) fn parse_types(
        &mut self,
        section: wasmparser::TypeSectionReader,
        ids: &mut IndicesToIds,
    ) -> Result<()> {
        log::debug!("parsing type section");

        // Track the module-level type index across all rec groups.
        let mut type_index_base: u32 = 0;

        for rec_group_result in section {
            let rec_group = rec_group_result?;
            let is_explicit = rec_group.is_explicit_rec_group();
            let sub_types: Box<[wasmparser::SubType]> = rec_group.into_types().collect();
            let rec_group_start = type_index_base;

            match &*sub_types {
                // wasmparser should never return an empty recursion group
                [] => bail!("rec group must contain at least one type"),
                // Implicit singleton rec group: no self-references or
                // forward references possible. Use deduplicating insert() so
                // structurally identical types share a single TypeId.
                [sub_type] if !is_explicit => {
                    let comp =
                        parse_composite_type(&sub_type.composite_type, ids, rec_group_start)?;
                    let supertype =
                        resolve_supertype(sub_type.supertype_idx, ids, rec_group_start)?;
                    let id = self.types.arena.next_id();
                    let ty = Type::new_composite(id, comp, sub_type.is_final, supertype);
                    let id = self.types.arena.insert(ty);
                    ids.push_type(id);
                    self.types.rec_groups.push(RecGroup { types: vec![id] });
                    type_index_base += 1;
                }
                sub_types => {
                    // Explicit or multi-type rec group: types can forward-reference
                    // each other. Pre-allocate TypeIds with placeholders so forward
                    // references within the group can be resolved.
                    let mut type_ids = Vec::with_capacity(sub_types.len());
                    for _ in sub_types {
                        let id = self.types.arena.next_id();
                        let id = self.types.arena.alloc_unique(Type::placeholder(id));
                        ids.push_type(id);
                        type_ids.push(id);
                    }

                    // Parse each sub type and overwrite the placeholder.
                    for (sub_type, &type_id) in sub_types.iter().zip(type_ids.iter()) {
                        let comp =
                            parse_composite_type(&sub_type.composite_type, ids, rec_group_start)?;
                        let supertype =
                            resolve_supertype(sub_type.supertype_idx, ids, rec_group_start)?;
                        let real_type =
                            Type::new_composite(type_id, comp, sub_type.is_final, supertype);
                        self.types.arena.replace_and_register(type_id, real_type);
                    }

                    let group_len = type_ids.len() as u32;
                    self.types.rec_groups.push(RecGroup { types: type_ids });
                    type_index_base += group_len;
                }
            }
        }

        Ok(())
    }
}

/// Parse a wasmparser `CompositeType` into a walrus `CompositeType`.
fn parse_composite_type(
    ct: &wasmparser::CompositeType,
    ids: &IndicesToIds,
    rec_group_start: u32,
) -> Result<CompositeType> {
    match &ct.inner {
        wasmparser::CompositeInnerType::Func(func_ty) => {
            let params = func_ty
                .params()
                .iter()
                .map(|vt| ValType::from_wasmparser(vt, ids, rec_group_start))
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice();
            let results = func_ty
                .results()
                .iter()
                .map(|vt| ValType::from_wasmparser(vt, ids, rec_group_start))
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice();
            Ok(CompositeType::Function(FunctionType::new(params, results)))
        }
        wasmparser::CompositeInnerType::Struct(struct_ty) => {
            let fields = struct_ty
                .fields
                .iter()
                .map(|ft| FieldType::from_wasmparser(*ft, ids, rec_group_start))
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice();
            Ok(CompositeType::Struct(StructType { fields }))
        }
        wasmparser::CompositeInnerType::Array(array_ty) => {
            let field = FieldType::from_wasmparser(array_ty.0, ids, rec_group_start)?;
            Ok(CompositeType::Array(ArrayType { field }))
        }
        wasmparser::CompositeInnerType::Cont(_) => {
            bail!("The stack switching proposal is not supported")
        }
    }
}

/// Resolve an optional supertype index from a wasmparser `PackedIndex` to a
/// walrus `TypeId`.
fn resolve_supertype(
    supertype_idx: Option<wasmparser::PackedIndex>,
    ids: &IndicesToIds,
    rec_group_start: u32,
) -> Result<Option<TypeId>> {
    match supertype_idx {
        None => Ok(None),
        Some(packed) => {
            let unpacked = packed.unpack();
            match unpacked {
                wasmparser::UnpackedIndex::Module(idx) => Ok(Some(ids.get_type(idx)?)),
                wasmparser::UnpackedIndex::RecGroup(idx) => {
                    Ok(Some(ids.get_type(rec_group_start + idx)?))
                }
                #[allow(unreachable_patterns)]
                _ => bail!("unsupported supertype index variant"),
            }
        }
    }
}

impl Emit for ModuleTypes {
    fn emit(&self, cx: &mut EmitContext) {
        log::debug!("emitting type section");

        let mut wasm_type_section = wasm_encoder::TypeSection::new();

        let mut tys = self
            .arena
            .iter()
            .filter(|(_, ty)| !ty.is_for_function_entry())
            .collect::<Vec<_>>();

        if tys.is_empty() {
            return;
        }

        // Sort for deterministic ordering.
        tys.sort_by_key(|&(_, ty)| ty);

        for (id, ty) in tys {
            cx.indices.push_type(id);
            wasm_type_section.ty().function(
                ty.params().iter().map(ValType::to_wasmencoder_type),
                ty.results().iter().map(ValType::to_wasmencoder_type),
            );
        }

        cx.wasm_module.section(&wasm_type_section);
    }
}
