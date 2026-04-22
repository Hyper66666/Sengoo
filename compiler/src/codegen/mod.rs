pub mod common;

pub mod jit;
mod declaration_helpers;
mod module_helpers;
mod instruction_helpers;
mod module_pipeline_helpers;
mod terminator_helpers;



pub use jit::JITCodegen;



use crate::mir::{self, Local, LocalKind, MIRType, MirConstant, MirFunction, MIR_I64};

use std::collections::{HashMap, HashSet};

use std::io::Write;



#[derive(Debug, Clone)]

pub struct ExternDecl {

    pub name: String,

    pub abi: String,

    pub link_name: Option<String>,

    pub params: Vec<MIRType>,

    pub ret: MIRType,

}



#[derive(Debug, Clone)]

pub struct ExportSymbol {

    pub internal_name: String,

    pub export_name: String,

}



#[derive(Debug, Clone, Default)]

pub struct FfiCodegenConfig {

    pub extern_decls: Vec<ExternDecl>,

    pub export_symbols: Vec<ExportSymbol>,

}



pub struct Codegen {

    ir: String,

    indent: usize,

    declarations: String,

    ffi: FfiCodegenConfig,

    /// Collected string literals emitted for the current module.
    strings: Vec<String>,

    /// 闁诲孩绋掗〃鍫ヮ敄娴ｅ湱鈻旈柟缁樺笧閸╂姊洪幓鎺旂妞ゆ挻鎮傚顐﹀级閹稿海褰滈梺?
    string_counter: usize,

    /// Cache local names for O(1) lookup during code generation.
    pub(crate) name_cache: Vec<String>,

    /// 缂傚倸鍊归幐鎼佹偤?MIR 缂備緡鍋夐褔鎮楅悜钘夌?LLVM 缂備緡鍋夐褔鎮楁搴樺亾濞戞瑯娈樻い鎴滅劍缁嬪鎳犻浣烘殸闂佸搫瀚慨鎾儍閻樿违?
    type_str_cache: HashMap<MIRType, String>,

    /// 婵?`load` 闂佸湱顭堝ú锝夊箮閵堝鍋ㄩ柣鏃傤焾閻忓洭鏌涢悜鍡楃仧缂佹梹鎸抽幆?SSA 婵炴垶鎸搁悺銊ヮ渻閸屾壕鍋撻棃娑橆仼闁宦板姂瀹曟娊濡搁妷锕€鈧娊鏌?
    load_counter: usize,

}



impl Codegen {

    pub fn new() -> Self {

        Self::with_ffi(FfiCodegenConfig::default())

    }



    pub fn with_ffi(ffi: FfiCodegenConfig) -> Self {

        let mut cg = Self {

            ir: String::new(),

            indent: 0,

            declarations: String::new(),

            ffi,

            strings: Vec::new(),

            string_counter: 0,

            name_cache: Vec::new(),

            type_str_cache: HashMap::new(),

            load_counter: 0,

        };

        cg.emit_header();

        cg.declare_runtime_functions();

        cg

    }












    pub fn codegen(&mut self, mir_fns: &[MirFunction]) -> Result<String, String> {

        self.emit_module_ir(mir_fns)?;

        let mut result = String::with_capacity(self.declarations.len() + self.ir.len());

        result.push_str(&self.declarations);

        result.push_str(&self.ir);

        Ok(result)

    }



    pub fn codegen_to_writer<W: Write>(

        &mut self,

        mir_fns: &[MirFunction],

        writer: &mut W,

    ) -> Result<(), String> {

        self.emit_module_ir(mir_fns)?;

        writer

            .write_all(self.declarations.as_bytes())

            .map_err(|e| format!("failed to write declarations: {}", e))?;

        writer

            .write_all(self.ir.as_bytes())

            .map_err(|e| format!("failed to write function IR: {}", e))?;

        writer

            .flush()

            .map_err(|e| format!("failed to flush LLVM IR output: {}", e))?;

        Ok(())

    }






    fn codegen_function(&mut self, mir_fn: &MirFunction) -> Result<(), String> {

        // Pre-compute all local names for O(1) lookup

        self.build_name_cache(mir_fn);

        // Reset load counter per function for clean SSA naming

        self.load_counter = 0;



        self.ir.push('\n');

        self.ir.push_str(&format!("; Function: {}\n", mir_fn.name));

        let return_type = self.mir_type_to_llvm_cached(&mir_fn.return_type);

        self.ir

            .push_str(&format!("define {} @{}(", return_type, mir_fn.name));

        for (i, ty) in mir_fn.params.iter().enumerate() {

            let param_name = format!("%l_{}", i + 1);

            if i > 0 {

                self.ir.push_str(", ");

            }

            let param_ty = self.mir_type_to_llvm_cached(ty);

            self.ir.push_str(&format!("{} {}", param_ty, param_name));

        }

        self.ir.push_str(") {\n");

        self.indent += 1;



        // Track which locals need alloca (user variables only)

        let user_locals: Vec<_> = mir_fn

            .locals

            .iter()

            .filter(|(l, _)| l.kind == LocalKind::User)

            .collect();



        for bb in &mir_fn.basic_blocks {

            let allocas: Vec<(Local, MIRType)> = if bb.id == 0 {

                user_locals.iter().map(|(l, t)| (*l, t.clone())).collect()

            } else {

                vec![]

            };

            self.codegen_basic_block(mir_fn, bb, &allocas)?;

        }

        self.indent -= 1;

        self.ir.push_str("}\n");

        Ok(())

    }



    fn codegen_basic_block(

        &mut self,

        mir_fn: &MirFunction,

        bb: &mir::BasicBlock,

        allocas: &[(Local, MIRType)],

    ) -> Result<(), String> {

        self.ir.push_str(&format!("bb_{}:\n", bb.id));

        self.indent += 1;



        // Emit allocas at the start of the entry block (before any instructions)

        for (local, ty) in allocas {

            let local_name = self.local_name(*local);

            let llvm_ty = self.mir_type_to_llvm_cached(ty);

            self.emit_indent();

            self.ir

                .push_str(&format!("{} = alloca {}\n", local_name, llvm_ty));

        }



        for inst_id in &bb.instructions {

            let inst = mir_fn.instruction(*inst_id);

            self.codegen_instruction(inst, mir_fn)?;

        }

        if let Some(terminator) = &bb.terminator {

            self.codegen_terminator(terminator, mir_fn)?;

        }

        self.indent -= 1;

        Ok(())

    }






    /// Build the local-name cache used during code generation.
    pub(crate) fn build_name_cache(&mut self, mir_fn: &MirFunction) {

        self.name_cache.clear();

        // Find the maximum local id to size the cache

        let max_id = mir_fn

            .locals

            .iter()

            .map(|(l, _)| l.index())

            .max()

            .unwrap_or(0);

        // Pre-fill with empty strings up to max_id + 1

        self.name_cache.resize(max_id + 1, String::new());

        for (local, _ty) in &mir_fn.locals {

            // Delegate to shared utility for name generation

            self.name_cache[local.index()] = common::local_name(*local);

        }

    }



    fn local_name(&self, local: Local) -> String {

        let idx = local.index();

        if idx < self.name_cache.len() && !self.name_cache[idx].is_empty() {

            self.name_cache[idx].clone()

        } else {

            common::local_name(local)

        }

    }



    fn emit_indent(&mut self) {

        common::emit_indent(&mut self.ir, self.indent);

    }



    /// 缂傚倸鍊归幐鎼佹偤?MIR 缂備緡鍋夐褔鎮楅悜钘夌?LLVM 缂備緡鍋夐褔鎮楁搴樺亾濞戞瑯娈樻い鎴滅劍缁嬪鎷犺缁€澶愭⒑椤掆偓閻忔繈宕㈤妶澶嬬厒鐎广儱鎷嬪Σ濠氭煕閹烘垶顥㈤柛妯稿€栫粙澶嬫償閵娿垹浜鹃柟鐑樺灩缁夋椽寮堕悜鍡楀鐎规洘鐓℃俊?
    fn mir_type_to_llvm_cached(&mut self, ty: &MIRType) -> String {

        if let Some(cached) = self.type_str_cache.get(ty) {

            return cached.clone();

        }

        // Delegate to shared utility for cache misses

        let result = common::mir_type_to_llvm_str(ty);

        self.type_str_cache.insert(ty.clone(), result.clone());

        result

    }



    fn get_local_type<'a>(&self, mir_fn: &'a MirFunction, local: Local) -> &'a MIRType {

        // 闂佸搫绉烽～澶婄暤?`local.id` 婵?locals 闁荤偞绋忛崝瀣嚈閹达箑鐭楅柡宥庡亯椤箓鏌涢妸銉劀缂佽鲸绻勭槐鎾绘惞鐟欏嫰妲ｉ梺鎼炲劤閸嬨倝鍩€椤戣棄浜鹃梺?`MIR_I64`闂?
        mir_fn

            .locals

            .get(local.index())

            .map(|(_, ty)| ty)

            .unwrap_or(&MIR_I64)

    }



    /// 闂佸吋鍎抽崲鑼躲亹閸ヮ剙绠肩€广儱瀚粙濠囨煛娴ｈ绶叉い鏇ㄥ枟閹棃寮崒娑氭殸 SSA 闂佺锕ㄦ竟鍫ュΥ閸愵亞鐭嗛柟顓熷坊閸?
    /// Resolve an operand to an LLVM value, loading from stack slots when needed.
    fn operand_value(&mut self, local: Local, mir_fn: &MirFunction) -> String {

        // Look up the local metadata before choosing the lowering path.
        let local_info = &mir_fn.locals[local.index()].0;



        match local_info.kind {

            LocalKind::User => {

                // 闂佹椿娼块崝宥夊春濞戙垹鐭楁慨妞诲亾闁革絾妞藉Λ渚€鍩€椤掑倹鍟哄ù锝囶焾鐢?load闂佹寧绋戦懟顖毭哄鍕枖?MIR 婵炴垶鎼╅崢濂告偩閻樺磭顩锋い鎺戝閸嬫挸顫濋鍌氱厬闁荤偞绋忛崝搴ㄥΦ濮橆厾鈻旈柛婵嗗閸曢箖鏌涜閸嬫捇鏌涙繝鍕靛劆闁?
                let ty = self.get_local_type(mir_fn, local);

                let llvm_ty = self.mir_type_to_llvm_cached(ty);

                let temp_reg = format!("%load.{}", self.load_counter);

                self.load_counter += 1;

                self.emit_indent();

                self.ir.push_str(&format!(

                    "{} = load {}, {}* {}\n",

                    temp_reg,

                    llvm_ty,

                    llvm_ty,

                    self.local_name(local)

                ));

                temp_reg

            }

            _ => {

                // 闂佸憡鐟ラ崐褰掑汲閻旂厧违濞达絽鍢查ˇ鏌ユ煛閸愵厽纭剧憸鏉款樀閺屽矁绠涢弴鐘虫儯闂佺儵鏅涢悺銊ф暜鐎涙ɑ濯撮悹鎭掑妽閺嗗繘鎮楅棃娑橆仼闁宦板姂瀹曟娊濡搁妷锕€鈧?

                self.local_name(local)

            }

        }

    }

}

impl Default for Codegen {
    fn default() -> Self {
        Self::new()
    }
}

