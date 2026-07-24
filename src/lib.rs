use std::collections::HashMap;

pub struct ThunkManager {
    wrapped_symbols: HashMap<String, usize>,
}

impl ThunkManager {
    pub fn new() -> Self {
        Self {
            wrapped_symbols: HashMap::new(),
        }
    }

    pub fn register_thunk(&mut self, name: &str, target_fn: usize) {
        self.wrapped_symbols.insert(name.to_string(), target_fn);
    }

    pub fn get_thunk(&self, name: &str) -> Option<usize> {
        self.wrapped_symbols.get(name).copied()
    }
}
