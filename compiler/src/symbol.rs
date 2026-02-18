use std::collections::HashMap;
use std::sync::Arc;

/// Compact symbol identifier used across AST/HIR/MIR to avoid repeated
/// string-key lookups and duplicate allocations in hot paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(u32);

impl SymbolId {
    pub const INVALID_RAW: u32 = u32::MAX;
    pub const INVALID: Self = Self(Self::INVALID_RAW);

    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID_RAW
    }
}

impl Default for SymbolId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Per-parse string interner used by the frontend pipeline.
#[derive(Debug, Default, Clone)]
pub struct SymbolInterner {
    id_by_symbol: HashMap<Arc<str>, SymbolId>,
    symbols: Vec<Arc<str>>,
}

impl SymbolInterner {
    pub fn intern(&mut self, symbol: &str) -> SymbolId {
        if let Some(id) = self.id_by_symbol.get(symbol) {
            return *id;
        }
        let id = SymbolId::new(self.symbols.len() as u32);
        let owned: Arc<str> = Arc::from(symbol);
        self.symbols.push(owned.clone());
        self.id_by_symbol.insert(owned, id);
        id
    }

    pub fn resolve(&self, id: SymbolId) -> Option<&str> {
        if !id.is_valid() {
            return None;
        }
        self.symbols.get(id.as_u32() as usize).map(AsRef::as_ref)
    }
}
