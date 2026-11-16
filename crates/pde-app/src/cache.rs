//! Cache des images de pages déjà rendues, limité en mémoire : les pages
//! les moins récemment demandées sont oubliées en premier (LRU).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use pde_core::document::{DocumentId, PageIndex};

/// Image d'une page à un zoom donné (zoom en centièmes : 1,5 → 150).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageKey {
    pub doc: DocumentId,
    pub page: PageIndex,
    pub scale_centi: u32,
}

pub struct PageCache {
    budget: usize,
    used: usize,
    entries: HashMap<PageKey, Arc<Vec<u8>>>,
    /// Du moins récemment utilisé au plus récent.
    order: VecDeque<PageKey>,
}

impl PageCache {
    /// Cache de `budget` octets au plus.
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Nouveau budget ; les images les plus anciennes partent si besoin.
    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        while self.used > self.budget {
            match self.order.pop_front() {
                Some(oldest) => self.forget(&oldest),
                None => break,
            }
        }
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&mut self, key: &PageKey) -> Option<Arc<Vec<u8>>> {
        let image = self.entries.get(key)?.clone();
        self.touch(key);
        Some(image)
    }

    /// Ajoute une image ; une image plus grosse que tout le cache n'est pas gardée.
    pub fn insert(&mut self, key: PageKey, image: Arc<Vec<u8>>) {
        self.remove(&key);
        if image.len() > self.budget {
            return;
        }
        while self.used + image.len() > self.budget {
            match self.order.pop_front() {
                Some(oldest) => self.forget(&oldest),
                None => break,
            }
        }
        self.used += image.len();
        self.entries.insert(key, image);
        self.order.push_back(key);
    }

    /// Oublie toutes les pages d'un document (à sa fermeture).
    pub fn remove_document(&mut self, doc: DocumentId) {
        let keys: Vec<PageKey> = self
            .order
            .iter()
            .filter(|k| k.doc == doc)
            .copied()
            .collect();
        for key in keys {
            self.remove(&key);
        }
    }

    fn remove(&mut self, key: &PageKey) {
        if self.entries.contains_key(key) {
            self.order.retain(|k| k != key);
            self.forget(key);
        }
    }

    fn forget(&mut self, key: &PageKey) {
        if let Some(image) = self.entries.remove(key) {
            self.used -= image.len();
        }
    }

    fn touch(&mut self, key: &PageKey) {
        self.order.retain(|k| k != key);
        self.order.push_back(*key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(doc: u64, page: usize) -> PageKey {
        PageKey {
            doc: DocumentId(doc),
            page,
            scale_centi: 100,
        }
    }

    fn image(len: usize) -> Arc<Vec<u8>> {
        Arc::new(vec![0; len])
    }

    #[test]
    fn keeps_images_within_budget() {
        let mut cache = PageCache::new(10);
        cache.insert(key(1, 0), image(4));
        cache.insert(key(1, 1), image(4));
        cache.insert(key(1, 2), image(4)); // oublie la page 0
        assert_eq!(cache.used(), 8);
        assert!(cache.get(&key(1, 0)).is_none());
        assert!(cache.get(&key(1, 1)).is_some());
    }

    #[test]
    fn recently_read_images_are_kept() {
        let mut cache = PageCache::new(10);
        cache.insert(key(1, 0), image(4));
        cache.insert(key(1, 1), image(4));
        cache.get(&key(1, 0)); // la page 1 devient la plus ancienne
        cache.insert(key(1, 2), image(4));
        assert!(cache.get(&key(1, 0)).is_some());
        assert!(cache.get(&key(1, 1)).is_none());
    }

    #[test]
    fn oversized_image_is_not_cached() {
        let mut cache = PageCache::new(10);
        cache.insert(key(1, 0), image(4));
        cache.insert(key(1, 1), image(11));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used(), 4);
    }

    #[test]
    fn replacing_an_image_updates_the_size() {
        let mut cache = PageCache::new(10);
        cache.insert(key(1, 0), image(4));
        cache.insert(key(1, 0), image(6));
        assert_eq!((cache.len(), cache.used()), (1, 6));
    }

    #[test]
    fn smaller_budget_evicts_oldest_images() {
        let mut cache = PageCache::new(100);
        cache.insert(key(1, 0), image(40));
        cache.insert(key(1, 1), image(40));
        cache.set_budget(50);
        assert_eq!((cache.len(), cache.used()), (1, 40));
        assert!(cache.get(&key(1, 1)).is_some());
    }

    #[test]
    fn closing_a_document_forgets_its_pages() {
        let mut cache = PageCache::new(100);
        cache.insert(key(1, 0), image(4));
        cache.insert(key(2, 0), image(4));
        cache.insert(key(1, 1), image(4));
        cache.remove_document(DocumentId(1));
        assert_eq!((cache.len(), cache.used()), (1, 4));
        assert!(cache.get(&key(2, 0)).is_some());
        assert!(!cache.is_empty());
    }
}
