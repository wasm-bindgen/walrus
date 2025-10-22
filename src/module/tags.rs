//! Tags for exception handling

use crate::emit::{Emit, EmitContext};
use crate::module::imports::ImportId;
use crate::parse::IndicesToIds;
use crate::tombstone_arena::{Id, Tombstone, TombstoneArena};
use crate::TypeId;
use anyhow::Result;

/// The id of a tag.
pub type TagId = Id<Tag>;

/// A tag in a WebAssembly module, used for exception handling.
#[derive(Debug)]
pub struct Tag {
    /// The id of this tag.
    pub id: TagId,
    /// The type signature of this tag (function type).
    pub ty: TypeId,
    /// The kind of tag (imported or local).
    pub kind: TagKind,
}

/// The kind of tag.
#[derive(Debug)]
pub enum TagKind {
    /// An imported tag.
    Import(ImportId),
    /// A locally defined tag.
    Local,
}

impl Tag {
    /// Create a new local tag with the given type.
    pub fn new(id: TagId, ty: TypeId) -> Tag {
        Tag {
            id,
            ty,
            kind: TagKind::Local,
        }
    }

    /// Get the id of this tag.
    pub fn id(&self) -> TagId {
        self.id
    }

    /// Get the type of this tag.
    pub fn ty(&self) -> TypeId {
        self.ty
    }
}

impl Tombstone for Tag {
    fn on_delete(&mut self) {
        // No resources to clean up
    }
}

impl Emit for ModuleTags {
    fn emit(&self, cx: &mut EmitContext) {
        log::debug!("emit tag section");

        let tags: Vec<_> = self
            .iter()
            .filter(|t| matches!(t.kind, TagKind::Local))
            .collect();

        if tags.is_empty() {
            return;
        }

        let mut tag_section = wasm_encoder::TagSection::new();

        for tag in tags {
            cx.indices.push_tag(tag.id);
            let ty_idx = cx.indices.get_type_index(tag.ty);
            tag_section.tag(wasm_encoder::TagType {
                kind: wasm_encoder::TagKind::Exception,
                func_type_idx: ty_idx,
            });
        }

        cx.wasm_module.section(&tag_section);
    }
}

/// All tags in a WebAssembly module.
#[derive(Debug, Default)]
pub struct ModuleTags {
    /// Arena for tags.
    arena: TombstoneArena<Tag>,
}

impl ModuleTags {
    /// Create a new empty tag arena.
    pub fn new() -> ModuleTags {
        ModuleTags::default()
    }

    /// Add a new tag to this module.
    pub fn add(&mut self, ty: TypeId) -> TagId {
        let id = self.arena.next_id();
        let tag = Tag::new(id, ty);
        self.arena.alloc(tag)
    }

    /// Add an imported tag to this module.
    pub fn add_import(&mut self, ty: TypeId, import: ImportId) -> TagId {
        let id = self.arena.next_id();
        let tag = Tag {
            id,
            ty,
            kind: TagKind::Import(import),
        };
        self.arena.alloc(tag)
    }

    /// Get a tag by id.
    pub fn get(&self, id: TagId) -> &Tag {
        &self.arena[id]
    }

    /// Get a mutable reference to a tag by id.
    pub fn get_mut(&mut self, id: TagId) -> &mut Tag {
        &mut self.arena[id]
    }

    /// Get an iterator over all tags.
    pub fn iter(&self) -> impl Iterator<Item = &Tag> {
        self.arena.iter().map(|(_, tag)| tag)
    }

    /// Get an iterator over all tags with their IDs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Tag> {
        self.arena.iter_mut().map(|(_, tag)| tag)
    }

    /// Delete a tag from this module.
    pub fn delete(&mut self, id: TagId) {
        self.arena.delete(id);
    }
}

impl Module {
    /// Parse the tag section of a wasm module.
    pub(crate) fn parse_tags(
        &mut self,
        section: wasmparser::TagSectionReader,
        ids: &mut IndicesToIds,
    ) -> Result<()> {
        log::debug!("parse tag section");
        for tag in section {
            let tag = tag?;
            // Currently Exception is the only TagKind variant
            let wasmparser::TagKind::Exception = tag.kind;
            let ty = ids.get_type(tag.func_type_idx)?;
            let tag_id = self.tags.add(ty);
            ids.push_tag(tag_id);
        }
        Ok(())
    }
}

use crate::Module;
