use std::collections::{BTreeSet, HashMap};

pub const BUILTIN_ASYNC_DISPATCH_NAMES: [&str; 2] =
    ["sengoo_async_sleep", "sengoo_async_timeout_bool"];

#[derive(Debug, Clone)]
pub struct AsyncDispatchRegistry {
    ids: HashMap<String, i64>,
}

impl AsyncDispatchRegistry {
    pub fn kind_id(&self, name: &str) -> Option<i64> {
        self.ids.get(name).copied()
    }
}

pub fn build_async_dispatch_registry<I>(async_names: I) -> AsyncDispatchRegistry
where
    I: IntoIterator<Item = String>,
{
    let mut ids = HashMap::new();
    let mut next_id = 1i64;

    for builtin in BUILTIN_ASYNC_DISPATCH_NAMES {
        ids.insert(builtin.to_string(), next_id);
        next_id += 1;
    }

    let mut sorted_names = BTreeSet::new();
    for name in async_names {
        if !ids.contains_key(&name) {
            sorted_names.insert(name);
        }
    }

    for name in sorted_names {
        ids.insert(name, next_id);
        next_id += 1;
    }

    AsyncDispatchRegistry { ids }
}
