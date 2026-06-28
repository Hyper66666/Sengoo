use super::JITCodegen;
use crate::mir::{Local, MIRType, MirFunction};

impl JITCodegen {
    pub(super) fn codegen_aggregate_instruction(
        &mut self,
        destination: Local,
        fields: &[Local],
        ty: &MIRType,
        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        // 鑱氬悎鍊煎垵濮嬪寲锛堟暟缁勫瓧闈㈤噺銆佺粨鏋勪綋瀛楅潰閲忥級
        match ty {
            MIRType::Array(elem_ty, len) => {
                // 数组初始化
                let llvm_elem_ty = self.mir_type_to_llvm_str(elem_ty);
                let llvm_array_ty = format!("[{} x {}]", len, llvm_elem_ty);
                let dest = self.local_name(destination);

                // 首先为每个元素加载值
                let mut elem_values = Vec::new();
                for (i, field_local) in fields.iter().enumerate() {
                    let field_ty = self.get_local_type(mir_fn, *field_local);
                    let llvm_field_ty = self.mir_type_to_llvm_str(&field_ty);
                    let reg = self.local_reg(*field_local);
                    let loaded = format!("%.elem.{}.{}", destination.id, i);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        loaded, llvm_field_ty, llvm_field_ty, reg
                    ));
                    elem_values.push((loaded, llvm_field_ty));
                }

                // 使用 alloca 初始化数组
                self.emit_indent();
                self.ir
                    .push_str(&format!("{} = alloca {}\n", dest, llvm_array_ty));

                // 瀛樺偍姣忎釜鍏冪礌
                for (i, (value, _)) in elem_values.iter().enumerate() {
                    self.emit_indent();
                    // 鑾峰彇鍏冪礌鎸囬拡
                    let gep = format!("%.ptr.{}.{}", destination.id, i);
                    self.ir.push_str(&format!(
                        "{} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                        gep, llvm_array_ty, llvm_elem_ty, dest, i
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {} {}, {}* {}\n",
                        llvm_elem_ty, value, llvm_elem_ty, gep
                    ));
                }
            }
            MIRType::Tuple(_field_tys) => {
                // 鍏冪粍/缁撴瀯浣撳垵濮嬪寲
                let dest = self.local_name(destination);

                // 首先为每个字段加载值
                let mut field_values = Vec::new();
                for (i, field_local) in fields.iter().enumerate() {
                    let field_ty = self.get_local_type(mir_fn, *field_local);
                    let llvm_field_ty = self.mir_type_to_llvm_str(&field_ty);
                    let reg = self.local_reg(*field_local);
                    let loaded = format!("%.elem.{}.{}", destination.id, i);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        loaded, llvm_field_ty, llvm_field_ty, reg
                    ));
                    field_values.push((loaded, llvm_field_ty));
                }

                // 涓哄厓缁勫垎閰嶇┖闂达紙浣跨敤 LLVM 缁撴瀯浣擄級
                let struct_fields: Vec<&str> =
                    field_values.iter().map(|(_, ty)| ty.as_str()).collect();
                let llvm_struct_ty = format!("{{{}}}", struct_fields.join(", "));
                self.emit_indent();
                self.ir
                    .push_str(&format!("{} = alloca {}\n", dest, llvm_struct_ty));

                // 存储每个字段
                for (i, (value, llvm_field_ty)) in field_values.iter().enumerate() {
                    self.emit_indent();
                    // 获取字段指针
                    let gep = format!("%.ptr.{}.{}", destination.id, i);
                    self.ir.push_str(&format!(
                        "{} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                        gep, llvm_struct_ty, llvm_field_ty, dest, i
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {} {}, {}* {}\n",
                        llvm_field_ty, value, llvm_field_ty, gep
                    ));
                }
            }
            MIRType::Enum { .. } => {
                let llvm_enum_ty = self.mir_type_to_llvm_str(ty);
                let dest = self.local_name(destination);

                let discr_reg = fields
                    .first()
                    .map(|local| self.local_reg(*local))
                    .ok_or_else(|| "enum aggregate missing discriminant field".to_string())?;
                let discr_loaded = format!("%.enum.discr.{}", destination.id);
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = load i64, i64* {}\n",
                    discr_loaded, discr_reg
                ));

                self.emit_indent();
                self.ir
                    .push_str(&format!("{} = alloca {}\n", dest, llvm_enum_ty));

                let discr_ptr = format!("%.ptr.{}.0", destination.id);
                self.emit_indent();
                self.ir.push_str(&format!(
                    "{} = getelementptr {}, {}* {}, i32 0, i32 0\n",
                    discr_ptr, llvm_enum_ty, llvm_enum_ty, dest
                ));
                self.emit_indent();
                self.ir
                    .push_str(&format!("store i64 {}, i64* {}\n", discr_loaded, discr_ptr));

                if let Some(payload_local) = fields.get(1) {
                    let payload_ty = self.get_local_type(mir_fn, *payload_local);
                    let llvm_payload_ty = self.mir_type_to_llvm_str(&payload_ty);
                    let payload_reg = self.local_reg(*payload_local);
                    let payload_loaded = format!("%.enum.payload.{}", destination.id);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        payload_loaded, llvm_payload_ty, llvm_payload_ty, payload_reg
                    ));

                    let payload_bytes = format!("%.ptr.{}.1", destination.id);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = getelementptr {}, {}* {}, i32 0, i32 1\n",
                        payload_bytes, llvm_enum_ty, llvm_enum_ty, dest
                    ));
                    let payload_ptr = format!("%.ptr.{}.1.typed", destination.id);
                    let payload_size = crate::codegen::common::enum_payload_storage_size(ty);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = bitcast [{} x i8]* {} to {}*\n",
                        payload_ptr, payload_size, payload_bytes, llvm_payload_ty
                    ));
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "store {} {}, {}* {}\n",
                        llvm_payload_ty, payload_loaded, llvm_payload_ty, payload_ptr
                    ));
                }
            }
            MIRType::Struct {
                fields: field_tys, ..
            } => {
                let llvm_struct_ty = self.mir_type_to_llvm_str(ty);
                let mut current = "undef".to_string();

                for (i, field_local) in fields.iter().enumerate() {
                    let field_ty = field_tys
                        .get(i)
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or_else(|| self.get_local_type(mir_fn, *field_local));
                    let llvm_field_ty = self.mir_type_to_llvm_str(&field_ty);
                    let field_value = format!("%.struct.field.{}.{}", destination.id, i);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        field_value,
                        llvm_field_ty,
                        llvm_field_ty,
                        self.local_reg(*field_local)
                    ));

                    let inserted = format!("%.struct.insert.{}.{}", destination.id, i);
                    self.emit_indent();
                    self.ir.push_str(&format!(
                        "{} = insertvalue {} {}, {} {}, {}\n",
                        inserted, llvm_struct_ty, current, llvm_field_ty, field_value, i
                    ));
                    current = inserted;
                }

                self.emit_indent();
                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_struct_ty,
                    current,
                    llvm_struct_ty,
                    self.local_reg(destination)
                ));
            }
            _ => {
                // 鍏朵粬鑱氬悎绫诲瀷锛堢粨鏋勪綋绛夛級
                let _dest = self.local_name(destination);
                self.emit_indent();
                self.ir.push_str(&format!("; aggregate: {:?}\n", ty));
            }
        }
        Ok(())
    }
}
