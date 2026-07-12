use super::*;

impl Codegen {
    pub(super) fn emit_string_constants(&mut self) {
        if self.strings.is_empty() {
            return;
        }

        self.declarations.push_str("; String Constants\n");
        for (i, s) in self.strings.iter().enumerate() {
            let escaped = common::escape_llvm_c_string(s);
            self.declarations.push_str(&format!(
                "@.str.{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                i,
                s.len() + 1,
                escaped
            ));
        }
        self.declarations.push('\n');
    }

    fn add_string(&mut self, s: &str) -> String {
        let idx = self.string_counter;
        self.string_counter += 1;
        self.strings.push(s.to_string());
        format!("@.str.{}", idx)
    }

    pub(super) fn emit_struct_types(&mut self, mir_fns: &[MirFunction]) {
        let mut seen = HashSet::new();
        for func in mir_fns {
            for (_, ty) in &func.locals {
                self.collect_struct_types(ty, &mut seen);
            }
        }
    }

    fn collect_struct_types(&mut self, ty: &MIRType, seen: &mut HashSet<String>) {
        match ty {
            MIRType::Struct { name, fields } => {
                if seen.insert(name.clone()) {
                    for (_, ft) in fields {
                        self.collect_struct_types(ft, seen);
                    }

                    let field_types: Vec<String> = fields
                        .iter()
                        .map(|(_, ft)| self.mir_type_to_llvm_cached(ft))
                        .collect();
                    self.declarations.push_str(&format!(
                        "%{} = type {{ {} }}\n",
                        name,
                        field_types.join(", ")
                    ));
                }
            }
            // Recurse into composite types that may contain structs.
            MIRType::Array(elem, _) | MIRType::Ptr(elem) | MIRType::Ref(elem) => {
                self.collect_struct_types(elem, seen);
            }
            MIRType::Tuple(types) => {
                for ty in types {
                    self.collect_struct_types(ty, seen);
                }
            }
            _ => {}
        }
    }

    pub(super) fn collect_string_constants(&mut self, mir_fn: &MirFunction) {
        for bb in &mir_fn.basic_blocks {
            for inst in mir_fn.block_instructions(bb) {
                if let mir::Instruction::Assign {
                    value: MirConstant::String(s),
                    ..
                } = inst
                {
                    self.add_string(s);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_string_appends_literals() {
        let mut cg = Codegen::new();
        let first = cg.add_string("hello");
        let second = cg.add_string("hello");

        assert_eq!(first, "@.str.0");
        assert_eq!(second, "@.str.1");
        assert_eq!(cg.strings, vec!["hello".to_string(), "hello".to_string()]);
    }

    #[test]
    fn emit_string_constants_uses_llvm_byte_escapes() {
        let mut cg = Codegen::new();
        cg.add_string("quote=\" slash=\\ newline=\n");
        cg.emit_string_constants();

        assert!(cg
            .declarations
            .contains("quote=\\22 slash=\\5C newline=\\0A"));
        assert!(!cg.declarations.contains("\\\""));
    }
}
