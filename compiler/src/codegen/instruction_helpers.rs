use super::*;

impl Codegen {
    pub(super) fn codegen_instruction(
        &mut self,

        inst: &mir::Instruction,

        mir_fn: &MirFunction,
    ) -> Result<(), String> {
        match inst {
            mir::Instruction::Nop => {}

            mir::Instruction::Assign { destination, value } => {
                let dest = self.local_name(*destination);

                self.emit_indent();

                match value {
                    mir::MirConstant::Int(n) => {
                        self.ir.push_str(&format!("{} = add i64 0, {}\n", dest, n));
                    }

                    mir::MirConstant::Uint(n) => {
                        self.ir.push_str(&format!("{} = add i64 0, {}\n", dest, n));
                    }

                    mir::MirConstant::Bool(b) => {
                        self.ir.push_str(&format!(
                            "{} = add i1 0, {}\n",
                            dest,
                            if *b { 1 } else { 0 }
                        ));
                    }

                    mir::MirConstant::Float(f) => {
                        self.ir
                            .push_str(&format!("{} = fadd double 0.0, {}\n", dest, f));
                    }

                    mir::MirConstant::Char(c) => {
                        self.ir
                            .push_str(&format!("{} = add i8 0, {}\n", dest, *c as i8));
                    }

                    mir::MirConstant::String(s) => {
                        // 闁诲繐绻愬Λ妤呮偤瑜忕划顓㈡晜閼愁垼娲柣銏╁灡閹倿宕冲ú顏勫強闁绘灏欏▓鎼佹煕閹烘挻绶查柛鎴斺偓鏂ユ灃闁靛鍎遍弬鈧梺姹囧妼鐎氼剟宕ｈ箛鏇氭勃闁逞屽墰閳ь剚绋掗〃鍫ヮ敄娴ｅ湱鈻旈柤纰卞墻閸庡﹪鏌涘▎鎰粵闁?
                        let str_idx = self.strings.iter().position(|x| x == s).unwrap_or(0);

                        let str_ref = format!("@.str.{}", str_idx);

                        self.ir.push_str(&format!(
                            "{} = bitcast [{} x i8]* {} to i8*\n",
                            dest,
                            s.len() + 1,
                            str_ref
                        ));
                    }

                    mir::MirConstant::Bytes(_) => {
                        self.ir.push_str(&format!("{} = add i64 0, 0\n", dest));
                    }

                    mir::MirConstant::GlobalRef(name) => {
                        let dest_ty = self.get_local_type(mir_fn, *destination);
                        let llvm_dest_ty = self.mir_type_to_llvm_cached(dest_ty);
                        if matches!(dest_ty, MIRType::Fn { .. }) {
                            self.ir.push_str(&format!(
                                "{} = bitcast {} @{} to {}
",
                                dest, llvm_dest_ty, name, llvm_dest_ty
                            ));
                        } else {
                            self.ir.push_str(&format!(
                                "{} = bitcast i64* @{} to i64
",
                                dest, name
                            ));
                        }
                    }

                    mir::MirConstant::Unit => {
                        self.ir.push_str(&format!("{} = add i8 0, 0\n", dest));
                    }
                }
            }

            mir::Instruction::Unary {
                destination,

                op,

                operand,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*operand, mir_fn);

                self.emit_indent();

                match op {
                    mir::MirUnOp::Neg => {
                        self.ir
                            .push_str(&format!("{} = sub i64 0, {}\n", dest, src_val));
                    }

                    mir::MirUnOp::Not => {
                        self.ir
                            .push_str(&format!("{} = xor i1 {}, true\n", dest, src_val));
                    }

                    mir::MirUnOp::BitNot => {
                        self.ir
                            .push_str(&format!("{} = xor i64 {}, -1\n", dest, src_val));
                    }
                }
            }

            mir::Instruction::Binary {
                destination,

                op,

                left,

                right,
            } => {
                let dest = self.local_name(*destination);

                // 婵犵鈧啿鈧綊鎮樻径鎰鐎广儱瀚粙濠囨煛娴ｅ搫顣兼俊鍙夋倐閹粙濡搁敃鈧悡鏇㈡煕濞嗘劧鑰块柛锝嗘そ閺佸秴鐣濋崟顑跨帛闁荤喐娲戠粈渚€宕?load

                let left_val = self.operand_value(*left, mir_fn);

                let right_val = self.operand_value(*right, mir_fn);

                self.emit_indent();

                match op {
                    mir::MirBinOp::Add => {
                        self.ir
                            .push_str(&format!("{} = add i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Sub => {
                        self.ir
                            .push_str(&format!("{} = sub i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Mul => {
                        self.ir
                            .push_str(&format!("{} = mul i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Div => {
                        self.ir.push_str(&format!(
                            "{} = sdiv i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Rem => {
                        self.ir.push_str(&format!(
                            "{} = srem i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Eq => {
                        self.ir.push_str(&format!(
                            "{} = icmp eq i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Ne => {
                        self.ir.push_str(&format!(
                            "{} = icmp ne i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Lt => {
                        self.ir.push_str(&format!(
                            "{} = icmp slt i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Le => {
                        self.ir.push_str(&format!(
                            "{} = icmp sle i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Gt => {
                        self.ir.push_str(&format!(
                            "{} = icmp sgt i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::Ge => {
                        self.ir.push_str(&format!(
                            "{} = icmp sge i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::BitAnd => {
                        self.ir
                            .push_str(&format!("{} = and i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::BitOr => {
                        self.ir
                            .push_str(&format!("{} = or i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::BitXor => {
                        self.ir
                            .push_str(&format!("{} = xor i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Shl => {
                        self.ir
                            .push_str(&format!("{} = shl i64 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::Shr => {
                        self.ir.push_str(&format!(
                            "{} = ashr i64 {}, {}\n",
                            dest, left_val, right_val
                        ));
                    }

                    mir::MirBinOp::LogAnd => {
                        self.ir
                            .push_str(&format!("{} = and i1 {}, {}\n", dest, left_val, right_val));
                    }

                    mir::MirBinOp::LogOr => {
                        self.ir
                            .push_str(&format!("{} = or i1 {}, {}\n", dest, left_val, right_val));
                    }
                }
            }

            mir::Instruction::Load {
                destination,

                source,
            } => {
                let dest = self.local_name(*destination);

                // 婵炶揪缍€濞夋洟寮?id 闂佺儵鏅涢悺銊ф暜鐎靛憡顫曢柕蹇曞Х缁屽潡鏌ㄥ☉姗嗗妺(1) 闂佸搫琚崕鍙夌珶?

                let (local_info, src_ty) = &mir_fn.locals[source.index()];

                self.emit_indent();

                // 闂佸搫绉烽～澶婄暤娴ｇ硶鏀?local 闂?kind 闂佸憡绮岄惉鑲╂偖椤愶箑鍨傞悗锝庝簻閻忔煡鏌￠崒銈呭绩缂佺粯鐗楃粙澶愬焵椤掑嫬绾ч柍鈺佸暞绗戦梺鍛婄啲缁犳挸銆掗崜浣瑰暫濞达絿鍎ら崺鍌涙叏濠垫挾鍒扮憸鏉垮€垮畷?load闂?                // 1. 闂佹椿娼块崝宥夊春濞戙垹鐭楁慨妞诲亾闁革絾妞介弻鍛潩椤掑倸鐓戦柣搴ｆ暩閹虫挾鑺?alloca闂佹寧绋戦惌渚€顢氭导鏉戠煑闁哄秲鍔嶉ˇ褔姊婚崶锝呬壕闁?load闂?
                // 2. 闂佸湱顭堝ú顓㈠箯閿熺姴绠ｉ柡宓懐鈹涢梺娲绘娇閸斿海鎮锕€鍨傞悗锝傛櫇閻﹀秹姊婚崶锝呬壕闁?load闂佹寧绋戦張顒佹櫠閻樼粯鍤勯柦妯侯槺缁讳線鏌涢幒鎾寸凡闁告瑩绠栭獮鎰板炊瑜庨崐濠氭煟閵娿儱顏╅柍褜鍓涢鏇㈠焵?
                let needs_load = local_info.kind == LocalKind::User
                    || matches!(src_ty, MIRType::Ptr(_) | MIRType::Ref(_));

                if needs_load {
                    let src = self.local_name(*source);

                    let llvm_ty = self.mir_type_to_llvm_cached(src_ty);

                    // load 闂佹眹鍔岀€氼喚鎮锕€鍨傞悗锝庝簻缁插潡鏌涢幇顒€甯犵紒顭戝墮閳瑰啴骞囬鐔稿劌婵炶揪绲剧划宥夊汲閻旇　鍋撻崷顓炰槐婵＄虎鍨堕獮鎰板炊瑜庨崐濠氭煟閵娿儱顏╅柛妯绘尵濡叉劙鎮╂担鍐炬蕉闂佹悶鍔岄鍐焵?
                    let load_ty = match src_ty {
                        MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                            self.mir_type_to_llvm_cached(inner)
                        }

                        _ => llvm_ty,
                    };

                    self.ir.push_str(&format!(
                        "{} = load {}, {}* {}\n",
                        dest, load_ty, load_ty, src
                    ));
                } else {
                    // Materialize a move by forwarding the SSA name directly.
                    let src = self.local_name(*source);

                    self.ir
                        .push_str(&format!("{} = add i64 0, {}\n", dest, src));
                }
            }

            mir::Instruction::Store { destination, value } => {
                if destination == value {
                    // Redundant self-writeback (`store x -> x`) does not change program state.

                    return Ok(());
                }

                let dest = self.local_name(*destination);

                let val = self.operand_value(*value, mir_fn);

                let ty = self.get_local_type(mir_fn, *value);

                let llvm_ty = self.mir_type_to_llvm_cached(ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "store {} {}, {}* {}\n",
                    llvm_ty, val, llvm_ty, dest
                ));
            }

            mir::Instruction::IndexAddr {
                destination,

                base,

                index,
            } => {
                let dest = self.local_name(*destination);

                let base_reg = self.local_name(*base);

                // 闂佸搫绉烽～澶婄暤?index local 闂?kind 闂佸憡鍔曢崯鍧楁偩妤ｅ啫鍙婃い鏍ㄧ閸庡﹪姊婚崶锝呬壕闁荤喐娲戠粈渚€宕?load 缂備椒绌堕崹鍦閳哄懎纾圭紒妤勩€€閸?
                let idx_local_info = &mir_fn.locals[index.index()].0;

                self.emit_indent();

                if idx_local_info.kind == LocalKind::User {
                    // 闂佹椿娼块崝宥夊春濞戞碍顫曢柕蹇曞Х缁屽潡鏌涘▎鎰惰€块柛锝嗘そ濡線鍩€椤掑倹鍟哄ù锝囶焾鐢?load闂?
                    let idx_reg = self.local_name(*index);

                    let idx_temp = format!("%idx.{}", destination.id);

                    self.ir
                        .push_str(&format!("{} = load i64, i64* {}\n", idx_temp, idx_reg));

                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = getelementptr i64, i64* {}, i64 {}\n",
                        dest, base_reg, idx_temp
                    ));
                } else {
                    // 婵炴垶鎸搁悺銊ヮ渻閸屾粍顫曢柕蹇曞Х缁屽潡鏌涙繝鍕付鐟滅増鐓￠幆鍕偓娑櫭径宥吤归敐鍡欑焼閻?getelementptr 闂佺顑呯换鎺嶇昂闂?
                    let idx_reg = self.local_name(*index);

                    self.ir.push_str(&format!(
                        "{} = getelementptr i64, i64* {}, i64 {}\n",
                        dest, base_reg, idx_reg
                    ));
                }
            }

            mir::Instruction::Aggregate {
                destination,

                fields,

                ty,
            } => {
                // 婵犮垼娉涚€氼噣骞冩繝鍥ф瀬闁规鍠氶惌?缂傚倷鐒﹂幐濠氭倵椤栨稒濯撮柟楣冣偓娑氶┏闂佸憡鑹鹃悧鍕焵?
                let dest = self.local_name(*destination);

                match ty {
                    MIRType::Array(elem_ty, _len) => {
                        // 闂佽桨鐒︽竟鍡欏垝瀹ュ鍤傛慨姗嗗墯閸娿倝鏌熺粙娆炬Ц闁告ɑ鎸惧Σ鎰版偐閻戔晛浜鹃柟閭︿邯閸ゅ鏌涢幇顓犳噧闁告瑥妫濋幆鍕敊閻ｅ苯鐏遍梺闈╅檮濠㈡ê顭囬崘顔芥櫖閻忕偟鍘ч埢蹇涙煟?store 闁诲海鎳撻張顒勫垂濮樿泛违?
                        let elem_llvm_ty = self.mir_type_to_llvm_cached(elem_ty);

                        for (i, field_local) in fields.iter().enumerate() {
                            // 闁荤姳绶ょ槐鏇㈡偩閺勫繈浜归柟鎯у暱椤ゅ懘鏌涜箛鎾虫殶缂佲偓瀹€鍕剭闁告洦鍋呴崟楣冩煕瑜夐崑鎾绘煏?
                            let elem_ptr = format!("{}.elem.{}", dest, i);

                            self.emit_indent();

                            self.ir.push_str(&format!(
                                "{} = getelementptr {}, {}* {}, i64 {}\n",
                                elem_ptr, elem_llvm_ty, elem_llvm_ty, dest, i
                            ));

                            // Evaluate each field value before storing it into the aggregate slot.
                            let field_val = self.operand_value(*field_local, mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!(
                                "store {} {}, {}* {}\n",
                                elem_llvm_ty, field_val, elem_llvm_ty, elem_ptr
                            ));
                        }
                    }

                    MIRType::Struct { .. } => {
                        // Build struct values incrementally with insertvalue.
                        let llvm_ty = self.mir_type_to_llvm_cached(ty);

                        if fields.is_empty() {
                            self.emit_indent();

                            self.ir
                                .push_str(&format!("{} = alloca {}\n", dest, llvm_ty));
                        } else {
                            let mut current = "undef".to_string();

                            for (i, field_local) in fields.iter().enumerate() {
                                let field_val = self.operand_value(*field_local, mir_fn);

                                let field_ty = self.get_local_type(mir_fn, *field_local);

                                let field_llvm = self.mir_type_to_llvm_cached(field_ty);

                                let temp = if i < fields.len() - 1 {
                                    format!("{}.f{}", dest, i)
                                } else {
                                    dest.clone()
                                };

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = insertvalue {} {}, {} {}, {}\n",
                                    temp, llvm_ty, current, field_llvm, field_val, i
                                ));

                                current = temp;
                            }
                        }
                    }

                    MIRType::Enum { .. } => {
                        let llvm_ty = self.mir_type_to_llvm_cached(ty);
                        let discr_local = fields.first().copied().ok_or_else(|| {
                            "enum aggregate missing discriminant field".to_string()
                        })?;
                        let discr_val = self.operand_value(discr_local, mir_fn);
                        let discr_temp = format!("{}.discr", dest);

                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = insertvalue {} undef, i64 {}, 0\n",
                            discr_temp, llvm_ty, discr_val
                        ));

                        let payload_val = if let Some(payload_local) = fields.get(1).copied() {
                            self.operand_value(payload_local, mir_fn)
                        } else {
                            "0".to_string()
                        };

                        self.emit_indent();
                        self.ir.push_str(&format!(
                            "{} = insertvalue {} {}, i64 {}, 1\n",
                            dest, llvm_ty, discr_temp, payload_val
                        ));
                    }

                    _ => {

                        // 闂佺绻戝﹢鍦垝椤掑倻灏甸悹鍥皺閳ь剛鍏樺鎶藉磼濞戞瑯妲柣鐘叉惈閻ゅ洨鎹?
                    }
                }
            }

            mir::Instruction::AddrOf {
                destination,

                source,
            } => {
                // Bitcast keeps the same bits while changing only the LLVM view of the value.
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                self.emit_indent();

                self.ir
                    .push_str(&format!("{} = bitcast i64* {} to i64\n", dest, src));
            }

            mir::Instruction::Call {
                destination,

                func,

                args,
            } => {
                // 闂佹眹鍨婚崰鎰板垂濮樿泛绀勯柤鎭掑劜濞堝爼鎮圭€ｎ亜鏆熼柡浣靛€濇俊?
                let dest = self.local_name(*destination);

                let dest_ty = self.get_local_type(mir_fn, *destination);

                let ret_ty = self.mir_type_to_llvm_cached(dest_ty);

                // 闁?`print` 闂佺顑嗗銊︾珶閹烘垟鏋斿┑鐘插亞濡查亶鏌ｉ悙鍙夛紨缂佽鲸绻勯埀顒€婀遍崑銈咁瀶椤栫偞鈷旂€广儱鎳庨悡?`puts`闂?
                let is_print = func == "print";

                let actual_func = if is_print { "puts" } else { func };

                let callee = if actual_func.starts_with('%') || actual_func.starts_with('@') {
                    actual_func.to_string()
                } else {
                    format!("@{}", actual_func)
                };

                // Resolve call operands through operand_value so loads happen consistently.
                let mut arg_strs: Vec<String> = Vec::new();

                for arg in args {
                    let arg_local = *arg;

                    let arg_ty = self.get_local_type(mir_fn, arg_local);

                    let llvm_arg_ty = self.mir_type_to_llvm_cached(arg_ty);

                    let val = self.operand_value(arg_local, mir_fn);

                    arg_strs.push(format!("{} {}", llvm_arg_ty, val));
                }

                self.emit_indent();

                if is_print {
                    // print lowers to puts and discards the C return code.
                    self.ir
                        .push_str(&format!("call i32 @puts({})\n", arg_strs.join(", ")));

                    // Model print as returning unit in Sengoo.
                    self.ir.push_str(&format!("{} = add i8 0, 0\n", dest));
                } else if ret_ty == "void" {
                    self.ir
                        .push_str(&format!("call void {}({})\n", callee, arg_strs.join(", ")));
                } else {
                    self.ir.push_str(&format!(
                        "{} = call {} {}({})\n",
                        dest,
                        ret_ty,
                        callee,
                        arg_strs.join(", ")
                    ));
                }
            }

            mir::Instruction::Discriminant {
                destination,

                source,
            } => {
                // Construct an enum as `{ discr, payload }`, leaving payload undef when absent.
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                // 婵炶揪缍€濞夋洟寮?extractvalue 闂佸憡鐟﹂悧鏇㈠吹椤撱垹绀嗛柕鍫濇噹閻掑ジ鏌涙繝鍕靛劆闁?                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = extractvalue {{ i64, i64 }} {}, 0\n",
                    dest, src
                ));
            }

            mir::Instruction::EnumConstruct {
                destination,

                discriminant,

                payload,

                enum_type: _,
            } => {
                // 闂佸搫顑呯€氫即鍩€椤掑倸校闁诲繐娲︾粙澶愬箚瑜夐崑鎾剁箔鐞涒€充壕?                // 閻熸粎澧楅幐鍛婃櫠?LLVM 婵炴垶鎼╅崢鎯р枔閹达箑鍑犳慨姗嗗亜椤╊剟鎮跺☉鏍у闁靛洦纰嶇粙?`{ 闂佸憡甯囬崐鏇㈠春閸℃稑纾? 闁哄鍋涢埀顒傚枎缁?}`闂?
                let dest = self.local_name(*destination);

                // Materialize the discriminant first.
                let discr_value = format!("{}.discr", dest);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0\n",
                    discr_value, discriminant
                ));

                // Fill payload slot 1 when the enum variant carries data.
                if let Some(payload_local) = payload {
                    let payload_val = self.operand_value(*payload_local, mir_fn);

                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = insertvalue {{ i64, i64 }} {}, i64 {}, 1\n",
                        dest, discr_value, payload_val
                    ));
                } else {
                    // Keep payload slot 1 as undef for payloadless variants.
                    self.emit_indent();

                    self.ir.push_str(&format!(
                        "{} = insertvalue {{ i64, i64 }} {}, i64 undef, 1\n",
                        dest, discr_value
                    ));
                }
            }

            mir::Instruction::ExtractPayload {
                destination,

                source,
            } => {
                // 闂佸湱绮崝鏇°亹閸ヮ剙鍑犳慨姗嗗亜椤╊剟寮堕悙娴嬪亾閻旈銈归梺?
                let dest = self.local_name(*destination);

                let src = self.local_name(*source);

                // 闁哄鍋涢埀顒傚枎缁佺懓霉閿濆懐小缂侇煈鍓涚槐鎺楀箻鐎电硶鍋撻鐐村剭闁告洦鍙庨崕?1 婵炴垶鎼╂禍婊堟偤瑜嶉埢鎾绘倷閸忓浜?                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = extractvalue {{ i64, i64 }} {}, 1\n",
                    dest, src
                ));
            }

            mir::Instruction::Cast {
                destination,

                value,

                to,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let dst_llvm = self.mir_type_to_llvm_cached(to);

                self.emit_indent();

                match (&src_ty, to) {
                    // Int -> Int闂佹寧绋掔喊宥嗘櫠鐠恒劉鍋撻崷顓熸珪闁?sext闂佹寧绋戦惉鑲╃磽婢跺瞼鐜婚柛鏇ㄥ幗閺?trunc闂?
                    (MIRType::Int(a), MIRType::Int(b)) if a < b => {
                        self.ir.push_str(&format!(
                            "{} = sext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Int(a), MIRType::Int(b)) if a > b => {
                        self.ir.push_str(&format!(
                            "{} = trunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Float -> Float闂佹寧绋掔喊宥嗘櫠鐠恒劉鍋撻崷顓熸珪闁?fpext闂佹寧绋戦惉鑲╃磽婢跺瞼鐜婚柛鏇ㄥ幗閺?fptrunc闂?
                    (MIRType::Float(a), MIRType::Float(b)) if a < b => {
                        self.ir.push_str(&format!(
                            "{} = fpext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    (MIRType::Float(a), MIRType::Float(b)) if a > b => {
                        self.ir.push_str(&format!(
                            "{} = fptrunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Int -> Float闂佹寧绋掗惌顔界箾閸ヮ剚鍋?sitofp闂佹寧绋戦悧濠傦耿娴ｈ櫣绠旈柨鏇楀亾鐟滄澘娼″顐も偓娑櫳戝▓鍫曞级閻戝棗澧悹鍥╁仱閹瑩鎯傞崫銉ь槴闂?
                    (MIRType::Int(_), MIRType::Float(_)) => {
                        self.ir.push_str(&format!(
                            "{} = sitofp {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Float -> Int闂佹寧绋掗惌顔界箾閸ヮ剚鍋?fptosi闂佹寧绋戦悧濠勬嫚閻愮儤鍊风痪顓炴噺缁侇噣鏌￠崼婵愭Ш妞ゆ垳绶氬畷锝夋煥鐎ｎ偅顔掗梺杞扮鎼存粎妲愬璺何?
                    (MIRType::Float(_), MIRType::Int(_)) => {
                        self.ir.push_str(&format!(
                            "{} = fptosi {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Bool -> Int闂佹寧绋掗惌顔界箾閸ヮ剚鍋?zext闂佹寧绋戝? 闂佸湱顣介弲娑㈠春?iN闂佹寧绋戦ˇ顓㈠焵?
                    (MIRType::Bool, MIRType::Int(_)) => {
                        self.ir.push_str(&format!(
                            "{} = zext {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Int -> Bool闂佹寧绋掗惌顔界箾閸ヮ剚鍋?trunc闂佹寧绋戝鐑?闂佽鎯屾禍婊堝春?i1闂佹寧绋戦ˇ顓㈠焵?
                    (MIRType::Int(_), MIRType::Bool) => {
                        self.ir.push_str(&format!(
                            "{} = trunc {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }

                    // Same type or unsupported: bitcast as fallback
                    _ => {
                        self.ir.push_str(&format!(
                            "{} = bitcast {} {} to {}\n",
                            dest, src_llvm, src_val, dst_llvm
                        ));
                    }
                }
            }

            mir::Instruction::Bitcast {
                destination,

                value,

                to,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                if !common::supports_mir_bitcast(src_ty, to) {
                    return Err(format!(
                        "invalid MIR bitcast from {} to {}",
                        self.mir_type_to_llvm_cached(src_ty),
                        self.mir_type_to_llvm_cached(to)
                    ));
                }

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let dst_llvm = self.mir_type_to_llvm_cached(to);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = bitcast {} {} to {}\n",
                    dest, src_llvm, src_val, dst_llvm
                ));
            }

            mir::Instruction::FieldAddr {
                destination,

                base,

                field,
            } => {
                let dest = self.local_name(*destination);

                let base_reg = self.local_name(*base);

                let base_ty = self.get_local_type(mir_fn, *base);

                let base_llvm = self.mir_type_to_llvm_cached(base_ty);

                self.emit_indent();

                // FieldAddr gets a pointer to a field within an aggregate type

                // Use getelementptr to compute the field address

                self.ir.push_str(&format!(
                    "{} = getelementptr inbounds {}, {}* {}, i32 0, i32 {}\n",
                    dest, base_llvm, base_llvm, base_reg, field
                ));
            }

            mir::Instruction::Extract {
                destination,

                value,

                index,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = extractvalue {} {}, {}\n",
                    dest, src_llvm, src_val, index
                ));
            }

            mir::Instruction::Insert {
                destination,

                value,

                field,

                new_value,
            } => {
                let dest = self.local_name(*destination);

                let src_val = self.operand_value(*value, mir_fn);

                let src_ty = self.get_local_type(mir_fn, *value);

                let src_llvm = self.mir_type_to_llvm_cached(src_ty);

                let new_val = self.operand_value(*new_value, mir_fn);

                let new_ty = self.get_local_type(mir_fn, *new_value);

                let new_llvm = self.mir_type_to_llvm_cached(new_ty);

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = insertvalue {} {}, {} {}, {}\n",
                    dest, src_llvm, src_val, new_llvm, new_val, field
                ));
            }

            mir::Instruction::Intrinsic {
                destination,

                intrinsic,

                args,
            } => {
                // Generate inline code for intrinsic operations

                match intrinsic {
                    mir::IntrinsicOp::AddWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = add i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::SubWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = sub i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::MulWithOverflow => {
                        if args.len() >= 2 {
                            let left_val = self.operand_value(args[0], mir_fn);

                            let right_val = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = mul i64 {}, {}\n",
                                    dest_name, left_val, right_val
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::Copy { size, .. } => {
                        if args.len() >= 2 {
                            let dest_ptr = self.operand_value(args[0], mir_fn);

                            let src_ptr = self.operand_value(args[1], mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!("call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)\n", dest_ptr, src_ptr, size));
                        }
                    }

                    mir::IntrinsicOp::Compare { size, .. } => {
                        if args.len() >= 2 {
                            let left_ptr = self.operand_value(args[0], mir_fn);

                            let right_ptr = self.operand_value(args[1], mir_fn);

                            if let Some(dest) = destination {
                                let dest_name = self.local_name(*dest);

                                self.emit_indent();

                                self.ir.push_str(&format!(
                                    "{} = call i32 @memcmp(i8* {}, i8* {}, i64 {})\n",
                                    dest_name, left_ptr, right_ptr, size
                                ));
                            }
                        }
                    }

                    mir::IntrinsicOp::MemMove { size, .. } => {
                        if args.len() >= 2 {
                            let dest_ptr = self.operand_value(args[0], mir_fn);

                            let src_ptr = self.operand_value(args[1], mir_fn);

                            self.emit_indent();

                            self.ir.push_str(&format!("call void @llvm.memmove.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)\n", dest_ptr, src_ptr, size));
                        }
                    }
                }
            }

            mir::Instruction::Phi {
                destination,

                incoming,
            } => {
                let dest = self.local_name(*destination);

                let ty = self.get_local_type(mir_fn, *destination);

                let is_void_like = match &ty {
                    MIRType::Unit | MIRType::Never => true,

                    MIRType::Tuple(fields) if fields.is_empty() => true,

                    _ => false,
                };

                if is_void_like {
                    // LLVM does not allow `phi void`.

                    return Ok(());
                }

                let llvm_ty = self.mir_type_to_llvm_cached(ty);

                let entries: Vec<String> = incoming
                    .iter()
                    .map(|(local, block_idx)| {
                        let val = self.local_name(*local);

                        format!("[ {}, %bb_{} ]", val, block_idx)
                    })
                    .collect();

                self.emit_indent();

                self.ir.push_str(&format!(
                    "{} = phi {} {}\n",
                    dest,
                    llvm_ty,
                    entries.join(", ")
                ));
            }
        }

        Ok(())
    }
}
