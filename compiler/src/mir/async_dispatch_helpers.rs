use std::collections::{HashMap, HashSet};

pub const BUILTIN_ASYNC_DISPATCH_NAMES: [&str; 2] =
    ["sengoo_async_sleep", "sengoo_async_timeout_bool"];

/// Runtime async helpers registered only when MIR lowering references them.
pub const OPTIONAL_ASYNC_DISPATCH_NAMES: [&str; 9] = [
    "sengoo_async_timeout_cancel_i64",
    "sengoo_async_spawn_blocking_i64",
    "sengoo_async_channel_send_i64",
    "sengoo_async_channel_recv_i64",
    "sengoo_async_mutex_lock_i64",
    "sengoo_async_rwlock_read",
    "sengoo_async_rwlock_write",
    "sengoo_http_server_next_request_async",
    "sengoo_async_file_wait_readable",
];

#[derive(Debug, Clone)]
pub struct AsyncDispatchRegistry {
    ids: HashMap<String, i64>,
    collisions: HashSet<String>,
}

impl AsyncDispatchRegistry {
    pub fn kind_id(&self, name: &str) -> Option<i64> {
        (!self.collisions.contains(name))
            .then(|| self.ids.get(name).copied())
            .flatten()
    }
}

fn stable_async_dispatch_kind_id(name: &str) -> i64 {
    if let Some(index) = BUILTIN_ASYNC_DISPATCH_NAMES
        .iter()
        .position(|builtin| *builtin == name)
    {
        return index as i64 + 1;
    }

    let mut hash = 2_166_136_261_u32;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    const NON_BUILTIN_KIND_COUNT: u64 = u32::MAX as u64 - 2;
    (3 + u64::from(hash) % NON_BUILTIN_KIND_COUNT) as i64
}

fn insert_stable_kind(
    ids: &mut HashMap<String, i64>,
    names_by_id: &mut HashMap<i64, String>,
    collisions: &mut HashSet<String>,
    name: String,
) {
    if ids.contains_key(&name) {
        return;
    }
    let id = stable_async_dispatch_kind_id(&name);
    if let Some(existing) = names_by_id.get(&id) {
        if existing != &name {
            collisions.insert(existing.clone());
            collisions.insert(name.clone());
        }
    } else {
        names_by_id.insert(id, name.clone());
    }
    ids.insert(name, id);
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
    let mut names_by_id = HashMap::new();
    let mut collisions = HashSet::new();

    for builtin in BUILTIN_ASYNC_DISPATCH_NAMES {
        insert_stable_kind(
            &mut ids,
            &mut names_by_id,
            &mut collisions,
            builtin.to_string(),
        );
    }

    for name in async_names {
        insert_stable_kind(&mut ids, &mut names_by_id, &mut collisions, name);
    }

    for extra in extras {
        insert_stable_kind(
            &mut ids,
            &mut names_by_id,
            &mut collisions,
            extra.to_string(),
        );
    }

    AsyncDispatchRegistry { ids, collisions }
}
