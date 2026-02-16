import sys

# Read the file
with open('compiler/src/codegen/mod.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# IndexAddr code to insert
code_to_insert = '''            mir::Instruction::IndexAddr { destination, base, index } => {
                let dest = self.local_name(*destination);
                let base_reg = self.local_name(*base);
                let idx_reg = self.local_name(*index);
                self.emit_indent();
                self.ir.push_str(&format!("%idx.{} = load i64, i64* {}\n", destination.id, idx_reg));
                self.emit_indent();
                self.ir.push_str(&format!("{} = getelementptr i64, i64* {}, i64 %idx.{}\n", dest, base_reg, destination.id));
            }'''

# Find the Nop case and insert after it
search_str = """            mir::Instruction::Nop => {
                // 忽略
            }"""

if search_str in content:
    content = content.replace(search_str, search_str + "\n" + code_to_insert)
    print("Successfully added IndexAddr handling")
else:
    print("Pattern not found")
    sys.exit(1)

# Write back the file
with open('compiler/src/codegen/mod.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("File updated successfully")
