use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

/// A singleton map that manages handles to instances of type T using thread-safe primitives
pub struct HandleMap<T: Clone> {
    contexts: Arc<Mutex<HashMap<u32, Arc<Mutex<T>>>>>,
    next_handle: AtomicU32,
}

impl<T: Clone> Default for HandleMap<T> {
    fn default() -> Self {
        Self {
            contexts: Arc::new(Mutex::new(HashMap::new())),
            next_handle: AtomicU32::new(1),
        }
    }
}

impl<T: Clone> HandleMap<T> {
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

        contexts.insert(handle, Arc::new(Mutex::new(instance)));
        handle
    }

    /// Remove an instance by its handle
    pub fn remove(&self, handle: u32) -> Option<T> {
        let mut contexts = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");

        contexts.remove(&handle).map(|arc_mutex| {
            // Get the value out of the Arc<Mutex<T>>
            let mutex = Arc::try_unwrap(arc_mutex).unwrap_or_else(|arc| {
                // If we can't get exclusive ownership, clone the inner value
                let guard = arc.lock().expect("Failed to acquire instance lock");
                Mutex::new(guard.clone())
            });

            mutex.into_inner().expect("Failed to unwrap mutex")
        })
    }

    /// Get a clone of the Arc<Mutex<T>> for the instance with the given handle
    pub fn get(&self, handle: u32) -> Option<Arc<Mutex<T>>> {
        let contexts = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");
        contexts.get(&handle).cloned()
    }

    /// Run a function that reads the instance with the given handle
    pub fn with_instance<F, R>(&self, handle: u32, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        let instance_arc = self.get(handle)?;
        let guard = instance_arc
            .lock()
            .expect("Failed to acquire instance lock");
        Some(f(&*guard))
    }

    /// Run a function that modifies the instance with the given handle
    pub fn with_instance_mut<F, R>(&self, handle: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let instance_arc = self.get(handle)?;
        let mut guard = instance_arc
            .lock()
            .expect("Failed to acquire instance lock");
        Some(f(&mut *guard))
    }

    /// Check if a handle exists
    #[allow(dead_code)]
    pub fn contains(&self, handle: u32) -> bool {
        let contexts = self
            .contexts
            .lock()
            .expect("Failed to acquire contexts lock");

        contexts.contains_key(&handle)
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

        // Access instances with read-only function
        HANDLES.with_instance(h1, |s| {
            assert_eq!(s, "test1");
        });

        HANDLES.with_instance(h2, |s| {
            assert_eq!(s, "test2");
        });

        // Modify instance
        HANDLES.with_instance_mut(h1, |s| {
            *s = "modified".to_string();
        });

        // Verify modification
        HANDLES.with_instance(h1, |s| {
            assert_eq!(s, "modified");
        });

        // Remove instance
        let removed = HANDLES.remove(h1).unwrap();
        assert_eq!(removed, "modified");
        assert!(!HANDLES.contains(h1));
    }
}
