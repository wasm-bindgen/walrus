use crate::tombstone_arena::{Tombstone, TombstoneArena};
use id_arena::Id;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops;

/// A set of unique `T`s that are backed by an arena.
#[derive(Debug)]
pub struct ArenaSet<T: Clone + Eq + Hash> {
    arena: TombstoneArena<T>,
    already_in_arena: HashMap<T, Id<T>>,
}

impl<T: Clone + Eq + Hash> ArenaSet<T> {
    /// Construct a new set.
    pub fn new() -> ArenaSet<T> {
        ArenaSet {
            arena: TombstoneArena::default(),
            already_in_arena: HashMap::new(),
        }
    }

    /// Insert a value into the arena and get its id.
    pub fn insert(&mut self, val: T) -> Id<T> {
        if let Some(id) = self.already_in_arena.get(&val) {
            return *id;
        }

        let id = self.arena.alloc(val.clone());
        self.already_in_arena.insert(val, id);
        id
    }

    /// Get the id that will be used for the next unique item added to this set.
    pub fn next_id(&self) -> Id<T> {
        self.arena.next_id()
    }

    /// Allocate a value without deduplication, returning a fresh Id.
    ///
    /// Used during parsing where each type index must get its own unique Id
    /// regardless of structural equality (e.g., for forward references in
    /// rec groups).
    pub(crate) fn alloc_unique(&mut self, val: T) -> Id<T> {
        self.arena.alloc(val)
        // Deliberately does NOT update already_in_arena
    }

    /// Replace the value at an existing Id and register it in the dedup map.
    ///
    /// Used after `alloc_unique()` to finalize a pre-allocated slot with
    /// its real value once all forward references can be resolved.
    pub(crate) fn replace_and_register(&mut self, id: Id<T>, val: T) {
        self.arena[id] = val.clone();
        self.already_in_arena.insert(val, id);
    }

    /// Check whether the given id is still live (not deleted / tombstoned).
    pub fn contains(&self, id: Id<T>) -> bool {
        self.arena.contains(id)
    }

    /// Remove an item from this set
    pub fn remove(&mut self, id: Id<T>)
    where
        T: Tombstone,
    {
        self.already_in_arena.remove(&self.arena[id]);
        self.arena.delete(id);
    }

    /// Iterate over the items in this arena and their ids.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.arena.iter()
    }
}

impl<T: Clone + Eq + Hash> ops::Index<Id<T>> for ArenaSet<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: Id<T>) -> &T {
        &self.arena[id]
    }
}

impl<T: Clone + Eq + Hash> ops::IndexMut<Id<T>> for ArenaSet<T> {
    #[inline]
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        &mut self.arena[id]
    }
}

impl<T: Clone + Eq + Hash> Default for ArenaSet<T> {
    fn default() -> ArenaSet<T> {
        ArenaSet::new()
    }
}
