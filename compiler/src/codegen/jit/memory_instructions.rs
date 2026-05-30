use super::JITCodegen;
use crate::mir::{Local, MIRType, MirFunction};

impl JITCodegen {
    pub(super) fn codegen_store_instruction(
        &mut self,
        destination: Local,
        value: Local,
        mir_fn: &MirFunction,
    ) {
        // 瀛樺偍 local 鍒?local (let 璧嬪€?
        let dest_ty = self.get_local_type(mir_fn, destination);
        let src_ty = self.get_local_type(mir_fn, value);
        let llvm_dest_ty = self.mir_type_to_llvm_str(&dest_ty);
        let llvm_src_ty = self.mir_type_to_llvm_str(&src_ty);
        let src_reg = self.local_reg(value);
        let dest_reg = self.local_reg(destination);

        match &dest_ty {
            MIRType::Ptr(inner) => {
                // destination 是 Ptr(T)，表示我们想存储到指针指向的位置
                let inner_ty = self.mir_type_to_llvm_str(inner);

                // 1. 浠?destination alloca 鍔犺浇鎸囬拡鍊?
                self.emit_indent();
                let ptr_temp = format!("{}.destptr", dest_reg);
                let ptr_ptr_ty = format!("{inner_ty}**");
                let ptr_ty = format!("{}*", inner_ty);
                self.ir.push_str(&format!(
                    "{} = load {}, {} {}\n",
                    ptr_temp, ptr_ty, ptr_ptr_ty, dest_reg
                ));

                // 2. 鍔犺浇婧愬€?
                self.emit_indent();
                let val_temp = format!("{}.srcval", dest_reg);
                let src_ptr_ty = format!("{}*", llvm_src_ty);
                self.ir.push_str(&format!(
                    "{} = load {}, {} {}\n",
                    val_temp, llvm_src_ty, src_ptr_ty, src_reg
                ));

                // 3. 瀛樺偍鍒版寚閽堟寚鍚戠殑浣嶇疆
                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_src_ty, val_temp, inner_ty, ptr_temp
                ));
            }
            MIRType::Ref(inner) => {
                // Ref 类型的处理类似 Ptr
                let inner_ty = self.mir_type_to_llvm_str(inner);

                self.emit_indent();
                let ptr_temp = format!("{}.destptr", dest_reg);
                let ptr_ptr_ty = format!("{inner_ty}**");
                let ptr_ty = format!("{}*", inner_ty);
                self.ir.push_str(&format!(
                    "{} = load {}, {} {}\n",
                    ptr_temp, ptr_ty, ptr_ptr_ty, dest_reg
                ));

                self.emit_indent();
                let val_temp = format!("{}.srcval", dest_reg);
                let src_ptr_ty = format!("{}*", llvm_src_ty);
                self.ir.push_str(&format!(
                    "{} = load {}, {} {}\n",
                    val_temp, llvm_src_ty, src_ptr_ty, src_reg
                ));

                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_src_ty, val_temp, inner_ty, ptr_temp
                ));
            }
            MIRType::Tuple(_) => {
                // 结构体类型：使用逐字段复制
                if let MIRType::Tuple(field_tys) = &dest_ty {
                    for (i, field_ty) in field_tys.iter().enumerate() {
                        let llvm_field_ty = self.mir_type_to_llvm_str(field_ty);

                        // 鑾峰彇婧愬瓧娈靛湴鍧€
                        self.emit_indent();
                        let src_gep = format!("%.src_gep.{}.{}", destination.id, i);
                        self.ir.push_str(&format!(
                            "{} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                            src_gep, llvm_src_ty, llvm_src_ty, src_reg, i
                        ));

                        // 鍔犺浇婧愬瓧娈?
                        self.emit_indent();
                        let src_field = format!("%.src_field.{}.{}", destination.id, i);
                        self.ir.push_str(&format!(
                            "{} = load {}, {}* {}\n",
                            src_field, llvm_field_ty, llvm_field_ty, src_gep
                        ));

                        // 获取目标字段地址
                        self.emit_indent();
                        let dest_gep = format!("%.dest_gep.{}.{}", destination.id, i);
                        self.ir.push_str(&format!(
                            "{} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                            dest_gep, llvm_dest_ty, llvm_dest_ty, dest_reg, i
                        ));

                        // 瀛樺偍鍒扮洰鏍囧瓧娈?
                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "store {} {}, {}* {}\n",
                            llvm_field_ty, src_field, llvm_field_ty, dest_gep
                        ));
                    }
                }
            }
            MIRType::Array(elem_ty, len) => {
                // 鏁扮粍绫诲瀷锛氫娇鐢?memcpy
                let _llvm_elem_ty = self.mir_type_to_llvm_str(elem_ty);
                let size = self.get_type_size(elem_ty) * len;

                self.emit_indent();
                self.ir.push_str(&format!(
                    "call void @llvm.memcpy.p0i8.p0i8.i64(i8* bitcast {}* {} to i8*), i8* bitcast {}* {} to i8*, i64 {}, i32 8, i1 false)\n",
                    llvm_dest_ty, dest_reg, llvm_src_ty, src_reg, size
                ));
            }
            _ => {
                // 鏍囬噺绫诲瀷锛氱洿鎺ュ姞杞藉拰瀛樺偍
                self.emit_indent();
                let src_temp = format!("%.src.{}", destination.id);
                // 娉ㄦ剰锛歴rc_reg 鏄?alloca 杩斿洖鐨勬寚閽堬紝闇€瑕佸姞 *
                let src_ptr_ty = format!("{}*", llvm_src_ty);
                self.ir.push_str(&format!(
                    "{} = load {}, {} {}\n",
                    src_temp, llvm_src_ty, src_ptr_ty, src_reg
                ));

                self.emit_indent();
                // dest_reg 涔熸槸鎸囬拡绫诲瀷锛岄渶瑕佸姞 *
                let dest_ptr_ty = format!("{}*", llvm_dest_ty);
                self.ir.push_str(&format!(
                    "store {} {}, {} {}\n",
                    llvm_dest_ty, src_temp, dest_ptr_ty, dest_reg
                ));
            }
        }
    }

    pub(super) fn codegen_index_addr_instruction(
        &mut self,
        destination: Local,
        base: Local,
        index: Local,
        mir_fn: &MirFunction,
    ) {
        // 鏁扮粍绱㈠紩鍦板潃璁＄畻: ptr = &base[index]
        let base_ty = self.get_local_type(mir_fn, base);
        let elem_ty = match &base_ty {
            MIRType::Array(elem, _) => (*elem).clone(),
            MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
            _ => Box::new(MIRType::Int(64)),
        };

        let llvm_elem_ty = self.mir_type_to_llvm_str(&elem_ty);
        let llvm_base_ty = self.mir_type_to_llvm_str(&base_ty);
        let dest = self.local_name(destination);
        let base_reg = self.local_reg(base);
        let index_reg = self.local_reg(index);

        // 鍔犺浇绱㈠紩鍊?
        self.emit_indent();
        let index_temp = format!("{}.idx", dest);
        self.ir
            .push_str(&format!("{} = load i64, i64* {}\n", index_temp, index_reg));

        // 澶勭悊鍩哄潃
        match &base_ty {
            MIRType::Array(_, _) => {
                // 数组类型：首先将数组退化为指向首元素的指针（C 语言行为）
                // base_ptr = &base[0]
                self.emit_indent();
                let base_ptr = format!("{}.decay", dest);
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i64 0, i64 0\n",
                    base_ptr, llvm_base_ty, llvm_elem_ty, base_reg
                ));

                // 鐒跺悗璁＄畻鍏冪礌鍦板潃: ptr = &base_ptr[index]
                self.emit_indent();
                let addr_temp = format!("{}.addr", dest);
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i64 {}\n",
                    addr_temp, llvm_elem_ty, llvm_elem_ty, base_ptr, index_temp
                ));

                // 将地址存储到 destination local（destination 是指针类型）
                // destination 是 Ptr(T)，在 LLVM 中是 T**
                // addr_temp 鏄?T*锛岄渶瑕佸瓨鍌ㄥ埌 T**
                self.emit_indent();
                let dest_ptr_ptr_ty = format!("{llvm_elem_ty}**");
                let addr_ptr_ty = format!("{}*", llvm_elem_ty); // addr_temp 鐨勭被鍨嬫槸 T*
                self.ir.push_str(&format!(
                    "store {} {}, {} {}\n",
                    addr_ptr_ty, addr_temp, dest_ptr_ptr_ty, dest
                ));
            }
            MIRType::Ptr(_) | MIRType::Ref(_) => {
                // 鎸囬拡/寮曠敤绫诲瀷锛氱洿鎺ュ姞杞芥寚閽堬紝鐒跺悗璁＄畻鍏冪礌鍦板潃
                self.emit_indent();
                let loaded_ptr = format!("{}.ptr", dest);
                self.ir.push_str(&format!(
                    "{} = load {}, {}* {}\n",
                    loaded_ptr, llvm_base_ty, llvm_elem_ty, base_reg
                ));

                // 璁＄畻鍏冪礌鍦板潃
                self.emit_indent();
                let addr_temp = format!("{}.addr", dest);
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i64 {}\n",
                    addr_temp, llvm_elem_ty, llvm_elem_ty, loaded_ptr, index_temp
                ));

                // 将地址存储到 destination local（destination 是指针类型）
                // destination 是 Ptr(T)，在 LLVM 中是 T**
                // addr_temp 鏄?T*锛岄渶瑕佸瓨鍌ㄥ埌 T**
                self.emit_indent();
                let dest_ptr_ptr_ty = format!("{llvm_elem_ty}**");
                let addr_ptr_ty = format!("{}*", llvm_elem_ty); // addr_temp 鐨勭被鍨嬫槸 T*
                self.ir.push_str(&format!(
                    "store {} {}, {} {}\n",
                    addr_ptr_ty, addr_temp, dest_ptr_ptr_ty, dest
                ));
            }
            MIRType::Tuple(field_tys) => {
                // 元组/结构体类型：使用 getelementptr 访问字段
                // 字段索引是常量，不需要动态索引
                let struct_fields: Vec<String> = field_tys
                    .iter()
                    .map(|ty| self.mir_type_to_llvm_str(ty))
                    .collect();
                let llvm_struct_ty = format!("{{{}}}", struct_fields.join(", "));

                // 直接使用 getelementptr 获取字段地址
                // 娉ㄦ剰锛氱储寮曞€煎簲璇ユ槸缂栬瘧鏃跺父閲?
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                    dest, llvm_struct_ty, llvm_elem_ty, base_reg, index_temp
                ));
            }
            _ => {
                // 其他类型：错误处理
                self.emit_indent();
                self.ir.push_str(&format!(
                    "; IndexAddr: unsupported base type {:?}\n",
                    base_ty
                ));
            }
        }
    }
}
