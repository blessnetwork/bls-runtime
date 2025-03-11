use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, MutexGuard,
    },
};

/// A singleton map that manages handles to instances of type T
pub struct HandleMap<T> {
    contexts: Mutex<HashMap<u32, T>>,
    next_handle: AtomicU32,
}

impl<T> Default for HandleMap<T> {
    fn default() -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            next_handle: AtomicU32::new(1),
        }
    }
}

impl<T> HandleMap<T> {
    /// Generate a new unique handle
    pub fn generate_handle(&self) -> u32 {
        self.next_handle.fetch_add(1, Ordering::SeqCst)
    }

    /// Insert a new instance and get its handle
    pub fn insert(&self, instance: T) -> u32 {
        let handle = self.generate_handle();
        let mut contexts = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");
        contexts.insert(handle, instance);
        handle
    }

    /// Remove an instance by its handle
    pub fn remove(&self, handle: u32) -> Option<T> {
        let mut contexts = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");
        contexts.remove(&handle)
    }

    /// Access a specific instance by handle
    pub fn get(&self, handle: u32) -> Option<InstanceGuard<T>> {
        let guard = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");
        if guard.contains_key(&handle) {
            Some(InstanceGuard { guard, handle })
        } else {
            None
        }
    }
}

/// A guard that provides safe access to a specific instance
pub struct InstanceGuard<'a, T> {
    guard: MutexGuard<'a, HashMap<u32, T>>,
    handle: u32,
}

impl<'a, T> Deref for InstanceGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.get(&self.handle).unwrap()
    }
}

impl<'a, T> DerefMut for InstanceGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.get_mut(&self.handle).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    static HANDLES: LazyLock<HandleMap<String>> = LazyLock::new(HandleMap::default);

    #[test]
    fn test_handle_map() {
        // Insert instances
        let h1 = HANDLES.insert("test1".to_string());
        let h2 = HANDLES.insert("test2".to_string());

        // Access instances
        assert_eq!(&*HANDLES.get(h1).unwrap(), "test1");
        assert_eq!(&*HANDLES.get(h2).unwrap(), "test2");

        // Modify instance
        if let Some(mut guard) = HANDLES.get(h1) {
            *guard = "modified".to_string();
        }

        // Verify modification
        assert_eq!(&*HANDLES.get(h1).unwrap(), "modified");

        // Remove instance
        let removed = HANDLES.remove(h1).unwrap();
        assert_eq!(removed, "modified");
        assert!(HANDLES.get(h1).is_none());
    }
}
