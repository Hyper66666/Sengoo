use crate::{
    set_contract_runtime_checks_override, set_large_project_mode_override, ContractChecksMode,
};

const DEFAULT_UNREACHABLE_PRUNE_MIN_FUNCTIONS: usize = 20_000;

pub(super) struct LargeProjectModeOverrideGuard {
    previous: Option<bool>,
}

impl LargeProjectModeOverrideGuard {
    pub(super) fn new(previous: Option<bool>) -> Self {
        Self { previous }
    }
}

impl Drop for LargeProjectModeOverrideGuard {
    fn drop(&mut self) {
        set_large_project_mode_override(self.previous);
    }
}

pub(super) struct ContractChecksOverrideGuard {
    previous: Option<bool>,
}

impl ContractChecksOverrideGuard {
    pub(super) fn new(previous: Option<bool>) -> Self {
        Self { previous }
    }
}

impl Drop for ContractChecksOverrideGuard {
    fn drop(&mut self) {
        set_contract_runtime_checks_override(self.previous);
    }
}

pub(super) fn resolve_contract_checks_enabled(mode: ContractChecksMode, opt_level: u8) -> bool {
    match mode {
        ContractChecksMode::On => true,
        ContractChecksMode::Off => false,
        ContractChecksMode::Auto => opt_level <= 1,
    }
}

pub(super) fn contract_checks_mode_label(mode: ContractChecksMode) -> &'static str {
    match mode {
        ContractChecksMode::Auto => "auto",
        ContractChecksMode::On => "on",
        ContractChecksMode::Off => "off",
    }
}

pub(super) fn configured_hir_prune_min_functions() -> usize {
    match std::env::var("SENGOO_HIR_PRUNE_MIN_FUNCTIONS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
    {
        Some(0) => usize::MAX,
        Some(value) => value,
        None => DEFAULT_UNREACHABLE_PRUNE_MIN_FUNCTIONS,
    }
}

fn parse_large_project_mode_env(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

pub(super) fn large_project_mode_effectively_enabled(choice: Option<bool>) -> bool {
    if let Some(explicit) = choice {
        return explicit;
    }
    std::env::var("SENGOO_LARGE_PROJECT_MODE")
        .ok()
        .and_then(|raw| parse_large_project_mode_env(&raw))
        .unwrap_or(true)
}
