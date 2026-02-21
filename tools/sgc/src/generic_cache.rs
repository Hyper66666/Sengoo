use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    BuildGraphV2, GenericInstanceCacheEntry, GenericInstanceCacheMetadata,
    GenericInstanceFingerprint, GenericInstancePlanStats, GenericItemFingerprint,
    GENERIC_INSTANCE_CACHE_SCHEMA_VERSION,
};

const DEFAULT_GENERIC_CACHE_MAX_ENTRIES: usize = 8_192;
const DEFAULT_GENERIC_CACHE_MAX_AGE_DAYS: u64 = 14;
const MILLIS_PER_DAY: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy)]
struct GenericCacheRetentionPolicy {
    max_entries: usize,
    max_age_ms: u64,
}

fn now_unix_ms_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn parse_env_usize(key: &str, default_value: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn parse_env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn retention_policy_from_env() -> GenericCacheRetentionPolicy {
    let max_entries = parse_env_usize(
        "SGC_GENERIC_CACHE_MAX_ENTRIES",
        DEFAULT_GENERIC_CACHE_MAX_ENTRIES,
    );
    let max_age_days = parse_env_u64(
        "SGC_GENERIC_CACHE_MAX_AGE_DAYS",
        DEFAULT_GENERIC_CACHE_MAX_AGE_DAYS,
    );
    GenericCacheRetentionPolicy {
        max_entries,
        max_age_ms: max_age_days.saturating_mul(MILLIS_PER_DAY),
    }
}

pub(crate) fn generic_instance_cache_path(build_dir: &Path, stem: &str) -> PathBuf {
    build_dir
        .join("workset")
        .join(format!("{}.generic-instance-cache.json", stem))
}

pub(crate) fn load_generic_instance_cache(path: &Path) -> Option<GenericInstanceCacheMetadata> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn save_generic_instance_cache(
    path: &Path,
    metadata: &GenericInstanceCacheMetadata,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize generic instance cache: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write generic instance cache: {}", e))
}

fn target_triple() -> String {
    format!(
        "{}-{}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        std::env::consts::FAMILY
    )
}

fn feature_flags_signature(flags: &[String]) -> String {
    let mut normalized = flags.to_vec();
    normalized.sort();
    normalized.dedup();
    normalized.join(",")
}

fn canonical_instance_key(
    instance: &GenericInstanceFingerprint,
    opt_level: u8,
    target: &str,
    feature_flags: &[String],
) -> String {
    format!(
        "{}|body={}|target={}|opt={}|features={}|compiler={}",
        instance.instance_key,
        instance.body_hash,
        target,
        opt_level,
        feature_flags_signature(feature_flags),
        env!("CARGO_PKG_VERSION")
    )
}

fn build_generic_item_map(
    graph: &BuildGraphV2,
    keep_item_ids: Option<&HashSet<String>>,
) -> HashMap<String, GenericItemFingerprint> {
    let mut map = HashMap::new();
    for node in &graph.nodes {
        for item in &node.generic_items {
            if let Some(keep) = keep_item_ids {
                if !keep.contains(&item.stable_item_id) {
                    continue;
                }
            }
            map.insert(item.stable_item_id.clone(), item.clone());
        }
    }
    map
}

fn build_generic_instances(graph: &BuildGraphV2) -> Vec<GenericInstanceFingerprint> {
    let mut instances = graph
        .nodes
        .iter()
        .flat_map(|node| node.generic_instances.clone())
        .collect::<Vec<_>>();
    instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
    instances.dedup_by(|a, b| a.instance_key == b.instance_key);
    instances
}

fn reverse_generic_call_edges(
    items: impl Iterator<Item = GenericItemFingerprint>,
) -> HashMap<String, HashSet<String>> {
    let all_items = items.collect::<Vec<_>>();
    let symbol_to_item = all_items
        .iter()
        .map(|item| (item.symbol.clone(), item.stable_item_id.clone()))
        .collect::<HashMap<_, _>>();

    let mut reverse = HashMap::<String, HashSet<String>>::new();
    for item in all_items {
        for callee in item.calls {
            let Some(target_item_id) = symbol_to_item.get(&callee) else {
                continue;
            };
            reverse
                .entry(target_item_id.clone())
                .or_default()
                .insert(item.stable_item_id.clone());
        }
    }
    reverse
}

fn dependency_invalidated_items(
    interface_changed: &HashSet<String>,
    reverse_edges: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut queue = interface_changed.iter().cloned().collect::<Vec<_>>();
    let mut seen = interface_changed.clone();
    let mut out = HashSet::new();

    while let Some(item_id) = queue.pop() {
        let Some(callers) = reverse_edges.get(&item_id) else {
            continue;
        };
        for caller in callers {
            if seen.insert(caller.clone()) {
                out.insert(caller.clone());
                queue.push(caller.clone());
            }
        }
    }

    out
}

pub(crate) fn generic_instance_hit_ratio(stats: &GenericInstancePlanStats) -> f64 {
    if stats.total_instances == 0 {
        1.0
    } else {
        stats.cache_hits as f64 / stats.total_instances as f64
    }
}

pub(crate) fn derive_generic_instance_plan(
    previous_cache: Option<&GenericInstanceCacheMetadata>,
    graph: &BuildGraphV2,
    opt_level: u8,
    feature_flags: &[String],
) -> (GenericInstancePlanStats, GenericInstanceCacheMetadata) {
    derive_generic_instance_plan_with_policy(
        previous_cache,
        graph,
        opt_level,
        feature_flags,
        retention_policy_from_env(),
    )
}

fn derive_generic_instance_plan_with_policy(
    previous_cache: Option<&GenericInstanceCacheMetadata>,
    graph: &BuildGraphV2,
    opt_level: u8,
    feature_flags: &[String],
    retention_policy: GenericCacheRetentionPolicy,
) -> (GenericInstancePlanStats, GenericInstanceCacheMetadata) {
    let target = target_triple();
    let current_instances = build_generic_instances(graph);
    let participating_item_ids = current_instances
        .iter()
        .map(|instance| instance.item_stable_id.clone())
        .collect::<HashSet<_>>();
    let current_items = build_generic_item_map(
        graph,
        if participating_item_ids.is_empty() {
            None
        } else {
            Some(&participating_item_ids)
        },
    );

    let mut previous_entries = HashMap::<String, GenericInstanceCacheEntry>::new();
    let mut previous_items = HashMap::<String, GenericItemFingerprint>::new();
    if let Some(previous) = previous_cache {
        if previous.schema_version == GENERIC_INSTANCE_CACHE_SCHEMA_VERSION
            && previous.compiler_version == env!("CARGO_PKG_VERSION")
            && previous.target_triple == target
            && previous.opt_level == opt_level
            && feature_flags_signature(&previous.feature_flags)
                == feature_flags_signature(feature_flags)
        {
            for entry in &previous.entries {
                previous_entries.insert(entry.instance_key.clone(), entry.clone());
            }
        }
        for entry in &previous.entries {
            if !participating_item_ids.is_empty()
                && !participating_item_ids.contains(&entry.item_stable_id)
            {
                continue;
            }
            previous_items
                .entry(entry.item_stable_id.clone())
                .or_insert_with(|| GenericItemFingerprint {
                    stable_item_id: entry.item_stable_id.clone(),
                    symbol: entry.item_stable_id.clone(),
                    module_id: entry.module_id.clone(),
                    kind: String::new(),
                    interface_hash: entry.interface_hash,
                    body_hash: entry.body_hash,
                    type_param_count: entry.canonical_type_args.len() as u32,
                    calls: Vec::new(),
                });
        }
    }

    let mut interface_changed = HashSet::<String>::new();
    let mut body_changed = HashSet::<String>::new();
    for (item_id, current) in &current_items {
        match previous_items.get(item_id) {
            Some(previous) => {
                if previous.interface_hash != current.interface_hash {
                    interface_changed.insert(item_id.clone());
                } else if previous.body_hash != current.body_hash {
                    body_changed.insert(item_id.clone());
                }
            }
            None => {
                interface_changed.insert(item_id.clone());
            }
        }
    }

    let reverse_edges = reverse_generic_call_edges(
        current_items
            .values()
            .cloned()
            .chain(previous_items.values().cloned()),
    );
    let dependency_changed = dependency_invalidated_items(&interface_changed, &reverse_edges);

    let mut stats = GenericInstancePlanStats {
        total_instances: current_instances.len() as u32,
        ..Default::default()
    };
    let mut next_entries = Vec::<GenericInstanceCacheEntry>::new();
    let now = now_unix_ms_u64();

    for instance in current_instances {
        let key = canonical_instance_key(&instance, opt_level, &target, feature_flags);
        let item_id = instance.item_stable_id.clone();
        let mut rebuilt = false;

        if interface_changed.contains(&item_id) {
            stats.interface_invalidated += 1;
            stats.rebuilt_instances += 1;
            rebuilt = true;
        } else if dependency_changed.contains(&item_id) {
            stats.dependency_invalidated += 1;
            stats.rebuilt_instances += 1;
            rebuilt = true;
        } else if body_changed.contains(&item_id) {
            stats.body_invalidated += 1;
            stats.rebuilt_instances += 1;
            rebuilt = true;
        } else if previous_entries.contains_key(&key) {
            stats.cache_hits += 1;
            stats.reuse_item_ids.push(item_id.clone());
            stats.reuse_instance_keys.push(key.clone());
        } else {
            stats.new_instances += 1;
            stats.rebuilt_instances += 1;
            rebuilt = true;
        }

        if rebuilt {
            stats.rebuild_item_ids.push(item_id.clone());
            stats.rebuild_instance_keys.push(key.clone());
        }

        next_entries.push(GenericInstanceCacheEntry {
            instance_key: key,
            item_stable_id: instance.item_stable_id,
            module_id: instance.module_id,
            canonical_type_args: instance.canonical_type_args,
            interface_hash: instance.interface_hash,
            body_hash: instance.body_hash,
            last_seen_unix_ms: now,
        });
    }
    stats.rebuild_item_ids.sort();
    stats.rebuild_item_ids.dedup();
    stats.rebuild_instance_keys.sort();
    stats.rebuild_instance_keys.dedup();
    stats.reuse_item_ids.sort();
    stats.reuse_item_ids.dedup();
    stats.reuse_instance_keys.sort();
    stats.reuse_instance_keys.dedup();

    let active_keys = next_entries
        .iter()
        .map(|entry| entry.instance_key.clone())
        .collect::<HashSet<_>>();
    let min_last_seen = now.saturating_sub(retention_policy.max_age_ms);
    let mut historical_entries = previous_entries
        .into_values()
        .filter(|entry| !active_keys.contains(&entry.instance_key))
        .filter(|entry| entry.last_seen_unix_ms >= min_last_seen)
        .collect::<Vec<_>>();
    historical_entries.sort_by(|a, b| b.last_seen_unix_ms.cmp(&a.last_seen_unix_ms));
    next_entries.extend(historical_entries);
    next_entries.sort_by(|a, b| {
        b.last_seen_unix_ms
            .cmp(&a.last_seen_unix_ms)
            .then_with(|| a.instance_key.cmp(&b.instance_key))
    });
    next_entries.dedup_by(|a, b| a.instance_key == b.instance_key);
    if next_entries.len() > retention_policy.max_entries {
        next_entries.truncate(retention_policy.max_entries);
    }

    let next_cache = GenericInstanceCacheMetadata {
        schema_version: GENERIC_INSTANCE_CACHE_SCHEMA_VERSION,
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        target_triple: target,
        opt_level,
        feature_flags: feature_flags.to_vec(),
        entries: next_entries,
    };

    (stats, next_cache)
}
