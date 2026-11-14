//! The evictable texture cache behind [`crate::TextureStore`]: handles,
//! case-folded keys, ages and the image/byte budget.
//!
//! What the C++ counts and when it evicts (plugin.cpp:3135-3156,
//! 2772-2843): before loading a texture that is not cached, while the
//! number of evictable cached textures is `>= m_nMaxImages` (3000) or their
//! summed `nSizeInBytes` is `>= m_nMaxBytes` (2e9), it evicts one; it gives
//! up when nothing is evictable and loads anyway. `nSizeInBytes` is
//! `w * h * bits_per_pixel / 8 + 16384` (mips not counted). Generated
//! textures (noise, blur) are not evictable and are not counted — here they
//! live outside this cache. A texture's age is stamped with
//! `m_nPresetsLoadedTotal` when loaded or bound; textures whose age is
//! `>= m_nPresetsLoadedTotal - 1` — those of the preset being loaded and of
//! the blend-from preset — are never evicted.
//!
//! Which texture goes: the C++ picks the "biggest" by a size weighted with
//! age, but its weight grows with recency (the opposite of its comment) and
//! is `0/0` when all candidates share an age, in which case nothing is
//! evicted. Per the brief we evict least-recently-used instead: the oldest
//! age, ties broken by the larger size, then by the earlier load.

use std::collections::HashMap;

/// `m_nMaxImages` (plugin.cpp:985).
pub const MAX_IMAGES: usize = 3000;
/// `m_nMaxBytes` (plugin.cpp:986).
pub const MAX_BYTES: u64 = 2_000_000_000;
/// The per-texture overhead the C++ adds to `nSizeInBytes`.
pub const TEXTURE_OVERHEAD_BYTES: u64 = 16384;

/// A stable handle to a cached texture. It goes stale when the texture is
/// evicted (lookups then return `None`); ids are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub(crate) u32);

struct Entry<T> {
    key: String,
    value: T,
    bytes: u64,
    age: u64,
}

pub(crate) struct TexCache<T> {
    entries: HashMap<u32, Entry<T>>,
    by_key: HashMap<String, u32>,
    next_id: u32,
    /// `m_nPresetsLoadedTotal`.
    generation: u64,
    pub max_images: usize,
    pub max_bytes: u64,
}

/// Case-folded cache key (texture names are case-insensitive, as file
/// names are on Windows).
pub(crate) fn fold(key: &str) -> String {
    key.to_lowercase()
}

impl<T> TexCache<T> {
    pub fn new() -> TexCache<T> {
        TexCache::starting_at(0)
    }

    /// An empty cache whose ids start at `first_id` (a second cache whose
    /// handles must not collide with another's).
    pub fn starting_at(first_id: u32) -> TexCache<T> {
        TexCache {
            entries: HashMap::new(),
            by_key: HashMap::new(),
            next_id: first_id,
            generation: 0,
            max_images: MAX_IMAGES,
            max_bytes: MAX_BYTES,
        }
    }

    /// The cached id for `key`, stamping its age (a cache hit in
    /// `CacheParams`, plugin.cpp:3117-3124).
    pub fn lookup(&mut self, key: &str) -> Option<TextureId> {
        let id = *self.by_key.get(&fold(key))?;
        if let Some(e) = self.entries.get_mut(&id) {
            e.age = self.generation;
        }
        Some(TextureId(id))
    }

    /// The cached id for `key` without touching its age.
    pub fn peek(&self, key: &str) -> Option<TextureId> {
        self.by_key.get(&fold(key)).map(|&id| TextureId(id))
    }

    /// Drop the entry `id` (a stale handle afterwards); `false` if absent.
    pub fn remove(&mut self, id: TextureId) -> bool {
        match self.entries.remove(&id.0) {
            Some(e) => {
                self.by_key.remove(&e.key);
                true
            }
            None => false,
        }
    }

    pub fn get(&self, id: TextureId) -> Option<&T> {
        self.entries.get(&id.0).map(|e| &e.value)
    }

    /// Evict until a new texture fits the budget, or nothing more can go.
    pub fn make_room(&mut self) {
        loop {
            let (n, bytes) = self.usage();
            if n < self.max_images && bytes < self.max_bytes {
                return;
            }
            if !self.evict_one() {
                return;
            }
        }
    }

    /// Insert `value` under `key` (replacing any entry with that key),
    /// aged now. Call [`TexCache::make_room`] first, as the C++ does.
    pub fn insert(&mut self, key: &str, value: T, bytes: u64) -> TextureId {
        let key = fold(key);
        if let Some(old) = self.by_key.remove(&key) {
            self.entries.remove(&old);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.by_key.insert(key.clone(), id);
        let age = self.generation;
        self.entries.insert(
            id,
            Entry {
                key,
                value,
                bytes,
                age,
            },
        );
        TextureId(id)
    }

    /// `m_nPresetsLoadedTotal++` on completing a preset load.
    pub fn preset_loaded(&mut self) {
        self.generation += 1;
    }

    /// (textures cached, bytes counted).
    pub fn usage(&self) -> (usize, u64) {
        let bytes = self.entries.values().map(|e| e.bytes).sum();
        (self.entries.len(), bytes)
    }

    /// Evict the least-recently-used texture not in use by the current or
    /// the blend-from preset; `false` when there is none.
    pub fn evict_one(&mut self) -> bool {
        let protect_from = self.generation.saturating_sub(1);
        let victim = self
            .entries
            .iter()
            .filter(|(_, e)| e.age < protect_from)
            .min_by(|(ia, a), (ib, b)| {
                a.age
                    .cmp(&b.age)
                    .then(b.bytes.cmp(&a.bytes))
                    .then(ia.cmp(ib))
            })
            .map(|(&id, _)| id);
        let Some(id) = victim else {
            return false;
        };
        if let Some(e) = self.entries.remove(&id) {
            self.by_key.remove(&e.key);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(max_images: usize, max_bytes: u64) -> TexCache<()> {
        let mut c = TexCache::new();
        c.max_images = max_images;
        c.max_bytes = max_bytes;
        c
    }

    fn load(c: &mut TexCache<()>, key: &str, bytes: u64) -> TextureId {
        c.make_room();
        c.insert(key, (), bytes)
    }

    #[test]
    fn defaults_match_cpp() {
        let c = TexCache::<()>::new();
        assert_eq!((c.max_images, c.max_bytes), (3000, 2_000_000_000));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let mut c = cache(10, 1000);
        let id = load(&mut c, "Heart", 1);
        assert_eq!(c.lookup("hEART"), Some(id));
        assert_eq!(c.peek("HEART"), Some(id));
    }

    #[test]
    fn lru_eviction_respects_budget() {
        let mut c = cache(2, u64::MAX);
        let a = load(&mut c, "a", 10); // generation 0
        c.preset_loaded();
        let b = load(&mut c, "b", 10); // generation 1
        c.preset_loaded();
        c.preset_loaded();
        // Generation 3: a (age 0) and b (age 1) are both evictable; a goes.
        let d = load(&mut c, "d", 10);
        assert!(c.get(a).is_none() && c.peek("a").is_none());
        assert!(c.get(b).is_some() && c.get(d).is_some());
        assert_eq!(c.usage(), (2, 20));
    }

    #[test]
    fn current_and_blend_from_presets_are_protected() {
        let mut c = cache(1, u64::MAX);
        load(&mut c, "a", 10); // generation 0
        c.preset_loaded();
        // Generation 1: a belongs to the blend-from preset -> kept, the
        // new texture loads anyway (the C++ gives up and loads).
        load(&mut c, "b", 10);
        assert_eq!(c.usage().0, 2);
        c.preset_loaded();
        // Generation 2: a (age 0) is evictable, b (age 1) is protected.
        c.make_room();
        assert!(c.peek("a").is_none() && c.peek("b").is_some());
    }

    #[test]
    fn byte_budget_and_rebinding_refreshes_age() {
        let mut c = cache(100, 25);
        load(&mut c, "a", 10);
        load(&mut c, "b", 10);
        c.preset_loaded();
        c.preset_loaded();
        // Generation 2: rebinding a stamps it, so b is the LRU.
        assert!(c.lookup("a").is_some());
        c.preset_loaded();
        c.preset_loaded();
        // 20 < 25: no eviction needed yet.
        load(&mut c, "c", 10);
        assert_eq!(c.usage(), (3, 30));
        // 30 >= 25: evict b (age 0) before a (age 2), then 20 < 25.
        load(&mut c, "d", 10);
        assert!(c.peek("b").is_none());
        assert!(c.peek("a").is_some() && c.peek("c").is_some());
        assert_eq!(c.usage(), (3, 30));
    }

    #[test]
    fn ties_evict_the_larger_texture() {
        let mut c = cache(2, u64::MAX);
        load(&mut c, "small", 10);
        load(&mut c, "big", 99);
        c.preset_loaded();
        c.preset_loaded();
        load(&mut c, "new", 1);
        assert!(c.peek("big").is_none() && c.peek("small").is_some());
    }
}
