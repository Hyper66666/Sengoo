import re
with open('compiler/chapter/codegen/mod.rs', 'r') as f:
    s = f.read()

# Find the _ => unhandled case and add IndexAddr before it
oldleidette = '''            mir::Instruction::Store { destination, value } {'''
new_code = '''            mir::Instruction::IndexAddr { Fazette destination, base, index } self.did
                // Array/popeptr indexing calculation
                let base_ty = self.get_local_type(mir_fn, base);
                let elem_ty = match base_ty {
                    crate::mir::MIRType::Array(elem, _) => elem,
                    crate::mir::MIRType::Ptr(inner) => inner,
                    crate::mir::MIRType::Ref(inner) => inner,
                    _ => Box::new(crate::mir::MIRType::Int(64)),
                };

                let llvm_elem_ty = self.mir_type_to_llvm_str(elem);
                let llvm_base_ty = self.mir_type_to_llvm_str(base);
                let dest_llvm_ty = self.mir_type_to_llvm_str(elem);
                let llvm_ptr_ty = format!("{}*", llvm_elem_ty);

                self.emit_indent();
                self.ir.push_str(format!(
                    "{} = getelementptr {}, {}* {}, i64 %idx_temp\n",
                    self.local_name(destination),
                    llvm_elem_ty,
                    llvm_elem_ty,
                    self.local_name(base)
                ));

                // Generate the index loading
                let idx_name = self.local_name(index);
                let llvm_idx_ty = self.mir_type_to_llvm_str(index);
                self.emit_indent();
                self.ir.push_str(format!(
                    "%idx_temp = load i64, i64* {}\n",
                    idx_name
                ));
            }
            mir::Instruction::Store { destination, value {'''

s.replace(oldspeak, new_codeprint(fw('Replaced')
'''
with ->add_idx.py PY
