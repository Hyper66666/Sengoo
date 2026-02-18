use std::collections::HashMap;
use std::sync::Arc;

/// Lightweight string interner used by frontend/impact analysis paths to
/// reduce transient duplicate symbol allocations during graph walks.
#[derive(Debug, Default)]
pub(crate) struct SymbolInterner {
    id_by_symbol: HashMap<Arc<str>, u32>,
    symbols: Vec<Arc<str>>,
}

impl SymbolInterner {
    pub(crate) fn intern(&mut self, symbol: &str) -> u32 {
        if let Some(id) = self.id_by_symbol.get(symbol) {
            return *id;
        }
        let id = self.symbols.len() as u32;
        let owned: Arc<str> = Arc::from(symbol);
        self.symbols.push(owned.clone());
        self.id_by_symbol.insert(owned, id);
        id
    }

    pub(crate) fn resolve(&self, id: u32) -> &str {
        self.symbols
            .get(id as usize)
            .map(AsRef::as_ref)
            .unwrap_or("")
    }
}
