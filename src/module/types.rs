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
            self.rec_groups.push(RecGroup {
                types: vec![id],
                is_explicit: false,
            });
        }
        id
    }

    /// Add a new struct type to this module (final, no supertype).
    ///
    /// Deduplicates with existing structurally identical types.
    /// Creates an implicit singleton rec group for genuinely new types.
    ///
    /// # Example
    ///
    /// ```
    /// use walrus::*;
    ///
    /// let mut module = Module::default();
    ///
    /// // Create: (type $point (struct (field (mut i32)) (field (mut i32))))
    /// let point_ty = module.types.add_struct(vec![
    ///     FieldType { element_type: StorageType::Val(ValType::I32), mutable: true },
    ///     FieldType { element_type: StorageType::Val(ValType::I32), mutable: true },
    /// ]);
    ///
    /// // Use in a function: (func (param i32 i32) (result (ref $point)))
    /// let point_ref = ValType::Ref(RefType {
    ///     nullable: true,
    ///     heap_type: HeapType::Concrete(point_ty),
    /// });
    /// let mut builder = FunctionBuilder::new(
    ///     &mut module.types,
    ///     &[ValType::I32, ValType::I32],
    ///     &[point_ref],
    /// );
    /// let x = module.locals.add(ValType::I32);
    /// let y = module.locals.add(ValType::I32);
    /// builder.func_body().local_get(x).local_get(y).struct_new(point_ty);
    /// let func = builder.finish(vec![x, y], &mut module.funcs);
    /// module.exports.add("make_point", func);
    /// ```
    pub fn add_struct(&mut self, fields: Vec<FieldType>) -> TypeId {
        self.add_composite(
            CompositeType::Struct(StructType {
                fields: fields.into_boxed_slice(),
            }),
            true,
            None,
        )
    }

    /// Add a new array type to this module (final, no supertype).
    ///
    /// Deduplicates with existing structurally identical types.
    /// Creates an implicit singleton rec group for genuinely new types.
    ///
    /// # Example
    ///
    /// ```
    /// use walrus::*;
    ///
    /// let mut module = Module::default();
    ///
    /// // Create: (type $i32_arr (array (mut i32)))
    /// let arr_ty = module.types.add_array(FieldType {
    ///     element_type: StorageType::Val(ValType::I32),
    ///     mutable: true,
    /// });
    ///
    /// // Use in a function: (func (param i32 i32) (result (ref $i32_arr)))
    /// let arr_ref = ValType::Ref(RefType {
    ///     nullable: true,
    ///     heap_type: HeapType::Concrete(arr_ty),
    /// });
    /// let mut builder = FunctionBuilder::new(
    ///     &mut module.types,
    ///     &[ValType::I32, ValType::I32],
    ///     &[arr_ref],
    /// );
    /// let init = module.locals.add(ValType::I32);
    /// let len = module.locals.add(ValType::I32);
    /// builder.func_body().local_get(init).local_get(len).array_new(arr_ty);
    /// let func = builder.finish(vec![init, len], &mut module.funcs);
    /// module.exports.add("make_array", func);
    /// ```
    pub fn add_array(&mut self, element: FieldType) -> TypeId {
        self.add_composite(
            CompositeType::Array(ArrayType { field: element }),
            true,
            None,
        )
    }

    /// Add any composite type with explicit subtyping controls.
    ///
    /// Deduplicates with existing structurally identical types.
    /// Creates an implicit singleton rec group for genuinely new types.
    ///
    /// # Example
    ///
    /// ```
    /// use walrus::*;
    ///
    /// let mut module = Module::default();
    ///
    /// // Create a non-final base type:
    /// // (type $base (sub (struct (field i32))))
    /// let base = module.types.add_composite(
    ///     CompositeType::Struct(StructType {
    ///         fields: vec![FieldType {
    ///             element_type: StorageType::Val(ValType::I32),
    ///             mutable: false,
    ///         }].into_boxed_slice(),
    ///     }),
    ///     false, // not final — open for subtyping
    ///     None,
    /// );
    ///
    /// // Create a final derived type:
    /// // (type $derived (sub final $base (struct (field i32) (field f64))))
    /// let derived = module.types.add_composite(
    ///     CompositeType::Struct(StructType {
    ///         fields: vec![
    ///             FieldType { element_type: StorageType::Val(ValType::I32), mutable: false },
    ///             FieldType { element_type: StorageType::Val(ValType::F64), mutable: false },
    ///         ].into_boxed_slice(),
    ///     }),
    ///     true, // final
    ///     Some(base),
    /// );
    /// ```
    pub fn add_composite(
        &mut self,
        comp: CompositeType,
        is_final: bool,
        supertype: Option<TypeId>,
    ) -> TypeId {
        let next_id = self.arena.next_id();
        let ty = Type::new_composite(next_id, comp, is_final, supertype);
        let id = self.arena.insert(ty);
        if id == next_id {
            self.rec_groups.push(RecGroup {
                types: vec![id],
                is_explicit: false,
            });
        }
        id
    }

    /// Add an explicit recursive type group.
    ///
    /// Pre-allocates `count` placeholder `TypeId`s and passes them to the
    /// `build` closure so that it can use them as forward references when
    /// constructing mutually-recursive type definitions. The closure must
    /// return exactly `count` type definitions as
    /// `(CompositeType, is_final, supertype)` tuples.
    ///
    /// Types in a rec group are **not** deduplicated — each gets a unique
    /// `TypeId`, matching the semantics of the wasm binary format.
    ///
    /// # Example
    ///
    /// ```
    /// use walrus::*;
    ///
    /// let mut module = Module::default();
    ///
    /// // Create two mutually-recursive struct types:
    /// // (rec
    /// //   (type $a (struct (field (ref null $b))))
    /// //   (type $b (struct (field (ref null $a))))
    /// // )
    /// let ids = module.types.add_rec_group(2, |type_ids| {
    ///     let a_id = type_ids[0];
    ///     let b_id = type_ids[1];
    ///
    ///     let a_def = CompositeType::Struct(StructType {
    ///         fields: vec![FieldType {
    ///             element_type: StorageType::Val(ValType::Ref(RefType {
    ///                 nullable: true,
    ///                 heap_type: HeapType::Concrete(b_id), // forward ref to $b
    ///             })),
    ///             mutable: false,
    ///         }].into_boxed_slice(),
    ///     });
    ///
    ///     let b_def = CompositeType::Struct(StructType {
    ///         fields: vec![FieldType {
    ///             element_type: StorageType::Val(ValType::Ref(RefType {
    ///                 nullable: true,
    ///                 heap_type: HeapType::Concrete(a_id), // back ref to $a
    ///             })),
    ///             mutable: false,
    ///         }].into_boxed_slice(),
    ///     });
    ///
    ///     vec![(a_def, true, None), (b_def, true, None)]
    /// });
    ///
    /// assert_eq!(ids.len(), 2);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the closure returns a different number of types than `count`.
    pub fn add_rec_group(
        &mut self,
        count: usize,
        build: impl FnOnce(&[TypeId]) -> Vec<(CompositeType, bool, Option<TypeId>)>,
    ) -> Vec<TypeId> {
        // Pre-allocate placeholder TypeIds.
        let mut type_ids = Vec::with_capacity(count);
        for _ in 0..count {
            let id = self.arena.next_id();
            let id = self.arena.alloc_unique(Type::placeholder(id));
            type_ids.push(id);
        }

        // Let the caller build the real type definitions using the
        // pre-allocated ids for forward references.
        let defs = build(&type_ids);
        assert_eq!(
            defs.len(),
            count,
            "add_rec_group: closure returned {} types, expected {}",
            defs.len(),
            count,
        );

        // Finalize each placeholder with its real definition.
        for (&type_id, (comp, is_final, supertype)) in type_ids.iter().zip(defs) {
            let real_type = Type::new_composite(type_id, comp, is_final, supertype);
            self.arena.replace_and_register(type_id, real_type);
        }

        // Register the explicit rec group.
        self.rec_groups.push(RecGroup {
            types: type_ids.clone(),
            is_explicit: true,
        });

        type_ids
    }

    pub(crate) fn add_entry_ty(&mut self, results: &[ValType]) -> TypeId {
        let next_id = self.arena.next_id();
        let id = self.arena.insert(Type::for_function_entry(
            next_id,
            results.to_vec().into_boxed_slice(),
        ));
        if id == next_id {
            self.rec_groups.push(RecGroup {
                types: vec![id],
                is_explicit: false,
            });
        }
        id
    }

    /// Find the existing type for the given parameters and results.
    pub fn find(&self, params: &[ValType], results: &[ValType]) -> Option<TypeId> {
        self.arena.iter().find_map(|(id, ty)| {
            if ty.is_function()
                && !ty.is_for_function_entry()
                && ty.params() == params
                && ty.results() == results
            {
                Some(id)
            } else {
                None
            }
        })
    }

    pub(crate) fn find_for_function_entry(&self, results: &[ValType]) -> Option<TypeId> {
        self.arena.iter().find_map(|(id, ty)| {
            if ty.is_function()
                && ty.is_for_function_entry()
                && ty.params().is_empty()
                && ty.results() == results
            {
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
                // Empty explicit rec group — valid per spec, just record it.
                [] => {
                    self.types.rec_groups.push(RecGroup {
                        types: vec![],
                        is_explicit: true,
                    });
                }
                // Implicit singleton rec group. Pre-allocate a placeholder
                // so self-references within the type body can resolve
                // (e.g., linked-list nodes). After parsing, try to
                // deduplicate with an existing structurally identical type.
                [sub_type] if !is_explicit => {
                    let next_id = self.types.arena.next_id();
                    let placeholder_id = self.types.arena.alloc_unique(Type::placeholder(next_id));
                    ids.push_type(placeholder_id);

                    let comp =
                        parse_composite_type(&sub_type.composite_type, ids, rec_group_start)?;
                    let supertype =
                        resolve_supertype(sub_type.supertype_idx, ids, rec_group_start)?;
                    let real_type =
                        Type::new_composite(placeholder_id, comp, sub_type.is_final, supertype);

                    // Try to deduplicate: if a structurally identical type
                    // already exists, remap to it and discard the placeholder.
                    if let Some(existing_id) = self.types.arena.find(&real_type) {
                        ids.remap_type(rec_group_start, existing_id);
                        self.types.arena.remove(placeholder_id);
                    } else {
                        // Genuinely new type — finalize the placeholder.
                        self.types
                            .arena
                            .replace_and_register(placeholder_id, real_type);
                        self.types.rec_groups.push(RecGroup {
                            types: vec![placeholder_id],
                            is_explicit: false,
                        });
                    }
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
                    self.types.rec_groups.push(RecGroup {
                        types: type_ids,
                        is_explicit: true,
                    });
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
        let mut any_emitted = false;

        for rec_group in &self.rec_groups {
            // Skip rec groups where any member type has been deleted (e.g.,
            // by the GC pass removing unused types).
            if rec_group.types.iter().any(|id| !self.arena.contains(*id)) {
                continue;
            }

            // Skip rec groups that only contain entry types (internal-only).
            // Note: `.all()` on an empty iterator returns true, so guard
            // against empty rec groups (which are valid and must be emitted).
            if !rec_group.types.is_empty()
                && rec_group
                    .types
                    .iter()
                    .all(|id| self.arena[*id].is_for_function_entry())
            {
                continue;
            }

            // Register all types in this group for index assignment before
            // building SubTypes, so that forward references within the group
            // resolve correctly.
            for &type_id in &rec_group.types {
                cx.indices.push_type(type_id);
            }

            // Build wasm-encoder SubTypes.
            let sub_types: Vec<wasm_encoder::SubType> = rec_group
                .types
                .iter()
                .map(|&id| walrus_type_to_encoder_subtype(&self.arena[id], cx.indices))
                .collect();

            // Emit: use rec() for explicit or multi-type groups, subtype() for
            // implicit singletons.
            if rec_group.is_explicit || sub_types.len() > 1 {
                wasm_type_section.ty().rec(sub_types);
            } else {
                debug_assert_eq!(sub_types.len(), 1);
                wasm_type_section.ty().subtype(&sub_types[0]);
            }
            any_emitted = true;
        }

        if any_emitted {
            cx.wasm_module.section(&wasm_type_section);
        }
    }
}

/// Convert a walrus `Type` to a `wasm_encoder::SubType`.
fn walrus_type_to_encoder_subtype(
    ty: &Type,
    indices: &crate::emit::IdsToIndices,
) -> wasm_encoder::SubType {
    let composite_type = walrus_composite_to_encoder(ty.kind(), indices);
    wasm_encoder::SubType {
        is_final: ty.is_final,
        supertype_idx: ty.supertype.map(|id| indices.get_type_index(id)),
        composite_type: wasm_encoder::CompositeType {
            inner: composite_type,
            shared: false,
            descriptor: None,
            describes: None,
        },
    }
}

/// Convert a walrus `CompositeType` to a `wasm_encoder::CompositeInnerType`.
fn walrus_composite_to_encoder(
    comp: &CompositeType,
    indices: &crate::emit::IdsToIndices,
) -> wasm_encoder::CompositeInnerType {
    match comp {
        CompositeType::Function(ft) => {
            let params: Vec<wasm_encoder::ValType> = ft
                .params()
                .iter()
                .map(|vt| vt.to_wasmencoder_type(indices))
                .collect();
            let results: Vec<wasm_encoder::ValType> = ft
                .results()
                .iter()
                .map(|vt| vt.to_wasmencoder_type(indices))
                .collect();
            wasm_encoder::CompositeInnerType::Func(wasm_encoder::FuncType::new(params, results))
        }
        CompositeType::Struct(st) => {
            let fields: Vec<wasm_encoder::FieldType> = st
                .fields
                .iter()
                .map(|f| f.to_wasmencoder_type(indices))
                .collect();
            wasm_encoder::CompositeInnerType::Struct(wasm_encoder::StructType {
                fields: fields.into_boxed_slice(),
            })
        }
        CompositeType::Array(at) => {
            let field = at.field.to_wasmencoder_type(indices);
            wasm_encoder::CompositeInnerType::Array(wasm_encoder::ArrayType(field))
        }
    }
}
