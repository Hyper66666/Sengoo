use super::JITCodegen;
use crate::mir::{self, MirFunction};

impl JITCodegen {
    /// 鐢熸垚鍑芥暟瀹氫箟
    pub(super) fn codegen_function(&mut self, mir_fn: &MirFunction) -> Result<(), String> {
        self.ir.push_str(&format!("; Function: {}\n", mir_fn.name));

        // main 鍑芥暟杩斿洖 i32锛屽叾浠栧嚱鏁颁娇鐢ㄥ疄闄呰繑鍥炵被鍨?
        let return_type = if mir_fn.name == "main" {
            "i32".to_string()
        } else {
            self.mir_type_to_llvm_str(&mir_fn.return_type)
        };

        // 鍑芥暟澶?
        self.ir
            .push_str(&format!("define {} @{}(", return_type, mir_fn.name));

        // 鍙傛暟鍒楄〃
        for (i, ty) in mir_fn.params.iter().enumerate() {
            if i > 0 {
                self.ir.push_str(", ");
            }
            self.ir
                .push_str(&format!("{} %l_{}", self.mir_type_to_llvm_str(ty), i + 1));
        }

        self.ir.push_str(") {\n");
        self.indent += 1;

        // 生成基本块（allocas 将在第一个基本块内生成）
        for (i, bb) in mir_fn.basic_blocks.iter().enumerate() {
            self.codegen_basic_block(mir_fn, bb, i == 0)?;
        }

        self.indent -= 1;
        self.ir.push_str("}\n\n");

        Ok(())
    }

    /// 鐢熸垚鍩烘湰鍧?
    fn codegen_basic_block(
        &mut self,
        mir_fn: &MirFunction,
        bb: &mir::BasicBlock,
        is_first: bool,
    ) -> Result<(), String> {
        // 鏇存柊褰撳墠鍩烘湰鍧?ID
        self.current_block_id = bb.id;

        // LLVM IR 鍩烘湰鍧楁爣绛句笉闇€瑕?% 鍓嶇紑锛屼絾寮曠敤鏃堕渶瑕?
        self.ir.push_str(&format!("bb_{}:\n", bb.id));
        self.indent += 1;

        // 如果是第一个基本块，在这里分配局部变量并存储参数
        if is_first {
            // 棣栧厛鍒嗛厤鎵€鏈夊眬閮ㄥ彉閲忥紙鍖呮嫭 Temp銆丳aram 鍜?Return 妲戒綅锛?
            for (local, ty) in &mir_fn.locals {
                self.emit_indent();
                self.ir.push_str(&format!(
                    "%local_{} = alloca {}\n",
                    local.id,
                    self.mir_type_to_llvm_str(ty)
                ));
            }

            // 存储参数到对应的 alloca
            for (i, ty) in mir_fn.params.iter().enumerate() {
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {} %l_{}, {}* %local_{}\n",
                    self.mir_type_to_llvm_str(ty),
                    i + 1,
                    self.mir_type_to_llvm_str(ty),
                    i + 1 // 鍙傛暟鐨?local id 浠?1 寮€濮?
                ));
            }
        }

        // 鐢熸垚鎸囦护
        for inst_id in &bb.instructions {
            let inst = mir_fn.instruction(*inst_id);
            self.codegen_instruction(inst, mir_fn)?;
        }

        // 生成终止符
        if let Some(terminator) = &bb.terminator {
            self.codegen_terminator(terminator, mir_fn)?;
        }

        self.indent -= 1;
        Ok(())
    }

    /// 发射 main 包装器
    pub(super) fn emit_main_wrapper(&mut self) {
        self.ir.push_str("; Main wrapper\n");
        self.ir.push_str("define i32 @main() {\n");
        self.indent += 1;
        self.emit_indent();
        self.ir.push_str("call void @_entry()\n");
        self.emit_indent();
        self.ir.push_str("ret i32 0\n");
        self.indent -= 1;
        self.ir.push_str("}\n\n");
    }
}
