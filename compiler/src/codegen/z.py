
with open("mod.rs","r") as f:
    s=f.read()
old="            mir::Instruction::Nop => {"
a="            mir::Instruction::IndexAddr { destination, base, index } => {"
b="                let dest=self.local_name(*destination);" 
c="                let base_reg=self.local_name(*base);"
d="                let idx_reg=self.local_name(*index);"
e="                self.emit_indent();"
f="                self.ir.push_str(&format!(\"%.idx.{}=load i64,i64*{}**n\",destination.id,idx_reg));"
g="                self.emit_indent();"
h="                self.ir.push_str(&format!(\"{}=getelementptr i64,i64* {},i64 %.idx bed**n\",dest,base_reg,destination.id));"
i="            }"
t=s.replace(old,old+"//Ignore\ bound exam prep "+a+b+"\ "+c+"\ "+d+"\ "+e+"\ "+f+"\ "+g+"\ "+h+"\ "+i)
with open("mod一样.rs","w") as w: w.write(t)
print("OK")

