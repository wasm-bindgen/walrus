//! Tables within a wasm module.

use std::convert::TryInto;

use crate::emit::{Emit, EmitContext};
use crate::map::IdHashSet;
use crate::parse::IndicesToIds;
use crate::tombstone_arena::{Id, Tombstone, TombstoneArena};
use crate::{ConstExpr, Element, ImportId, Module, RefType, Result};
use anyhow::bail;

/// The id of a table.
pub type TableId = Id<Table>;

/// A table in the wasm.
#[derive(Debug)]
pub struct Table {
    id: TableId,
    /// Whether or not this is a 64-bit table.
    pub table64: bool,
    /// The initial size of this table
    pub initial: u64,
    /// The maximum size of this table
    pub maximum: Option<u64>,
    /// The type of the elements in this table
    pub element_ty: RefType,
    /// The initialization expression for the table elements.
    /// Required for non-defaultable (non-nullable) element types.
    /// If None, the table is initialized with null references.
    pub init: Option<ConstExpr>,
    /// Whether or not this table is imported, and if so what imports it.
    pub import: Option<ImportId>,
    /// Active data segments that will be used to initialize this memory.
    pub elem_segments: IdHashSet<Element>,
    /// The name of this table, used for debugging purposes in the `name`
    /// custom section.
    pub name: Option<String>,
}

impl Tombstone for Table {}

impl Table {
    /// Get this table's id.
    pub fn id(&self) -> TableId {
        self.id
    }
}

/// The set of tables in this module.
#[derive(Debug, Default)]
pub struct ModuleTables {
    /// The arena containing this module's tables.
    arena: TombstoneArena<Table>,
}

impl ModuleTables {
    /// Adds a new imported table to this list of tables
    pub fn add_import(
        &mut self,
        table64: bool,
        initial: u64,
        maximum: Option<u64>,
        element_ty: RefType,
        import: ImportId,
    ) -> TableId {
        let id = self.arena.next_id();
        self.arena.alloc(Table {
            id,
            table64,
            initial,
            maximum,
            element_ty,
            init: None,
            import: Some(import),
            elem_segments: Default::default(),
            name: None,
        })
    }

    /// Construct a new table, that does not originate from any of the input
    /// wasm tables.
    pub fn add_local(
        &mut self,
        table64: bool,
        initial: u64,
        maximum: Option<u64>,
        element_ty: RefType,
    ) -> TableId {
        self.add_local_with_init(table64, initial, maximum, element_ty, None)
    }

    /// Construct a new table with an optional initializer expression.
    /// The initializer is required for non-defaultable (non-nullable) element types.
    pub fn add_local_with_init(
        &mut self,
        table64: bool,
        initial: u64,
        maximum: Option<u64>,
        element_ty: RefType,
        init: Option<ConstExpr>,
    ) -> TableId {
        let id = self.arena.next_id();
        let id2 = self.arena.alloc(Table {
            id,
            table64,
            initial,
            maximum,
            element_ty,
            init,
            import: None,
            elem_segments: Default::default(),
            name: None,
        });
        debug_assert_eq!(id, id2);
        id
    }

    /// Returns the actual table associated with an ID
    pub fn get(&self, table: TableId) -> &Table {
        &self.arena[table]
    }

    /// Returns the actual table associated with an ID
    pub fn get_mut(&mut self, table: TableId) -> &mut Table {
        &mut self.arena[table]
    }

    /// Removes a table from this module.
    ///
    /// It is up to you to ensure that any potential references to the deleted
    /// table are also removed, eg `call_indirect` expressions and exports, etc.
    pub fn delete(&mut self, id: TableId) {
        self.arena.delete(id);
    }

    /// Iterates over all tables in this section.
    pub fn iter(&self) -> impl Iterator<Item = &Table> {
        self.arena.iter().map(|p| p.1)
    }

    /// Finds a unique function table in a module.
    ///
    /// Modules produced by compilers like LLVM typically have one function
    /// table for indirect function calls. This function will look for a single
    /// function table inside this module, and return that if found. If no
    /// function tables are present `None` will be returned
    ///
    /// # Errors
    ///
    /// Returns an error if there are two function tables in this module
    pub fn main_function_table(&self) -> Result<Option<TableId>> {
        let mut tables = self.iter().filter(|t| t.element_ty == RefType::FUNCREF);
        let id = match tables.next() {
            Some(t) => t.id(),
            None => return Ok(None),
        };
        if tables.next().is_some() {
            bail!("module contains more than one function table");
        }
        Ok(Some(id))
    }

    /// Iterates over all tables in this section.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Table> {
        self.arena.iter_mut().map(|p| p.1)
    }
}

impl Module {
    /// Construct a new, empty set of tables for a module.
    pub(crate) fn parse_tables(
        &mut self,
        section: wasmparser::TableSectionReader,
        ids: &mut IndicesToIds,
    ) -> Result<()> {
        log::debug!("parse table section");
        for t in section {
            let t = t?;
            let init = match t.init {
                wasmparser::TableInit::RefNull => None,
                wasmparser::TableInit::Expr(expr) => Some(ConstExpr::eval(&expr, ids)?),
            };
            let id = self.tables.add_local_with_init(
                t.ty.table64,
                t.ty.initial,
                t.ty.maximum,
                t.ty.element_type.try_into()?,
                init,
            );
            ids.push_table(id);
        }
        Ok(())
    }
}

impl Emit for ModuleTables {
    fn emit(&self, cx: &mut EmitContext) {
        log::debug!("emit table section");

        let mut wasm_table_section = wasm_encoder::TableSection::new();

        // Skip imported tables because those are emitted in the import section.
        let tables = self.iter().filter(|t| t.import.is_none()).count();
        if tables == 0 {
            return;
        }

        for table in self.iter().filter(|t| t.import.is_none()) {
            cx.indices.push_table(table.id());

            let table_type = wasm_encoder::TableType {
                table64: table.table64,
                minimum: table.initial,
                maximum: table.maximum,
                element_type: table.element_ty.to_wasmencoder_ref_type(),
                shared: false,
            };

            match &table.init {
                Some(init_expr) => {
                    wasm_table_section
                        .table_with_init(table_type, &init_expr.to_wasmencoder_type(cx));
                }
                None => {
                    wasm_table_section.table(table_type);
                }
            }
        }

        cx.wasm_module.section(&wasm_table_section);
    }
}
