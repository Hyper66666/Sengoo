use std::collections::{BTreeSet, HashMap};

pub const BUILTIN_ASYNC_DISPATCH_NAMES: [&str; 2] =
    ["sengoo_async_sleep", "sengoo_async_timeout_bool"];

/// Runtime async helpers registered only when MIR lowering references them.
pub const OPTIONAL_ASYNC_DISPATCH_NAMES: [&str; 6] = [
    "sengoo_async_timeout_cancel_i64",
    "sengoo_async_spawn_blocking_i64",
    "sengoo_async_channel_send_i64",
    "sengoo_async_channel_recv_i64",
    "sengoo_async_mutex_lock_i64",
    "sengoo_http_server_next_request_async",
];

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
    build_async_dispatch_registry_with_extras(async_names, [])
}

pub fn build_async_dispatch_registry_with_extras<I, E>(
    async_names: I,
    extras: E,
) -> AsyncDispatchRegistry
where
    I: IntoIterator<Item = String>,
    E: IntoIterator<Item = &'static str>,
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

    for extra in extras {
        if !ids.contains_key(extra) {
            ids.insert(extra.to_string(), next_id);
            next_id += 1;
        }
    }

    AsyncDispatchRegistry { ids }
}
