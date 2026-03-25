use crate::const_expr::ConstOp;
use crate::ir::*;
use crate::map::IdHashSet;
use crate::ty::HeapType;
use crate::{ConstExpr, Data, DataId, DataKind, Element, ExportItem, Function};
use crate::{ElementId, ElementItems, ElementKind, Module, RefType, Tag, TagId, Type, TypeId};
use crate::{FunctionId, FunctionKind, Global, GlobalId};
use crate::{GlobalKind, Memory, MemoryId, Table, TableId};

/// Set of all root used items in a wasm module.
#[derive(Debug, Default)]
pub struct Roots {
    tables: Vec<TableId>,
    funcs: Vec<FunctionId>,
    globals: Vec<GlobalId>,
    memories: Vec<MemoryId>,
    tags: Vec<TagId>,
    datas: Vec<DataId>,
    elements: Vec<ElementId>,
    used: Used,
}

impl Roots {
    /// Creates a new set of empty roots.
    pub fn new() -> Roots {
        Roots::default()
    }

    /// Adds a new function to the set of roots
    pub fn push_func(&mut self, func: FunctionId) -> &mut Roots {
        if self.used.funcs.insert(func) {
            log::trace!("function is used: {:?}", func);
            self.funcs.push(func);
        }
        self
    }

    /// Adds a new table to the set of roots
    pub fn push_table(&mut self, table: TableId) -> &mut Roots {
        if self.used.tables.insert(table) {
            log::trace!("table is used: {:?}", table);
            self.tables.push(table);
        }
        self
    }

    /// Adds a new memory to the set of roots
    pub fn push_memory(&mut self, memory: MemoryId) -> &mut Roots {
        if self.used.memories.insert(memory) {
            log::trace!("memory is used: {:?}", memory);
            self.memories.push(memory);
        }
        self
    }

    /// Adds a new global to the set of roots
    pub fn push_global(&mut self, global: GlobalId) -> &mut Roots {
        if self.used.globals.insert(global) {
            log::trace!("global is used: {:?}", global);
            self.globals.push(global);
        }
        self
    }

    /// Adds a new tag to the set of roots
    pub fn push_tag(&mut self, tag: TagId) -> &mut Roots {
        if self.used.tags.insert(tag) {
            log::trace!("tag is used: {:?}", tag);
            self.tags.push(tag);
        }
        self
    }

    fn push_data(&mut self, data: DataId) -> &mut Roots {
        if self.used.data.insert(data) {
            log::trace!("data is used: {:?}", data);
            self.datas.push(data);
        }
        self
    }

    fn push_element(&mut self, element: ElementId) -> &mut Roots {
        if self.used.elements.insert(element) {
            log::trace!("element is used: {:?}", element);
            self.elements.push(element);
        }
        self
    }

    fn push_const_expr(&mut self, expr: &ConstExpr) {
        match expr {
            ConstExpr::Global(g) => {
                self.push_global(*g);
            }
            ConstExpr::RefFunc(f) => {
                self.push_func(*f);
            }
            ConstExpr::RefNull(ref_type) => {
                mark_ref_type_used(&mut self.used, ref_type);
            }
            ConstExpr::Value(_) => {}
            ConstExpr::Extended(ops) => {
                for op in ops {
                    match op {
                        ConstOp::GlobalGet(g) => {
                            self.push_global(*g);
                        }
                        ConstOp::RefFunc(f) => {
                            self.push_func(*f);
                        }
                        ConstOp::StructNew(ty)
                        | ConstOp::StructNewDefault(ty)
                        | ConstOp::ArrayNew(ty)
                        | ConstOp::ArrayNewDefault(ty) => {
                            self.used.types.insert(*ty);
                        }
                        ConstOp::ArrayNewFixed { ty, .. } => {
                            self.used.types.insert(*ty);
                        }
                        ConstOp::RefNull(ref_type) => {
                            mark_ref_type_used(&mut self.used, ref_type);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Finds the things within a module that are used.
///
/// This is useful for implementing something like a linker's `--gc-sections` so
/// that our emitted `.wasm` binaries are small and don't contain things that
/// are not used.
#[derive(Debug, Default)]
pub struct Used {
    /// The module's used tables.
    pub tables: IdHashSet<Table>,
    /// The module's used types.
    pub types: IdHashSet<Type>,
    /// The module's used functions.
    pub funcs: IdHashSet<Function>,
    /// The module's used globals.
    pub globals: IdHashSet<Global>,
    /// The module's used memories.
    pub memories: IdHashSet<Memory>,
    /// The module's used tags.
    pub tags: IdHashSet<Tag>,
    /// The module's used passive element segments.
    pub elements: IdHashSet<Element>,
    /// The module's used passive data segments.
    pub data: IdHashSet<Data>,
}

impl Used {
    /// Construct a new `Used` set for the given module.
    pub fn new(module: &Module) -> Used {
        log::debug!("starting to calculate used set");
        let mut stack = Roots::default();

        // All exports are roots
        for export in module.exports.iter() {
            match export.item {
                ExportItem::Function(f) => stack.push_func(f),
                ExportItem::Table(t) => stack.push_table(t),
                ExportItem::Memory(m) => stack.push_memory(m),
                ExportItem::Global(g) => stack.push_global(g),
                ExportItem::Tag(t) => stack.push_tag(t),
            };
        }

        // The start function is an implicit root as well
        if let Some(f) = module.start {
            stack.push_func(f);
        }

        // Initialization of memories or tables is a side-effectful operation
        // because they can be out-of-bounds, so keep all active segments.
        for data in module.data.iter() {
            if let DataKind::Active { .. } = &data.kind {
                stack.push_data(data.id());
            }
        }
        for elem in module.elements.iter() {
            match elem.kind {
                // Active segments are rooted because they initialize imported
                // tables.
                ElementKind::Active { table, .. } => {
                    if module.tables.get(table).import.is_some() {
                        stack.push_element(elem.id());
                    }
                }
                // Declared segments can probably get gc'd but for now we're
                // conservative and we root them
                ElementKind::Declared => {
                    stack.push_element(elem.id());
                }
                ElementKind::Passive => {}
            }
        }

        // And finally ask custom sections for their roots
        for (_id, section) in module.customs.iter() {
            section.add_gc_roots(&mut stack);
        }

        // Iteratively visit all items until our stack is empty
        while !stack.funcs.is_empty()
            || !stack.tables.is_empty()
            || !stack.memories.is_empty()
            || !stack.globals.is_empty()
            || !stack.tags.is_empty()
            || !stack.datas.is_empty()
            || !stack.elements.is_empty()
        {
            while let Some(f) = stack.funcs.pop() {
                let func = module.funcs.get(f);
                stack.used.types.insert(func.ty());

                match &func.kind {
                    FunctionKind::Local(func) => {
                        let mut visitor = UsedVisitor {
                            stack: &mut stack,
                            local_ids: Vec::new(),
                        };
                        dfs_in_order(&mut visitor, func, func.entry_block());
                        // Mark types referenced by used locals (non-arg locals
                        // may have concrete ref types that need to survive GC).
                        for local_id in visitor.local_ids {
                            let ty = module.locals.get(local_id).ty();
                            mark_val_type_used(&mut stack.used, &ty);
                        }
                    }
                    FunctionKind::Import(_) => {}
                    FunctionKind::Uninitialized(_) => unreachable!(),
                }
            }

            while let Some(t) = stack.tables.pop() {
                let table = module.tables.get(t);
                mark_ref_type_used(&mut stack.used, &table.element_ty);
                // Process the table's init expression for function/type refs.
                if let Some(init) = &table.init {
                    stack.push_const_expr(init);
                }
                for elem in table.elem_segments.iter() {
                    stack.push_element(*elem);
                }
            }

            while let Some(t) = stack.globals.pop() {
                let global = module.globals.get(t);
                mark_val_type_used(&mut stack.used, &global.ty);
                match &global.kind {
                    GlobalKind::Import(_) => {}
                    GlobalKind::Local(init) => {
                        stack.push_const_expr(init);
                    }
                }
            }

            while let Some(t) = stack.tags.pop() {
                let tag = module.tags.get(t);
                stack.used.types.insert(tag.ty);
            }

            while let Some(t) = stack.memories.pop() {
                for data in &module.memories.get(t).data_segments {
                    stack.push_data(*data);
                }
            }

            while let Some(d) = stack.datas.pop() {
                let d = module.data.get(d);
                if let DataKind::Active { memory, offset } = &d.kind {
                    stack.push_memory(*memory);
                    stack.push_const_expr(offset);
                }
            }

            while let Some(e) = stack.elements.pop() {
                let e = module.elements.get(e);
                if let ElementItems::Functions(function_ids) = &e.items {
                    function_ids.iter().for_each(|f| {
                        stack.push_func(*f);
                    });
                }
                if let ElementItems::Expressions(ref_type, items) = &e.items {
                    // Mark the element segment's declared type as used.
                    mark_ref_type_used(&mut stack.used, ref_type);
                    // Process all items regardless of element type.
                    for item in items {
                        stack.push_const_expr(item);
                    }
                }
                if let ElementKind::Active { offset, table } = &e.kind {
                    stack.push_const_expr(offset);
                    stack.push_table(*table);
                }
            }
        }

        // Transitively mark all types referenced by used types. GC types
        // (structs, arrays) and function types with ref-typed parameters can
        // reference other types via HeapType::Concrete(TypeId). Without this
        // transitive closure, the GC pass would delete types that are still
        // needed during emission.
        //
        // Additionally, rec groups are atomic: if any member is used, all
        // members must survive. We expand each newly-used type to include
        // its entire rec group.
        {
            let mut type_stack: Vec<TypeId> = stack.used.types.iter().copied().collect();
            let mut refs = Vec::new();
            while let Some(type_id) = type_stack.pop() {
                // Expand rec group: if this type belongs to a rec group,
                // mark all sibling types in the group as used too.
                if let Some(rg) = module.types.rec_group_for_type(type_id) {
                    for &sibling in &rg.types {
                        if stack.used.types.insert(sibling) {
                            type_stack.push(sibling);
                        }
                    }
                }

                let ty = module.types.get(type_id);
                refs.clear();
                ty.referenced_types(&mut refs);
                for &ref_id in &refs {
                    if stack.used.types.insert(ref_id) {
                        type_stack.push(ref_id);
                    }
                }
            }
        }

        // Wabt seems to have weird behavior where a `data` segment, if present
        // even if passive, requires a `memory` declaration. Our GC pass is
        // pretty aggressive and if you have a passive data segment and only
        // `data.drop` instructions you technically don't need the `memory`.
        // Let's keep `wabt` passing though and just say that if there are data
        // segments kept, but no memories, then we try to add the first memory,
        // if any, to the used set.
        if !stack.used.data.is_empty() && stack.used.memories.is_empty() {
            if let Some(mem) = module.memories.iter().next() {
                stack.used.memories.insert(mem.id());
            }
        }

        stack.used
    }
}

struct UsedVisitor<'a> {
    stack: &'a mut Roots,
    local_ids: Vec<LocalId>,
}

impl<'expr> Visitor<'expr> for UsedVisitor<'_> {
    fn visit_function_id(&mut self, &func: &FunctionId) {
        self.stack.push_func(func);
    }

    fn visit_memory_id(&mut self, &m: &MemoryId) {
        self.stack.push_memory(m);
    }

    fn visit_global_id(&mut self, &g: &GlobalId) {
        self.stack.push_global(g);
    }

    fn visit_table_id(&mut self, &t: &TableId) {
        self.stack.push_table(t);
    }

    fn visit_type_id(&mut self, &t: &TypeId) {
        self.stack.used.types.insert(t);
    }

    fn visit_local_id(&mut self, &id: &LocalId) {
        self.local_ids.push(id);
    }

    fn visit_data_id(&mut self, &d: &DataId) {
        self.stack.push_data(d);
    }

    fn visit_element_id(&mut self, &e: &ElementId) {
        self.stack.push_element(e);
    }

    fn visit_tag_id(&mut self, &tag: &TagId) {
        self.stack.push_tag(tag);
    }

    // RefType/ValType fields on Select and RefNull are still
    // #[walrus(skip_visit)] so we handle them manually here.

    fn visit_select(&mut self, instr: &Select) {
        if let Some(ty) = &instr.ty {
            mark_val_type_used(&mut self.stack.used, ty);
        }
    }

    fn visit_ref_null(&mut self, instr: &RefNull) {
        mark_ref_type_used(&mut self.stack.used, &instr.ty);
    }
}

/// If the `RefType`'s heap type is concrete, mark that type as used.
fn mark_ref_type_used(used: &mut Used, ref_type: &RefType) {
    if let HeapType::Concrete(type_id) | HeapType::Exact(type_id) = ref_type.heap_type {
        used.types.insert(type_id);
    }
}

/// If the `ValType` is a ref type with a concrete heap type, mark it as used.
fn mark_val_type_used(used: &mut Used, val_type: &crate::ValType) {
    if let crate::ValType::Ref(ref_type) = val_type {
        mark_ref_type_used(used, ref_type);
    }
}
