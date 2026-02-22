//! 缂傚倸鍊风欢锟犲磻婢舵劦鏁嬬憸鏃堝箖濡も偓閻ｏ繝骞忛弮鈧惄顖炲春閳ь剚銇勯幒鎴濐仼闁?//!
//! 闂?AST 闂備礁鎼ˇ顐﹀疾濠婂懐鐭欓柡宥庡幑閳ь兛绶氶獮瀣晜閻ｅ苯濮搁柣搴＄畭閸庨亶骞婃惔銊ラ敜濠电姴娲﹂悡鏇熺節婵犲倸鏆熼柣蹇涗憾閹泛顫濋悡搴濆枈閻庤娲栧﹢閬嶅焵椤掑﹦绉甸柛瀣笒椤繑銈ｉ崘鈺冨幐闁诲繒鍋犻褔宕濆鈧弻?
use crate::ast::pattern::Pattern;
use crate::ast::Visibility;
use crate::ast::*;
use crate::error::CompileError;
use crate::typeck::env::{Symbol, SymbolKind, TypeEnv};
use crate::typeck::ffi as ffi_check;
use crate::typeck::infer::TypeInfer;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplRegistry, TraitRegistry};
use crate::typeck::ty::{FloatKind, IntKind, Ty, TyKind, TyVarId, TypeckError};
use crate::Result;
use std::collections::{HashMap, HashSet};

type TyResult<T> = std::result::Result<T, TypeckError>;

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    parent: Option<String>,
    fields: Vec<(String, Type)>,
    methods: Vec<Function>,
}

#[derive(Debug, Clone)]
struct GenericTypeParamMeta {
    name: String,
    var_id: TyVarId,
    bounds: Vec<String>,
    default: Option<Ty>,
}

#[derive(Debug, Clone)]
struct GenericFunctionMeta {
    params: Vec<GenericTypeParamMeta>,
}

#[derive(Debug, Clone)]
struct GenericTypeMeta {
    params: Vec<GenericTypeParamMeta>,
}

/// 缂傚倸鍊风欢锟犲磻婢舵劦鏁嬬憸鏃堝箖濡も偓閻ｏ繝骞忛弮鈧惄顖炲春閳ь剚銇勯幒鎴濐仼闁藉啰鍠栭弻鏇熷緞閸繂濮曢梺?#[derive(Debug)]
pub struct TypeChecker {
    /// 缂傚倸鍊风欢锟犲磻婢舵劦鏁嬬憸鏃堝箖濡ゅ懏鍊婚柤鎭掑劚娴犮垹顪冮妶鍡欏闁告垵缍婂?
    env: TypeEnv,
    /// 缂傚倸鍊风欢锟犲磻婢舵劦鏁嬬憸鏃堝箖濡ゅ懏鍊婚柦妯侯槺椤斿﹪姊洪棃娑辩叚闂傚嫬瀚伴幃鐐寸鐎ｎ偆鍘?
    infer: TypeInfer,
    /// Trait 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕?
    trait_registry: TraitRegistry,
    /// Impl 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕?
    impl_registry: ImplRegistry,
    struct_field_defs: HashMap<String, Vec<(String, Type)>>,
    class_decls: HashMap<String, ClassDeclInfo>,
    generic_function_metas: HashMap<String, GenericFunctionMeta>,
    generic_type_metas: HashMap<String, GenericTypeMeta>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let env = TypeEnv::new();
        let infer = TypeInfer::with_env(env.clone());
        Self {
            env,
            infer,
            trait_registry: TraitRegistry::new(),
            impl_registry: ImplRegistry::new(),
            struct_field_defs: HashMap::new(),
            class_decls: HashMap::new(),
            generic_function_metas: HashMap::new(),
            generic_type_metas: HashMap::new(),
        }
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴滃綊鏌涢妷銏℃珖閻忓繒鏁婚幃褰掑炊椤忓嫮姣㈤梺閫炲苯澧伴柛蹇旓耿楠炲啴骞庣粵瀣櫖濠殿喗锚閸氬鈻?
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴滃綊鏌涢妷銏℃珖閻忓繒鏁婚幃褰掑炊椤忓嫮姣㈤梺閫炲苯澧伴柛蹇旓耿閻涱噣骞掑Δ鈧儫闂佹寧姊婚弲顐﹀礉閻戣姤鈷?
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴?Trait 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕?
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴?Impl 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕?
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴?Trait 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕戝鈧弻鏇熺箾瑜嶇€氼噣寮抽悩缁樷拺闁告稑锕﹂幊鎰版煕閵婏箑顎滈柕鍥ㄥ姈瀵板嫭绻濇惔鈩冾吙闂備礁鎼ú銊︽叏閻㈢姹?
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴?Impl 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕戝鈧弻鏇熺箾瑜嶇€氼噣寮抽悩缁樷拺闁告稑锕﹂幊鎰版煕閵婏箑顎滈柕鍥ㄥ姈瀵板嫭绻濇惔鈩冾吙闂備礁鎼ú銊︽叏閻㈢姹?
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潥闂備礁鎼Λ瀵稿緤閸撗呯煓濠㈣埖鍔﹂弫鍌炴煕閳ュ磭绠查柣鎾跺█閺?
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        self.generic_function_metas.clear();
        self.generic_type_metas.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            self.check_decl(decl)?;
        }

        Ok(())
    }

    pub fn check_program_with_filtered_function_bodies(
        &mut self,
        program: &Program,
        checked_function_names: &HashSet<String>,
    ) -> Result<()> {
        self.generic_function_metas.clear();
        self.generic_type_metas.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            self.check_decl_with_filtered_function_bodies(decl, checked_function_names)?;
        }

        Ok(())
    }

    /// 婵犵數濮伴崹鐟帮耿鏉堛劍娅犳俊銈傚亾閸楅亶鏌熺€电浠ч柣婵嗙埣閺岋絽螖閳ь剟鎮ф繝鍥风稏闁哄稁鍘介悡銉︾箾閹寸儐鐒藉褎鐩弻娑滎槻闁挎洦浜滈悾宄扳攽鐎ｎ偄浜归梺鍦帛鐢偤宕㈤幒鏃傜＝濞达綀顕栧▓锝囩磼閻樺啿鐏遍柕鍥ㄥ姉閹瑰嫰濡搁敃鈧禒鎺戭渻閵堝骸骞楅悽顖滃仧缁?
    fn declare_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                let name = fn_decl.name.name.clone();

                if fn_decl.abi.is_some() {
                    let mut param_types = Vec::new();
                    for param in &fn_decl.params {
                        let ty = self.check_type(&param.ty)?;
                        param_types.push(ty);
                    }
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };
                    self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

                    let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                    self.env.insert_fn(name.clone(), fn_ty, param_types, ret_ty);
                    self.set_generic_function_meta(name, Vec::new());
                    return Ok(());
                }

                // 闂傚倷娴囬妴鈧柛瀣崌閺屾盯顢曢敐鍡欘槰闂佽壈灏欐繛鈧柟顔筋殜瀹曠兘顢橀悙鐗堫潟婵犵绱曢搹搴ㄥ垂鐠鸿櫣鏆﹂柨婵嗩槸绾惧吋绻涢幋鐐垫噭妞ゆ柨绉剁槐鎾寸瑹閸パ傚嚱濡炪倖娉﹂崶銊モ偓鐢告煟閹达絾顥夋俊鐐垫櫕閳ь剙鍘滈崑鎾绘煕閹板吀绨芥い鏂款樀濮婃椽骞栭悙鎻掑Б闂佺顑囬崰鎾诲箖椤曗偓椤㈡洟鏁冮埀顒勫垂閸岀偞鍊甸柨婵嗘噹椤ｅ磭绱掗埀顒佸緞閹邦厾鍘搁梺閫炲苯澧存い銏＄☉閳藉螣閸忓す銉モ攽閻愯尙鎽犵紒顔肩Ф閺侇噣鏁撻悩鑼姦濡炪倖甯婄粈浣虹箔閹烘梻纾界€广儱鎷戦煬顒傗偓瑙勬礃閻熲晠骞婇悙鍝勎ㄩ柨鏃傜摂閸熲偓闂備浇宕垫慨鎾敄閸涙潙鐤ù鍏兼綑閺?
                let mut param_types = Vec::new();
                let mut fallback = false;
                let mut generic_meta = Vec::new();
                self.env.push_scope();
                match self.bind_type_params_with_meta(&fn_decl.type_params) {
                    Ok(meta) => {
                        generic_meta = meta;
                    }
                    Err(_) => {
                        fallback = true;
                    }
                }
                if !fallback {
                    for param in &fn_decl.params {
                        match self.check_type(&param.ty) {
                            Ok(ty) => param_types.push(ty),
                            Err(_) => {
                                fallback = true;
                                break;
                            }
                        }
                    }
                }

                if fallback {
                    self.env.pop_scope();
                    // 缂傚倸鍊风欢锟犲磻婢舵劦鏁嬬憸鏃堝箖濡や緡妲归幖娣灩閺嬪倿姊洪幐搴ｇ畵婵☆偅鐩崺鈧い鎺戝暙琚氶悗鍨緲鐎氼厼顭囪箛娑辨晝闁靛鍔栧ú鐔煎蓟閿熺姴绀冩い鎾跺枔閵嗘劕鈹戦悙鎻掔骇闁绘濞€瀵宕ㄩ弶鎴犲姦濡炪倖甯掔€氼剛绮堥崱娑欑厸濠㈣泛锕︽禒銏㈢磼閹邦収娈旈棁澶愭煥濠靛棙鍣介懖鏍ь渻?
                    let unit = self.env.unit_ty();
                    let ty = self.env.fn_ty(vec![], unit.clone());
                    self.env.insert_fn(name.clone(), ty, vec![], unit);
                    self.set_generic_function_meta(name, Vec::new());
                } else {
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret).unwrap_or_else(|_| self.env.unit_ty())
                    } else {
                        self.env.unit_ty()
                    };
                    self.env.pop_scope();

                    let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                    self.env.insert_fn(name.clone(), fn_ty, param_types, ret_ty);
                    self.set_generic_function_meta(name, generic_meta);
                }
            }
            DeclKind::ExternBlock(extern_block) => {
                ffi_check::validate_abi(&extern_block.abi).map_err(CompileError::from)?;
                for item in &extern_block.items {
                    match item {
                        ExternItem::Function(fn_decl) => {
                            let mut param_types = Vec::new();
                            for param in &fn_decl.params {
                                param_types.push(self.check_type(&param.ty)?);
                            }
                            let ret_ty = if let Some(ret) = &fn_decl.return_type {
                                self.check_type(ret)?
                            } else {
                                self.env.unit_ty()
                            };
                            ffi_check::validate_signature(
                                &extern_block.abi,
                                &param_types,
                                &ret_ty,
                                fn_decl.is_unsafe,
                            )
                            .map_err(CompileError::from)?;
                            let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                            self.env.insert_fn(
                                fn_decl.name.name.clone(),
                                fn_ty,
                                param_types,
                                ret_ty,
                            );
                        }
                        ExternItem::Static(static_decl) => {
                            let ty = self.check_type(&static_decl.ty)?;
                            self.env.insert_var(static_decl.name.name.clone(), ty);
                        }
                    }
                }
            }
            DeclKind::Struct(struct_decl) => {
                let name = struct_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&struct_decl.type_params);
                self.set_generic_type_meta(struct_decl.name.name.clone(), type_meta);
                let fields = struct_decl
                    .fields
                    .iter()
                    .map(|field| {
                        let field_name = field
                            .name
                            .as_ref()
                            .map(|ident| ident.name.clone())
                            .unwrap_or_default();
                        (field_name, field.ty.clone())
                    })
                    .collect::<Vec<_>>();
                self.struct_field_defs
                    .insert(struct_decl.name.name.clone(), fields);
            }
            DeclKind::Enum(enum_decl) => {
                let name = enum_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&enum_decl.type_params);
                self.set_generic_type_meta(enum_decl.name.name.clone(), type_meta);
            }
            DeclKind::Class(class_decl) => {
                let name = class_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&class_decl.type_params);
                self.set_generic_type_meta(class_decl.name.name.clone(), type_meta);
            }
            DeclKind::TypeAlias(type_alias) => {
                let name = type_alias.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&type_alias.type_params);
                self.set_generic_type_meta(type_alias.name.name.clone(), type_meta);
            }
            DeclKind::Const(const_decl) => {
                let name = const_decl.name.name.clone();
                let ty = self.env.error_ty();
                self.env.insert_var(name, ty);
            }
            DeclKind::Static(static_decl) => {
                let name = static_decl.name.name.clone();
                let ty = self.env.error_ty();
                self.env.insert_var(name, ty);
            }
            DeclKind::Trait(trait_decl) => {
                let name = trait_decl.name.name.clone();
                let symbol = Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Trait { name },
                };
                self.env.insert(trait_decl.name.name.clone(), symbol);
            }
            DeclKind::Impl(_impl_decl) => {}
            DeclKind::Import(_import_decl) => {}
            DeclKind::Module(module_decl) => {
                let name = module_decl.name.name.clone();
                let symbol = Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Module { name },
                };
                self.env.insert(module_decl.name.name.clone(), symbol);
            }
        }
        Ok(())
    }

    fn set_generic_function_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_function_metas.remove(&name);
        } else {
            self.generic_function_metas
                .insert(name, GenericFunctionMeta { params });
        }
    }

    fn set_generic_type_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_type_metas.remove(&name);
        } else {
            self.generic_type_metas
                .insert(name, GenericTypeMeta { params });
        }
    }

    fn collect_generic_type_meta(
        &mut self,
        type_params: &[TypeParam],
    ) -> Vec<GenericTypeParamMeta> {
        if type_params.is_empty() {
            return Vec::new();
        }
        self.env.push_scope();
        let result = self.bind_type_params_with_meta(type_params);
        self.env.pop_scope();
        result.unwrap_or_default()
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛闂備焦瀵х粙鎺楀礉濞嗗浚鍤?
    fn check_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                self.check_function_decl(fn_decl)?;
            }
            DeclKind::ExternBlock(extern_block) => {
                self.check_extern_block_decl(extern_block)?;
            }
            DeclKind::Struct(struct_decl) => {
                self.check_struct_decl(struct_decl)?;
            }
            DeclKind::Enum(enum_decl) => {
                self.check_enum_decl(enum_decl)?;
            }
            DeclKind::Class(class_decl) => {
                self.check_class_decl(class_decl)?;
            }
            DeclKind::TypeAlias(type_alias) => {
                self.check_type_alias(type_alias)?;
            }
            DeclKind::Const(const_decl) => {
                self.check_const_decl(const_decl)?;
            }
            DeclKind::Static(static_decl) => {
                self.check_static_decl(static_decl)?;
            }
            DeclKind::Trait(trait_decl) => {
                self.check_trait_decl(trait_decl)?;
            }
            DeclKind::Impl(impl_decl) => {
                self.check_impl_decl(impl_decl)?;
            }
            DeclKind::Import(_) | DeclKind::Module(_) => {}
        }
        Ok(())
    }

    fn check_decl_with_filtered_function_bodies(
        &mut self,
        decl: &Decl,
        checked_function_names: &HashSet<String>,
    ) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                if checked_function_names.contains(&fn_decl.name.name) {
                    self.check_function_decl(fn_decl)?;
                } else {
                    self.check_function_signature_decl(fn_decl)?;
                }
            }
            _ => {
                self.check_decl(decl)?;
            }
        }
        Ok(())
    }

    fn prepare_class_hierarchy(&mut self, program: &Program) -> Result<()> {
        self.class_decls.clear();
        self.collect_class_decls(program)?;
        self.validate_class_parent_targets()?;
        self.validate_class_cycles()?;

        let mut class_names: Vec<String> = self.class_decls.keys().cloned().collect();
        class_names.sort();

        let mut field_cache: HashMap<String, Vec<(String, Type)>> = HashMap::new();
        for class_name in &class_names {
            let mut stack = HashSet::new();
            let fields = self
                .resolve_class_fields_for(class_name, &mut field_cache, &mut stack)
                .map_err(CompileError::from)?;
            self.struct_field_defs.insert(class_name.clone(), fields);
        }

        let mut method_cache: HashMap<String, HashMap<String, Function>> = HashMap::new();
        for class_name in class_names {
            let mut stack = HashSet::new();
            let methods = self
                .resolve_class_methods_for(&class_name, &mut method_cache, &mut stack)
                .map_err(CompileError::from)?;

            let target_ty = self
                .env
                .lookup(&class_name)
                .and_then(|symbol| symbol.get_ty())
                .cloned()
                .unwrap_or_else(|| {
                    self.env.new_ty(TyKind::Adt {
                        name: class_name.clone(),
                        args: vec![],
                    })
                });

            let mut impl_info = crate::typeck::r#trait::ImplInfo::new(target_ty.clone(), None);
            let mut method_names: Vec<String> = methods.keys().cloned().collect();
            method_names.sort();

            for method_name in method_names {
                if let Some(method) = methods.get(&method_name) {
                    let fn_ty = self
                        .class_method_signature(method)
                        .map_err(CompileError::from)?;
                    impl_info.add_method(method_name, fn_ty);
                }
            }

            self.impl_registry
                .register_inherent(type_key(&target_ty), impl_info);
        }

        Ok(())
    }

    fn collect_class_decls(&mut self, program: &Program) -> Result<()> {
        for decl in &program.decls {
            let DeclKind::Class(class_decl) = &decl.kind else {
                continue;
            };

            let parent = class_decl.extends.as_ref().and_then(|path| {
                path.as_simple()
                    .map(|ident| ident.name.clone())
                    .or_else(|| path.segments.last().map(|ident| ident.name.clone()))
            });

            let mut fields = Vec::new();
            let mut methods = Vec::new();

            for (field_index, member) in class_decl.members.iter().enumerate() {
                match member {
                    ClassMember::Field(field) => {
                        let field_name = field
                            .name
                            .as_ref()
                            .map(|ident| ident.name.clone())
                            .unwrap_or_else(|| format!("_{}", field_index));
                        fields.push((field_name, field.ty.clone()));
                    }
                    ClassMember::Method(method) => {
                        methods.push(method.clone());
                    }
                }
            }

            self.class_decls.insert(
                class_decl.name.name.clone(),
                ClassDeclInfo {
                    parent,
                    fields,
                    methods,
                },
            );
        }

        Ok(())
    }

    fn validate_class_parent_targets(&self) -> Result<()> {
        for (class_name, class_info) in &self.class_decls {
            if let Some(parent) = &class_info.parent {
                if !self.class_decls.contains_key(parent) {
                    return Err(CompileError::TypeckError(TypeckError::Other(format!(
                        "class `{}` has unknown parent class `{}`",
                        class_name, parent
                    ))));
                }
            }
        }

        Ok(())
    }

    fn validate_class_cycles(&self) -> Result<()> {
        let mut state: HashMap<String, u8> = HashMap::new();
        let mut stack = Vec::new();
        let mut class_names: Vec<String> = self.class_decls.keys().cloned().collect();
        class_names.sort();

        for class_name in class_names {
            self.detect_class_cycle(&class_name, &mut state, &mut stack)
                .map_err(CompileError::from)?;
        }

        Ok(())
    }

    fn detect_class_cycle(
        &self,
        class_name: &str,
        state: &mut HashMap<String, u8>,
        stack: &mut Vec<String>,
    ) -> TyResult<()> {
        match state.get(class_name).copied() {
            Some(2) => return Ok(()),
            Some(1) => {
                let cycle_start = stack
                    .iter()
                    .position(|name| name == class_name)
                    .unwrap_or(0);
                let mut cycle: Vec<String> = stack[cycle_start..].to_vec();
                cycle.push(class_name.to_string());
                return Err(TypeckError::Other(format!(
                    "cyclic class inheritance detected: {}",
                    cycle.join(" -> ")
                )));
            }
            _ => {}
        }

        state.insert(class_name.to_string(), 1);
        stack.push(class_name.to_string());

        if let Some(parent) = self
            .class_decls
            .get(class_name)
            .and_then(|class_info| class_info.parent.as_ref())
        {
            self.detect_class_cycle(parent, state, stack)?;
        }

        stack.pop();
        state.insert(class_name.to_string(), 2);
        Ok(())
    }

    fn resolve_class_fields_for(
        &self,
        class_name: &str,
        cache: &mut HashMap<String, Vec<(String, Type)>>,
        stack: &mut HashSet<String>,
    ) -> TyResult<Vec<(String, Type)>> {
        if let Some(cached) = cache.get(class_name) {
            return Ok(cached.clone());
        }

        if !stack.insert(class_name.to_string()) {
            return Err(TypeckError::Other(format!(
                "cyclic class inheritance detected near `{}`",
                class_name
            )));
        }

        let class_info = self.class_decls.get(class_name).ok_or_else(|| {
            TypeckError::Other(format!(
                "internal error: class `{}` not collected",
                class_name
            ))
        })?;

        let mut merged = Vec::new();
        let mut seen = HashSet::new();

        if let Some(parent) = &class_info.parent {
            let parent_fields = self.resolve_class_fields_for(parent, cache, stack)?;
            for (field_name, field_ty) in parent_fields {
                seen.insert(field_name.clone());
                merged.push((field_name, field_ty));
            }
        }

        for (field_name, field_ty) in &class_info.fields {
            if !seen.insert(field_name.clone()) {
                return Err(TypeckError::Other(format!(
                    "duplicate inherited field `{}` in class `{}`",
                    field_name, class_name
                )));
            }
            merged.push((field_name.clone(), field_ty.clone()));
        }

        stack.remove(class_name);
        cache.insert(class_name.to_string(), merged.clone());
        Ok(merged)
    }

    fn resolve_class_methods_for(
        &self,
        class_name: &str,
        cache: &mut HashMap<String, HashMap<String, Function>>,
        stack: &mut HashSet<String>,
    ) -> TyResult<HashMap<String, Function>> {
        if let Some(cached) = cache.get(class_name) {
            return Ok(cached.clone());
        }

        if !stack.insert(class_name.to_string()) {
            return Err(TypeckError::Other(format!(
                "cyclic class inheritance detected near `{}`",
                class_name
            )));
        }

        let class_info = self.class_decls.get(class_name).ok_or_else(|| {
            TypeckError::Other(format!(
                "internal error: class `{}` not collected",
                class_name
            ))
        })?;

        let mut resolved = HashMap::new();
        if let Some(parent) = &class_info.parent {
            resolved = self.resolve_class_methods_for(parent, cache, stack)?;
        }

        let mut local_seen = HashSet::new();
        for method in &class_info.methods {
            let method_name = method.name.name.clone();
            if !local_seen.insert(method_name.clone()) {
                return Err(TypeckError::Other(format!(
                    "duplicate method `{}` in class `{}`",
                    method_name, class_name
                )));
            }
            resolved.insert(method_name, method.clone());
        }

        stack.remove(class_name);
        cache.insert(class_name.to_string(), resolved.clone());
        Ok(resolved)
    }

    fn class_method_signature(&mut self, method: &Function) -> TyResult<FunctionTy> {
        self.env.push_scope();
        if let Err(err) = self.bind_type_params_with_meta(&method.type_params) {
            self.env.pop_scope();
            return Err(TypeckError::Other(err.to_string()));
        }

        let mut param_types = Vec::new();
        for param in &method.params {
            param_types.push(self.check_type(&param.ty)?);
        }

        let ret_ty = if let Some(ret) = &method.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };

        let sig = FunctionTy::new(method.self_param.is_some(), param_types, ret_ty);
        self.env.pop_scope();
        Ok(sig)
    }

    fn is_result_placeholder(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ident(ident) => ident.name == "result",
            ExprKind::Path(path) => path
                .as_simple()
                .is_some_and(|segment| segment.name == "result"),
            _ => false,
        }
    }

    fn extract_result_literal_comparison(expr: &Expr) -> Option<(BinOp, Literal)> {
        let ExprKind::Binary { op, left, right } = &expr.kind else {
            return None;
        };

        if !matches!(op, BinOp::Eq | BinOp::NotEq) {
            return None;
        }

        if Self::is_result_placeholder(left) {
            if let ExprKind::Literal(lit) = &right.kind {
                return Some((*op, lit.clone()));
            }
        }

        if Self::is_result_placeholder(right) {
            if let ExprKind::Literal(lit) = &left.kind {
                return Some((*op, lit.clone()));
            }
        }

        None
    }

    fn extract_constant_return_literal(fn_decl: &Function) -> Option<Literal> {
        let stmt = fn_decl.body.stmts.last()?;
        match &stmt.kind {
            StmtKind::Expr(expr) => match &expr.kind {
                ExprKind::Literal(lit) => Some(lit.clone()),
                ExprKind::Return(Some(value)) => {
                    if let ExprKind::Literal(lit) = &value.kind {
                        Some(lit.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn validate_contracts_for_function(&mut self, fn_decl: &Function, ret_ty: &Ty) -> Result<()> {
        if let Some(precondition) = &fn_decl.precondition {
            let pre_ty = self.check_expr(precondition).map_err(CompileError::from)?;
            self.infer
                .unify(&pre_ty, &self.env.bool_ty())
                .map_err(CompileError::from)?;
        }

        if let Some(postcondition) = &fn_decl.postcondition {
            self.env.push_scope();
            self.env.insert_var("result".to_string(), ret_ty.clone());
            let post_ty = self.check_expr(postcondition);
            self.env.pop_scope();

            let post_ty = post_ty.map_err(CompileError::from)?;
            self.infer
                .unify(&post_ty, &self.env.bool_ty())
                .map_err(CompileError::from)?;

            if matches!(postcondition.kind, ExprKind::Literal(Literal::Bool(false))) {
                return Err(CompileError::from(TypeckError::Other(format!(
                    "postcondition for function `{}` is always false",
                    fn_decl.name.name
                ))));
            }

            if let (Some(return_lit), Some((op, ensured_lit))) = (
                Self::extract_constant_return_literal(fn_decl),
                Self::extract_result_literal_comparison(postcondition),
            ) {
                let contradiction = match op {
                    BinOp::Eq => return_lit != ensured_lit,
                    BinOp::NotEq => return_lit == ensured_lit,
                    _ => false,
                };
                if contradiction {
                    return Err(CompileError::from(TypeckError::Other(format!(
                        "postcondition contradicts constant return value in function `{}`",
                        fn_decl.name.name
                    ))));
                }
            }
        }

        Ok(())
    }

    fn check_function_signature_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();
        let signature = (|| -> Result<(Vec<Ty>, Ty, Vec<GenericTypeParamMeta>)> {
            let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

            let mut param_types = Vec::new();
            for param in &fn_decl.params {
                let ty = self.check_type(&param.ty).map_err(CompileError::from)?;
                self.env.insert_var(param.name.name.clone(), ty.clone());
                param_types.push(ty);
            }

            let ret_ty = if let Some(ret) = &fn_decl.return_type {
                self.check_type(ret).map_err(CompileError::from)?
            } else {
                self.env.unit_ty()
            };

            self.validate_contracts_for_function(fn_decl, &ret_ty)?;
            self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

            Ok((param_types, ret_ty, generic_meta))
        })();
        self.env.pop_scope();

        let (param_types, ret_ty, generic_meta) = signature?;
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛闂備焦鎮堕崐婵囩鐠轰警鍤曟い鎰剁畱缁狙囨煕閺嶇數纾块柣顓燁殜濮?
    fn check_function_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();
        let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

        let mut param_types = Vec::new();
        for param in &fn_decl.params {
            let ty = self.check_type(&param.ty)?;
            self.env.insert_var(param.name.name.clone(), ty.clone());
            param_types.push(ty);
        }

        let ret_ty = if let Some(ret) = &fn_decl.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };
        self.validate_contracts_for_function(fn_decl, &ret_ty)?;
        self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

        // 婵犵妲呴崑鍛熆濡皷鍋撳鐓庡籍鐎殿噮鍋婇幃娆撳传閸曨収妲伴梺璇茬箳閸嬬姴螞閸曨倣鎺楀箛閻楀牏鍘告繛杈剧悼閹虫捇藟鐎ｎ偁浜滈柟鐑樻煥閸樺鈧娲忛崕閬嶎敇閼规壆鐤€闁哄洨濯Σ鐑芥⒒娴ｅ鈧偓闁稿鎸婚妵鍕冀閵娧€妲堥梺浼欏瘜閸ｏ絽顫忓ú顏嶆晝闁挎繂鎳愰悷銊х磽閸屾氨孝闁挎洩绠撻獮蹇涱敃閿曗偓缁€鍐┿亜韫囨挻鍣烘繛?
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env.insert_fn(
            fn_decl.name.name.clone(),
            fn_ty,
            param_types.clone(),
            ret_ty.clone(),
        );

        // Function.body is always present (Block)
        let body_ty = self.check_block(&fn_decl.body)?;

        // 闂傚倷鑳剁划顖炪€冩径鎰剁稏濠㈣埖鍔栭崑鈺呮煃閸濆嫬鈧摜娆㈤悙鐑樼厱闁哄洢鍔岄獮妤呮煕婵犲嫬浠遍柡灞诲妼閳藉鈻庨幒鎴婵＄偑鍊栧ú锕傚窗濡ゅ啰鐭?main 闂傚倷绀侀幉锟犲垂閸忓吋鍙忛柕鍫濐槸濮规煡鏌ｉ弬鍨倯闁哄拋鍓熼幃姗€鎮欓悽鍨啒濠电偛鐪伴崐婵嬪蓟閿涘嫧鍋撻敐搴′簽闁活厼娴风槐鎺旂磼濡櫣浼岄悗瑙勬礀閻栧ジ骞冨▎鎰闁告劗鍋撻拺澶愭⒒娴ｈ鍋犻柛鏂跨焸椤㈡牠宕卞顫秮楠炴牗鎷呴崨濠勨偓顒勬煟鎼淬垻鈯曢拑杈ㄧ箾閸繂顣崇紒杈ㄥ浮椤㈡洟濡烽鍏碱唲闂備浇顕ч柊锝夊绩鏉堚晝鐭欏鑸靛姇濡﹢鏌涢…鎴濇灍闁伙讣缍佸鐑樻姜閹殿喚鐛㈠銈忕秶婵″洨妲愰悙纰樺亾閿濆骸浜濆ù婧垮€濋弻锟犲磼濞戞﹩鍤嬬紓浣插亾闁?()闂傚倷鐒︾€笛呯矙閹达附鍤愭い鏍仜閸ㄥ倹銇勯弽顐粶缂佲偓閸屾褰掓晲閸モ晜鎲橀梺?        // 闂備礁鎼ˇ顐﹀疾濠婂牊鍋￠柨鏇炲€归崑?main 闂傚倷绀侀幉锟犲垂閸忓吋鍙忛柕鍫濐槸濮规煡鏌ｉ弮鍌氬付闁活厽顨嗛妵鍕冀閵娧勫櫏缂備降鍔嬮崡鎶藉蓟閿濆鏁傞柛鎰靛幖閸橈繝姊洪崫鍕棡闁告梹鐟ラ锝夋偨缁嬭法鍔﹀銈嗗笒鐎氼剛鎲撮敃鍌氱閺夊牆澧界粙濠氭煟?return 0
        let is_main_with_implicit_return = fn_decl.name.name == "main"
            && matches!(body_ty.kind, TyKind::Unit)
            && matches!(ret_ty.kind, TyKind::Int(_));

        if !is_main_with_implicit_return {
            self.infer
                .unify(&body_ty, &ret_ty)
                .map_err(|e| CompileError::from(e))?;
        }

        self.env.pop_scope();

        // 闂傚倸鍊烽悞锕併亹閸愵亞鐭撻柣銏㈩焾閽冪喎鈹戦悩鎻掆偓鐢稿几閺嶎厽鐓忓┑鐐茬仢閸旀岸鏌￠崒妤€浜鹃梻鍌欑劍鐎笛呯矙閹达附鍋嬮柟鎷屽焽閳ь剙鎳橀、鏇㈡晜閽樺澹嗛梻浣告惈缁嬩線宕ｆ惔銊ユ辈闁哄洨鍠撶粻鎯ь熆鐠鸿櫣澧曞┑鈥炽偢閺屾盯鎮欓幍顔剧厯闂佽桨绀佺粔闈涱嚗閸曨偀妲堟慨姗€纭稿Σ椋庣磽娴ｉ缚妾搁柛娆忛叄瀹曚即寮撮悩鍐插簥濠殿喗顭堝▔娑氣偓姘樀閺屽秷顧侀柛鎾寸〒濡叉劙骞掑Δ鈧柋鍥煏韫囧﹥娅呭┑顔诲嵆濮婃椽宕ㄦ繝鍛棟缂備礁顦遍弫濠氥€佸Δ鈧…銊╁川椤旂厧骞?
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);

        Ok(())
    }

    fn validate_ffi_function_decl(
        &mut self,
        fn_decl: &Function,
        param_types: &[Ty],
        ret_ty: &Ty,
    ) -> Result<()> {
        if fn_decl.abi.is_none() {
            if fn_decl.no_mangle || fn_decl.export_name.is_some() {
                return Err(CompileError::from(TypeckError::Other(
                    "no_mangle/export_name require `extern \"...\" fn`".to_string(),
                )));
            }
            return Ok(());
        }

        if !fn_decl.type_params.is_empty() {
            return Err(CompileError::from(TypeckError::Other(
                "generic extern functions are not supported in FFI MVP".to_string(),
            )));
        }

        let abi = fn_decl.abi.as_deref().unwrap_or("C");
        ffi_check::validate_signature(abi, param_types, ret_ty, fn_decl.is_unsafe)
            .map_err(CompileError::from)?;

        if fn_decl.export_name.is_some() && !matches!(fn_decl.vis, Visibility::Public) {
            return Err(CompileError::from(TypeckError::Other(
                "export_name requires `pub extern` function".to_string(),
            )));
        }

        Ok(())
    }

    fn check_extern_block_decl(&mut self, extern_block: &ExternBlock) -> Result<()> {
        ffi_check::validate_abi(&extern_block.abi).map_err(CompileError::from)?;
        for item in &extern_block.items {
            match item {
                ExternItem::Function(fn_decl) => {
                    let mut param_types = Vec::new();
                    for param in &fn_decl.params {
                        param_types.push(self.check_type(&param.ty)?);
                    }
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };
                    ffi_check::validate_signature(
                        &extern_block.abi,
                        &param_types,
                        &ret_ty,
                        fn_decl.is_unsafe,
                    )
                    .map_err(CompileError::from)?;
                }
                ExternItem::Static(static_decl) => {
                    self.check_type(&static_decl.ty)?;
                }
            }
        }

        Ok(())
    }

    fn bind_type_params_with_meta(
        &mut self,
        type_params: &[TypeParam],
    ) -> Result<Vec<GenericTypeParamMeta>> {
        let mut metas = Vec::with_capacity(type_params.len());
        for type_param in type_params {
            let fresh_var = self.env.new_ty_var();
            let var_id = match fresh_var.kind {
                TyKind::Var(id) => id,
                _ => {
                    return Err(CompileError::from(TypeckError::Other(
                        "internal error: expected fresh type variable".to_string(),
                    )))
                }
            };
            self.env
                .insert_type(type_param.name.name.clone(), fresh_var);
            metas.push(GenericTypeParamMeta {
                name: type_param.name.name.clone(),
                var_id,
                bounds: Vec::new(),
                default: None,
            });
        }

        // Resolve defaults and trait bound paths inside the same generic scope.
        for (type_param, meta) in type_params.iter().zip(metas.iter_mut()) {
            for bound in &type_param.bounds {
                let trait_name = bound
                    .path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .ok_or_else(|| {
                        CompileError::from(TypeckError::Other(
                            "unsupported trait bound path in type parameter".to_string(),
                        ))
                    })?;
                if !matches!(
                    self.env.lookup(&trait_name).map(|symbol| &symbol.kind),
                    Some(SymbolKind::Trait { .. })
                ) {
                    return Err(CompileError::from(TypeckError::UndefinedType {
                        name: trait_name,
                    }));
                }
                meta.bounds.push(trait_name);
            }

            if let Some(default_ty) = &type_param.default {
                meta.default = Some(self.check_type(default_ty).map_err(CompileError::from)?);
            }
        }

        Ok(metas)
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶戠紓鍌欑贰閸犳捇宕濋幋婵愬殨闁归棿绀佺粈瀣亜韫囨挻顥犲鍥р攽閻橆喖鐏╂繝鈧潏銊︽珷婵°倐鍋撻崡?
    fn check_struct_decl(&mut self, struct_decl: &Struct) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&struct_decl.type_params)?;

        for field in &struct_decl.fields {
            self.check_type(&field.ty)?;
        }

        self.env.pop_scope();
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潥闂備浇顕х换鎰洪敂鍓х煓濠㈣埖鍔曠粻姘辨喐濠靛牊顫曢柨婵嗩槹閻?
    fn check_enum_decl(&mut self, enum_decl: &Enum) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&enum_decl.type_params)?;
        for variant in &enum_decl.variants {
            for field in &variant.fields {
                match field {
                    VariantField::Named(_, ty) => {
                        self.check_type(ty)?;
                    }
                    VariantField::Unnamed(ty) => {
                        self.check_type(ty)?;
                    }
                }
            }
        }
        self.env.pop_scope();
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶戦梺鑽ゅ仦閸戝綊宕戦崨顖滃崥闁绘柨鍚嬮崑瀣煕椤愩倕娅忔繛?
    fn check_class_decl(&mut self, class_decl: &Class) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&class_decl.type_params)?;

        for member in &class_decl.members {
            match member {
                ClassMember::Field(field) => {
                    self.check_type(&field.ty)?;
                }
                ClassMember::Method(method) => {
                    self.check_class_method_decl(&class_decl.name.name, method)?;
                }
            }
        }

        self.env.pop_scope();
        Ok(())
    }

    fn check_class_method_decl(&mut self, class_name: &str, method: &Function) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&method.type_params)?;

        if method.self_param.is_some() {
            let self_ty = self
                .env
                .lookup(class_name)
                .and_then(|symbol| symbol.get_ty())
                .cloned()
                .unwrap_or_else(|| {
                    self.env.new_ty(TyKind::Adt {
                        name: class_name.to_string(),
                        args: vec![],
                    })
                });
            self.env.insert_var("self".to_string(), self_ty);
        }

        for param in &method.params {
            let ty = self.check_type(&param.ty)?;
            self.env.insert_var(param.name.name.clone(), ty);
        }

        let ret_ty = if let Some(ret) = &method.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };

        let body_ty = self.check_block(&method.body)?;
        self.infer
            .unify(&body_ty, &ret_ty)
            .map_err(CompileError::from)?;

        self.env.pop_scope();
        Ok(())
    }

    fn check_type_alias(&mut self, type_alias: &TypeAlias) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&type_alias.type_params)?;
        self.check_type(&type_alias.ty)?;
        self.env.pop_scope();
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛闂備胶绮悧鏇㈡偉婵傜钃熺€光偓閸愵亞鏉搁梺鐟扮仢鐎氼噣鎯屽Δ鍛拺?
    fn check_const_decl(&mut self, const_decl: &Const) -> Result<()> {
        let ty = self.check_type(&const_decl.ty)?;
        let value_ty = self.check_expr(&const_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶曟繝鐢靛仧椤戞洟宕愬┑瀣祦闁逞屽墮闇夐柨婵嗘祩閻掑墽绱撳鍕獢婵﹥妞介獮鎾诲箳閺冨偆鍞堕梻浣瑰缁嬫帡宕濆▎蹇ｅ殨?
    fn check_static_decl(&mut self, static_decl: &Static) -> Result<()> {
        let ty = self.check_type(&static_decl.ty)?;
        // Static.value is always present
        let value_ty = self.check_expr(&static_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?Trait 婵犵數濮伴崹鐟帮耿鏉堛劍娅犳俊銈傚亾閸?
    fn check_trait_decl(&mut self, trait_decl: &Trait) -> Result<()> {
        use crate::typeck::r#trait::{MethodSig, TraitInfo};

        self.env.push_scope();
        self.bind_type_params_with_meta(&trait_decl.type_params)?;

        let mut trait_info = TraitInfo::new(
            trait_decl.name.name.clone(),
            trait_decl
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect(),
            matches!(trait_decl.vis, Visibility::Public),
        );

        // 闂傚倷娴囬妴鈧柛瀣崌閺屾盯顢曢敐鍡欘槰闂佽壈灏欐繛鈧柡宀嬬節瀹曟帒鈹戦幇顓犵Х缂備胶铏庨崢鍏兼櫠鎼达絽鍨濋柣銏㈩焾缁犳氨鎲告惔銊ョ９?
        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    self.env.push_scope();
                    self.bind_type_params_with_meta(&method.type_params)?;
                    // 闂傚倷娴囬妴鈧柛瀣崌閺屾盯顢曢敐鍡欘槰闂佽壈灏欐繛鈧柡灞剧☉閳诲氦绠涢幘顖氫壕鐟滅増甯掑Ч鏌ユ煟閺冨洦顏犻悘蹇曟暬閹綊宕堕鍕闂?
                    let mut param_types = Vec::new();
                    let mut has_self = false;

                    for param in &method.params {
                        if param.name.name == "self" {
                            has_self = true;
                        } else {
                            let ty = self.check_type(&param.ty)?;
                            param_types.push(ty);
                        }
                    }

                    // 闂傚倷绀侀崥瀣磿閹惰棄搴婇柤鑹扮堪娴滃綊鏌涢妷锝呭濞存嚎鍊濋弻锟犲磼濞戞﹩鍤嬬紓浣插亾闁逞屽墰缁辨挻绗熼崶褌鍑藉銈嗘肠閸ャ劌鈧?
                    let ret_ty = if let Some(ret) = &method.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };

                    // A trait method has a default implementation if its body is non-empty
                    let has_default = !method.body.stmts.is_empty();
                    let sig = if has_default {
                        MethodSig::with_default(has_self, param_types, ret_ty)
                    } else {
                        MethodSig::new(has_self, param_types, ret_ty)
                    };
                    trait_info.add_method(method.name.name.clone(), sig);
                    self.env.pop_scope();
                }
                TraitItem::Const(const_decl) => {
                    let ty = self.check_type(&const_decl.ty)?;
                    trait_info.add_const(const_decl.name.name.clone(), ty);
                }
                TraitItem::Type(type_alias) => {
                    trait_info.add_assoc_type(type_alias.name.name.clone());
                }
            }
        }

        self.trait_registry.register(trait_info);

        self.env.pop_scope();
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?Impl 婵犵數濮伴崹鐟帮耿鏉堛劍娅犳俊銈傚亾閸?
    fn check_impl_decl(&mut self, impl_decl: &Impl) -> Result<()> {
        use crate::typeck::r#trait::type_key;
        use crate::typeck::r#trait::{FunctionTy, ImplInfo};

        self.env.push_scope();
        self.bind_type_params_with_meta(&impl_decl.type_params)?;

        let target_ty = self.check_type(&impl_decl.target_type)?;
        let target_key = type_key(&target_ty);

        let trait_name = impl_decl
            .trait_path
            .as_ref()
            .and_then(|p| p.as_simple())
            .map(|s| s.name.clone());

        let mut impl_info = ImplInfo::new(target_ty.clone(), trait_name);

        // 闂傚倷娴囬妴鈧柛瀣崌閺屾盯顢曢敐鍡欘槰闂佽壈灏欐繛鈧柡宀嬬節瀹曟帒鈹戦幇顓犵Х缂?
        for item in &impl_decl.items {
            self.env.push_scope();
            self.bind_type_params_with_meta(&item.type_params)?;
            let mut param_types = Vec::new();
            let mut has_self = false;

            for param in &item.params {
                if param.name.name == "self" {
                    has_self = true;
                } else {
                    let ty = self.check_type(&param.ty)?;
                    param_types.push(ty);
                }
            }

            let ret_ty = if let Some(ret) = &item.return_type {
                self.check_type(ret)?
            } else {
                self.env.unit_ty()
            };

            impl_info.add_method(
                item.name.name.clone(),
                FunctionTy::new(has_self, param_types, ret_ty),
            );
            self.env.pop_scope();
        }

        // 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩炴繛瀵稿Т椤戝懐绮?Impl 濠电姷鏁搁崑娑⑺囬銏犵鐎光偓閸曨偉鍩為梺浼欑到閺堫剟宕?
        if let Some(trait_name) = impl_info.trait_name.clone() {
            // For trait impls, also register default methods from the trait
            // definition that are not overridden by the impl.
            // Also check that all required (non-default) methods are implemented.
            if let Some(trait_info) = self.trait_registry.get(&trait_name) {
                let mut missing_methods = Vec::new();

                for (method_name, method_sig) in &trait_info.methods {
                    if !impl_info.has_method(method_name) {
                        if method_sig.has_default {
                            // This method has a default implementation in the trait
                            // and is not overridden 闂?register it in the impl info
                            impl_info.add_method(
                                method_name.clone(),
                                FunctionTy::new(
                                    method_sig.has_self,
                                    method_sig.param_types.clone(),
                                    method_sig.return_type.clone(),
                                ),
                            );
                        } else {
                            // This method is required but not implemented
                            missing_methods.push(method_name.clone());
                        }
                    }
                }

                if !missing_methods.is_empty() {
                    missing_methods.sort();
                    self.env.pop_scope();
                    let err = TypeckError::Other(format!(
                        "impl `{}` for `{}` is missing required trait methods: {}",
                        trait_name,
                        target_key,
                        missing_methods.join(", ")
                    ));
                    return Err(CompileError::TypeckError(err));
                }
            }

            self.impl_registry
                .register_trait_impl(trait_name, target_key, impl_info);
        } else {
            self.impl_registry.register_inherent(target_key, impl_info);
        }

        self.env.pop_scope();
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶戦梺鑽ゅ仦閸戝綊宕戞繝鍌滄殾?
    fn path_name(&self, path: &Path) -> TyResult<String> {
        path.as_simple()
            .map(|ident| ident.name.clone())
            .ok_or_else(|| TypeckError::UndefinedType {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
    }

    fn builtin_type_by_name(&mut self, name: &str) -> Option<Ty> {
        Some(match name {
            "()" => self.env.unit_ty(),
            "bool" => self.env.bool_ty(),
            "i8" => self.env.int_ty(IntKind::I8),
            "i16" => self.env.int_ty(IntKind::I16),
            "i32" => self.env.int_ty(IntKind::I32),
            "i64" => self.env.int_ty(IntKind::I64),
            "i128" => self.env.int_ty(IntKind::I128),
            "isize" => self.env.int_ty(IntKind::ISize),
            "u8" => self.env.int_ty(IntKind::U8),
            "u16" => self.env.int_ty(IntKind::U16),
            "u32" => self.env.int_ty(IntKind::U32),
            "u64" => self.env.int_ty(IntKind::U64),
            "u128" => self.env.int_ty(IntKind::U128),
            "usize" => self.env.int_ty(IntKind::USize),
            "f32" => self.env.float_ty(FloatKind::F32),
            "f64" => self.env.float_ty(FloatKind::F64),
            "str" => self.env.str_ty(),
            "char" => self.env.new_ty(TyKind::Char),
            "!" => self.env.never_ty(),
            _ => return None,
        })
    }

    fn substitute_ty_vars(&self, ty: &Ty, subst: &HashMap<TyVarId, Ty>) -> Ty {
        match &ty.kind {
            TyKind::Var(var_id) => subst.get(var_id).cloned().unwrap_or_else(|| ty.clone()),
            TyKind::Tuple(types) => Ty {
                id: ty.id,
                kind: TyKind::Tuple(
                    types
                        .iter()
                        .map(|inner| self.substitute_ty_vars(inner, subst))
                        .collect(),
                ),
            },
            TyKind::Array(elem, len) => Ty {
                id: ty.id,
                kind: TyKind::Array(Box::new(self.substitute_ty_vars(elem, subst)), *len),
            },
            TyKind::Slice(elem) => Ty {
                id: ty.id,
                kind: TyKind::Slice(Box::new(self.substitute_ty_vars(elem, subst))),
            },
            TyKind::Ref(is_mut, inner) => Ty {
                id: ty.id,
                kind: TyKind::Ref(*is_mut, Box::new(self.substitute_ty_vars(inner, subst))),
            },
            TyKind::Ptr(inner) => Ty {
                id: ty.id,
                kind: TyKind::Ptr(Box::new(self.substitute_ty_vars(inner, subst))),
            },
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => Ty {
                id: ty.id,
                kind: TyKind::Fn {
                    params: params
                        .iter()
                        .map(|param| self.substitute_ty_vars(param, subst))
                        .collect(),
                    ret: Box::new(self.substitute_ty_vars(ret, subst)),
                    is_variadic: *is_variadic,
                },
            },
            TyKind::Adt { name, args } => Ty {
                id: ty.id,
                kind: TyKind::Adt {
                    name: name.clone(),
                    args: args
                        .iter()
                        .map(|arg| self.substitute_ty_vars(arg, subst))
                        .collect(),
                },
            },
            _ => ty.clone(),
        }
    }

    fn resolve_generic_type_args(
        &self,
        type_name: &str,
        meta: &GenericTypeMeta,
        explicit_args: Vec<Ty>,
    ) -> TyResult<Vec<Ty>> {
        if explicit_args.len() > meta.params.len() {
            return Err(TypeckError::Other(format!(
                "type {} expects at most {} generic arguments, found {}",
                type_name,
                meta.params.len(),
                explicit_args.len()
            )));
        }

        let mut resolved = Vec::with_capacity(meta.params.len());
        let mut subst = HashMap::<TyVarId, Ty>::new();

        for (index, param) in meta.params.iter().enumerate() {
            let current = if let Some(arg) = explicit_args.get(index) {
                arg.clone()
            } else if let Some(default_ty) = &param.default {
                self.substitute_ty_vars(default_ty, &subst)
            } else {
                return Err(TypeckError::Other(format!(
                    "missing generic argument {} for type {}",
                    param.name, type_name
                )));
            };

            for bound in &param.bounds {
                let concrete_key = type_key(&current);
                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                    return Err(TypeckError::Other(format!(
                        "generic constraint violated in type {}: {} does not implement {} for {}",
                        type_name, current, bound, param.name
                    )));
                }
            }

            subst.insert(param.var_id, current.clone());
            resolved.push(current);
        }

        Ok(resolved)
    }

    fn check_path_type(&mut self, path: &Path, explicit_args: Vec<Ty>) -> TyResult<Ty> {
        let name = self.path_name(path)?;

        if let Some(meta) = self.generic_type_metas.get(&name).cloned() {
            let args = self.resolve_generic_type_args(&name, &meta, explicit_args)?;
            return Ok(self.env.new_ty(TyKind::Adt { name, args }));
        }

        if !explicit_args.is_empty() {
            return Err(TypeckError::Other(format!("type {} is not generic", name)));
        }

        if let Some(symbol) = self.env.lookup(&name) {
            if let Some(ty) = symbol.get_ty() {
                return Ok(ty.clone());
            }
        }

        if let Some(ty) = self.builtin_type_by_name(&name) {
            return Ok(ty);
        }

        Err(TypeckError::UndefinedType { name })
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶戦梺鑽ゅ仦閸戝綊宕戞繝鍌滄殾?
    fn check_type(&mut self, ty: &Type) -> TyResult<Ty> {
        Ok(match &ty.kind {
            TypeKind::Path(path) => self.check_path_type(path, Vec::new())?,
            TypeKind::PathWithArgs { path, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.check_type(arg))
                    .collect::<TyResult<Vec<_>>>()?;
                self.check_path_type(path, args)?
            }
            TypeKind::Tuple(types) => {
                let elem_types = types
                    .iter()
                    .map(|t| self.check_type(t))
                    .collect::<TyResult<Vec<_>>>()?;
                self.env.tuple_ty(elem_types)
            }
            TypeKind::Array(elem, len) => {
                let elem_ty = self.check_type(elem)?;
                self.env.array_ty(elem_ty, *len as usize)
            }
            TypeKind::Slice(elem) => {
                let elem_ty = self.check_type(elem)?;
                self.env.slice_ty(elem_ty)
            }
            TypeKind::Ptr { base, is_mut: _ } => {
                let inner_ty = self.check_type(base)?;
                self.env.new_ty(TyKind::Ptr(Box::new(inner_ty)))
            }
            TypeKind::Ref { base, is_mut } => {
                let inner_ty = self.check_type(base)?;
                self.env.ref_ty(*is_mut, inner_ty)
            }
            TypeKind::Fn { params, ret } => {
                let param_types = params
                    .iter()
                    .map(|p| self.check_type(p))
                    .collect::<TyResult<Vec<_>>>()?;
                let ret_ty = match ret {
                    Some(r) => self.check_type(r)?,
                    None => self.env.unit_ty(),
                };
                self.env.fn_ty(param_types, ret_ty)
            }
            TypeKind::Never => self.env.never_ty(),
            TypeKind::Infer => self.infer.fresh_ty_var(),
            TypeKind::Dyn(trait_bounds) => {
                let names: Vec<String> = trait_bounds
                    .iter()
                    .filter_map(|b| b.path.as_simple())
                    .map(|ident| ident.name.clone())
                    .collect();
                self.env.new_ty(TyKind::Dyn(names))
            }
            TypeKind::ImplTrait(trait_bounds) => {
                let names: Vec<String> = trait_bounds
                    .iter()
                    .filter_map(|b| b.path.as_simple())
                    .map(|ident| ident.name.clone())
                    .collect();
                self.env.new_ty(TyKind::ImplTrait(names))
            }
        })
    }

    fn check_expr(&mut self, expr: &Expr) -> TyResult<Ty> {
        match &expr.kind {
            ExprKind::Literal(lit) => self.check_literal(lit),
            ExprKind::Ident(ident) => self.check_ident(ident),
            ExprKind::Binary { op, left, right } => self.check_binary(op, left, right),
            ExprKind::Unary { op, operand } => self.check_unary(op, operand),
            ExprKind::Assign { target, value } => self.check_assign(target, value),
            ExprKind::AssignOp { op, target, value } => self.check_assign_op(op, target, value),
            ExprKind::Index { base, index } => self.check_index(base, index),
            ExprKind::Field { base, field } => self.check_field(base, field),
            ExprKind::Call { func, args } => self.check_call(func, args),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call(receiver, method, args),
            ExprKind::Tuple(elems) => self.check_tuple(elems),
            ExprKind::Array(elems) => self.check_array(elems),
            ExprKind::Block(block) => self.check_block(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.check_if(cond, then_branch, else_branch),
            ExprKind::While { cond, body } => self.check_while(cond, body),
            ExprKind::For {
                pattern,
                iter,
                body,
            } => self.check_for(pattern, iter, body),
            ExprKind::Loop(body) => self.check_loop(body),
            ExprKind::Match { scrutinee, arms } => self.check_match(scrutinee, arms),
            ExprKind::Return(value) => self.check_return(value),
            ExprKind::Break(value) => self.check_break(value),
            ExprKind::Continue => self.check_continue(),
            ExprKind::Path(path) => self.check_path(path),
            ExprKind::Lambda { params, body } => self.check_lambda(params, body),
            ExprKind::Struct { path, fields, .. } => {
                let name = path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .unwrap_or_default();

                let field_defs = self
                    .struct_field_defs
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| TypeckError::UndefinedType { name: name.clone() })?;

                let mut field_types: HashMap<String, Ty> = HashMap::new();
                for (field_name, field_ty) in field_defs {
                    field_types.insert(field_name, self.check_type(&field_ty)?);
                }

                let mut seen = HashSet::new();
                for field_value in fields {
                    let field_name = match &field_value.name {
                        crate::ast::FieldName::Ident(ident) => ident.name.clone(),
                        crate::ast::FieldName::String(name) => name.clone(),
                    };

                    if !seen.insert(field_name.clone()) {
                        return Err(TypeckError::Other(format!(
                            "duplicate struct literal field `{}` for `{}`",
                            field_name, name
                        )));
                    }

                    let expected_ty = field_types.get(&field_name).cloned().ok_or_else(|| {
                        TypeckError::FieldNotFound {
                            type_name: name.clone(),
                            field_name: field_name.clone(),
                        }
                    })?;

                    let value_ty = self.check_expr(&field_value.value)?;
                    self.infer.unify(&expected_ty, &value_ty)?;
                }

                if let Some(symbol) = self.env.lookup(&name) {
                    if let Some(ty) = symbol.get_ty() {
                        Ok(ty.clone())
                    } else {
                        Ok(self.env.new_ty(TyKind::Adt { name, args: vec![] }))
                    }
                } else {
                    Err(TypeckError::UndefinedType { name })
                }
            }
            _ => Ok(self.env.error_ty()),
        }
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛闂備胶鍎垫慨宥夊炊椤垶顥堥梻渚€娼х换鍫ュ磹閺囩偐鏋?
    fn check_literal(&mut self, lit: &Literal) -> TyResult<Ty> {
        Ok(match lit {
            Literal::Int(_) => self.env.int_ty(IntKind::I64), // 婵犳鍠楃敮妤冪矙閹烘せ鈧箓宕奸妷顔芥櫍婵犵數濮甸懝楣冨几娓氣偓閹鈽夊▍铏灥閳绘捇宕奸弴鐔封偓鍨箾閹寸偟鎳愭繛鍫熺矋缁绘盯姊婚弶鎴濈ギ闂佸搫鑻惌浣虹不濞戞瑦鍎熼柕鍫濇祩濡?i64
            Literal::Float(_) => self.env.float_ty(FloatKind::F64),
            Literal::String(_) => {
                let str_ty = self.env.str_ty();
                self.env.ref_ty(false, str_ty)
            }
            Literal::Char(_) => self.env.new_ty(TyKind::Char),
            Literal::Bytes(_) => self.env.new_ty(TyKind::Bytes),
            Literal::Bool(_) => self.env.bool_ty(),
            Literal::Null => self.env.new_ty(TyKind::Adt {
                name: "Option".to_string(),
                args: vec![],
            }),
            Literal::Unit => self.env.unit_ty(),
        })
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潥闂備胶绮悧顓犲緤閸ф绠柣妯款嚙楠炪垺绻涢崱妯忣亪宕?
    fn check_ident(&mut self, ident: &Ident) -> TyResult<Ty> {
        let symbol = if let Some(symbol) = self.env.lookup(&ident.name) {
            symbol.clone()
        } else {
            return Err(TypeckError::UndefinedVariable {
                name: ident.name.clone(),
            });
        };

        match &symbol.kind {
            SymbolKind::Function { ty, .. } => {
                Ok(self.infer.instantiate_with_fresh_vars(ty.clone()))
            }
            _ => {
                if let Some(ty) = symbol.get_ty() {
                    Ok(self.infer.instantiate(ty.clone()))
                } else {
                    Err(TypeckError::UndefinedVariable {
                        name: ident.name.clone(),
                    })
                }
            }
        }
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶撻梻浣规た閸樹粙宕曢幎绛嬫晪?
    fn check_path(&mut self, path: &Path) -> TyResult<Ty> {
        if let Some(ident) = path.as_simple() {
            self.check_ident(ident)
        } else {
            Err(TypeckError::UndefinedVariable {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
        }
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潚缂傚倷闄嶉崝澶愬疾閻樿尙鏆︽俊銈呮噹缁€鍐┿亜閹炬潙顥氶柛瀣崌瀹曟﹢鍩￠崒婊呅ら梻浣筋嚃閸ㄥ酣宕ㄩ锝嗘暏
    fn check_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr) -> TyResult<Ty> {
        let left_ty = self.check_expr(left)?;
        let right_ty = self.check_expr(right)?;

        self.infer
            .unify(&left_ty, &right_ty)
            .map_err(|_| TypeckError::TypeMismatch {
                expected: right_ty.kind.clone(),
                found: left_ty.kind.clone(),
            })?;

        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => left_ty,
            BinOp::And | BinOp::Or => self.env.bool_ty(),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => left_ty,
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.env.bool_ty()
            }
            BinOp::Pipe | BinOp::Compose | BinOp::Range | BinOp::RangeInclusive => left_ty,
        })
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潚缂傚倷鐒﹀褰掑箰閸愯尙鏆︽俊銈呮噹缁€鍐┿亜閹炬潙顥氶柛瀣崌瀹曟﹢鍩￠崒婊呅ら梻浣筋嚃閸ㄥ酣宕ㄩ锝嗘暏
    fn check_unary(&mut self, op: &UnOp, operand: &Expr) -> TyResult<Ty> {
        let ty = self.check_expr(operand)?;
        Ok(match op {
            UnOp::Neg | UnOp::Not | UnOp::Plus | UnOp::BitNot => ty.clone(),
            UnOp::Deref => {
                if let Some(inner) = ty.ref_inner() {
                    inner.clone()
                } else {
                    return Err(TypeckError::TypeMismatch {
                        expected: TyKind::Ref(false, Box::new(self.env.error_ty())),
                        found: ty.kind.clone(),
                    });
                }
            }
            UnOp::Ref => self.env.ref_ty(false, ty),
            UnOp::RefMut | UnOp::DerefMut => self.env.ref_ty(true, ty),
        })
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶撶紓鍌欐閻掞箓骞愰崘鑼殾?
    fn check_assign(&mut self, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛婵＄偑鍊ч梽鍕偂閳ュ磭鏆﹂柕澶嗘櫅闁卞洭鏌ㄥ┑鍡樺偍闁稿鍋ゅ?
    fn check_assign_op(&mut self, _op: &AssignOp, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶戦梻浣侯焾鐞氼偊宕濆畝鍕垫晪闁挎繂顦崡鎶藉箳閹惰棄鐒垫い鎺嗗亾闁哥噥鍨伴…鍥疀濞戞鐣鹃悷婊冪箳濞戠敻鍩€?
    fn check_index(&mut self, base: &Expr, index: &Expr) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;
        let index_ty = self.check_expr(index)?;

        if !index_ty.is_int() {
            return Err(TypeckError::TypeMismatch {
                expected: TyKind::Int(IntKind::ISize),
                found: index_ty.kind.clone(),
            });
        }

        Ok(match &base_ty.kind {
            TyKind::Array(elem, _) => (**elem).clone(),
            TyKind::Slice(elem) => (**elem).clone(),
            TyKind::Tuple(types) if !types.is_empty() => types[0].clone(),
            _ => self.env.error_ty(),
        })
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛闂備胶鍎垫慨宥夊礃閿濆棛浜栧┑鐐舵彧缁蹭粙宕板璺虹柈闊洦绋掗埛?
    fn check_field(&mut self, base: &Expr, name: &Ident) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;

        match &base_ty.kind {
            TyKind::Adt {
                name: type_name, ..
            } => {
                let field_defs =
                    self.struct_field_defs
                        .get(type_name)
                        .cloned()
                        .ok_or_else(|| TypeckError::FieldNotFound {
                            type_name: type_name.clone(),
                            field_name: name.name.clone(),
                        })?;

                let field_ty = field_defs
                    .into_iter()
                    .find(|(field_name, _)| field_name == &name.name)
                    .map(|(_, field_ty)| field_ty)
                    .ok_or_else(|| TypeckError::FieldNotFound {
                        type_name: type_name.clone(),
                        field_name: name.name.clone(),
                    })?;

                self.check_type(&field_ty)
            }
            _ => Err(TypeckError::FieldNotFound {
                type_name: base_ty.kind.to_string(),
                field_name: name.name.clone(),
            }),
        }
    }

    fn resolve_struct_field_types(&mut self, struct_name: &str) -> TyResult<Vec<(String, Ty)>> {
        let field_defs = self
            .struct_field_defs
            .get(struct_name)
            .cloned()
            .ok_or_else(|| {
                TypeckError::Other(format!(
                    "print cannot resolve fields for struct `{}`",
                    struct_name
                ))
            })?;

        let mut resolved = Vec::with_capacity(field_defs.len());
        for (field_name, field_ty) in field_defs {
            let ty = self.check_type(&field_ty)?;
            resolved.push((field_name, ty));
        }
        Ok(resolved)
    }

    fn ensure_type_printable_for_print(
        &mut self,
        ty: &Ty,
        context: &str,
        visiting: &mut HashSet<String>,
    ) -> TyResult<()> {
        match &ty.kind {
            TyKind::Int(_) | TyKind::Bool | TyKind::Float(_) | TyKind::Str => Ok(()),
            TyKind::Ref(_, inner) if matches!(inner.kind, TyKind::Str) => Ok(()),
            TyKind::Adt { name, .. } => self.ensure_struct_printable(name, context, visiting),
            _ => Err(TypeckError::Other(format!(
                "print does not support field `{}` of type {}",
                context, ty.kind
            ))),
        }
    }

    fn ensure_struct_printable(
        &mut self,
        struct_name: &str,
        context: &str,
        visiting: &mut HashSet<String>,
    ) -> TyResult<()> {
        if !visiting.insert(struct_name.to_string()) {
            return Ok(());
        }

        let fields = self.resolve_struct_field_types(struct_name)?;
        for (field_name, field_ty) in fields {
            let field_context = format!("{}.{}", context, field_name);
            self.ensure_type_printable_for_print(&field_ty, &field_context, visiting)?;
        }

        visiting.remove(struct_name);
        Ok(())
    }

    fn check_call(&mut self, func: &Expr, args: &[Expr]) -> TyResult<Ty> {
        // Special handling for `print` builtin function
        // Check both Ident and Path (single-segment) since the parser may produce either
        let is_print = match &func.kind {
            ExprKind::Ident(ident) => ident.name == "print",
            ExprKind::Path(path) => path.segments.len() == 1 && path.segments[0].name == "print",
            _ => false,
        };
        if is_print {
            // print expects exactly one argument
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let arg_ty = self.check_expr(&args[0])?;
            let mut visiting = HashSet::new();
            let context = match &arg_ty.kind {
                TyKind::Adt { name, .. } => name.clone(),
                _ => "print argument".to_string(),
            };
            self.ensure_type_printable_for_print(&arg_ty, &context, &mut visiting)?;

            // print returns unit
            return Ok(self.env.unit_ty());
        }

        let direct_fn_name = match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.clone()),
            ExprKind::Path(path) if path.segments.len() == 1 => Some(path.segments[0].name.clone()),
            _ => None,
        };

        let mut generic_ctx: Option<(String, GenericFunctionMeta, HashMap<TyVarId, TyVarId>)> =
            None;
        let func_ty = if let Some(name) = direct_fn_name {
            match self.env.lookup(&name).cloned() {
                Some(Symbol {
                    kind: SymbolKind::Function { ty, .. },
                    ..
                }) => {
                    if let Some(meta) = self.generic_function_metas.get(&name).cloned() {
                        let (instantiated, var_map) =
                            self.infer.instantiate_with_fresh_vars_and_map(ty);
                        generic_ctx = Some((name, meta, var_map));
                        instantiated
                    } else {
                        self.infer.instantiate_with_fresh_vars(ty)
                    }
                }
                _ => self.check_expr(func)?,
            }
        } else {
            self.check_expr(func)?
        };

        if let TyKind::Fn { params, ret, .. } = &func_ty.kind {
            if params.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: params.len(),
                    found: args.len(),
                });
            }

            for (arg_ty, arg_expr) in params.iter().zip(args.iter()) {
                let actual_ty = self.check_expr(arg_expr)?;
                self.infer.unify(arg_ty, &actual_ty)?;
            }

            if let Some((name, meta, var_map)) = generic_ctx.as_ref() {
                self.enforce_generic_function_constraints(name, meta, var_map)?;
            }

            Ok(self.infer.apply_subst(ret))
        } else {
            Err(TypeckError::UndefinedFunction {
                name: "closure".to_string(),
            })
        }
    }

    fn enforce_generic_function_constraints(
        &mut self,
        function_name: &str,
        meta: &GenericFunctionMeta,
        var_map: &HashMap<TyVarId, TyVarId>,
    ) -> TyResult<()> {
        for param in &meta.params {
            let mut concrete_ty = if let Some(instantiated_var) = var_map.get(&param.var_id) {
                let placeholder = Ty::new(0, TyKind::Var(*instantiated_var));
                self.infer.apply_subst(&placeholder)
            } else if let Some(default_ty) = &param.default {
                // Generic parameter is not present in function type (phantom generic).
                // In this case, default type is the only inference source.
                self.infer.apply_subst(default_ty)
            } else if param.bounds.is_empty() {
                // Unused unconstrained generic parameter does not affect call typing.
                // Keep backward compatibility for benchmark and existing code.
                continue;
            } else {
                return Err(TypeckError::Other(format!(
                    "cannot infer generic type parameter `{}` in call to `{}`",
                    param.name, function_name
                )));
            };

            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                if let Some(default_ty) = &param.default {
                    let default_ty = self.infer.apply_subst(default_ty);
                    self.infer.unify(&concrete_ty, &default_ty)?;
                    concrete_ty = self.infer.apply_subst(&default_ty);
                }
            }

            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                return Err(TypeckError::Other(format!(
                    "cannot infer generic type parameter `{}` in call to `{}`",
                    param.name, function_name
                )));
            }

            for trait_name in &param.bounds {
                let concrete_key = type_key(&concrete_ty);
                if !self
                    .impl_registry
                    .implements_trait(trait_name, &concrete_key)
                {
                    return Err(TypeckError::Other(format!(
                        "generic constraint violated in `{}`: `{}` does not implement `{}` for `{}`",
                        function_name, concrete_key, trait_name, param.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潥闂備礁鎼惉濂稿窗鎼淬劌鐓濋柡鍐ㄧ墕閸楁娊鏌ㄥ☉妯侯仾妞ゆ柨绉瑰?
    fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &Ident,
        args: &[Expr],
    ) -> TyResult<Ty> {
        use crate::typeck::r#trait::type_key;

        let receiver_ty = self.check_expr(receiver)?;
        let receiver_key = type_key(&receiver_ty);

        let mut arg_types = Vec::new();
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        let method_name = &method.name;

        // Built-in string method: (&str).len() -> i64
        let is_str_ref =
            matches!(&receiver_ty.kind, TyKind::Ref(_, inner) if matches!(inner.kind, TyKind::Str));
        if is_str_ref && method_name == "len" {
            if !args.is_empty() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 0,
                    found: args.len(),
                });
            }
            return Ok(self.env.int_ty(crate::typeck::ty::IntKind::I64));
        }

        // Inherent impl lookup first.
        if let Some(fn_ty) = self
            .impl_registry
            .lookup_inherent_method(&receiver_key, method_name)
        {
            if fn_ty.param_types.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: fn_ty.param_types.len(),
                    found: args.len(),
                });
            }

            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }

            return Ok(self.infer.apply_subst(&fn_ty.return_type));
        }

        // Then trait impl lookup.
        for trait_name in self.trait_registry.all_traits() {
            if let Some(fn_ty) =
                self.impl_registry
                    .lookup_trait_method(&trait_name, &receiver_key, method_name)
            {
                if fn_ty.param_types.len() != args.len() {
                    return Err(TypeckError::ArgumentCountMismatch {
                        expected: fn_ty.param_types.len(),
                        found: args.len(),
                    });
                }

                for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                    self.infer.unify(expected, actual)?;
                }

                return Ok(self.infer.apply_subst(&fn_ty.return_type));
            }
        }

        Err(TypeckError::MethodNotFound {
            type_name: receiver_key,
            method_name: method_name.clone(),
        })
    }

    fn check_tuple(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        let elem_types = elems
            .iter()
            .map(|e| self.check_expr(e))
            .collect::<TyResult<Vec<_>>>()?;
        Ok(self.env.tuple_ty(elem_types))
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潥闂備礁鎼Λ娑㈠窗閹捐埖顫?
    fn check_array(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        if elems.is_empty() {
            return Ok(self.env.array_ty(self.infer.fresh_ty_var(), 0));
        }

        let first_ty = self.check_expr(&elems[0])?;
        for elem in &elems[1..] {
            let ty = self.check_expr(elem)?;
            self.infer.unify(&first_ty, &ty)?;
        }

        Ok(self.env.array_ty(first_ty, elems.len()))
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?Lambda 闂傚倸鍊搁崐鎼佸磻閸℃稑鍌ㄩ柤娴嬫杹閸嬫捇宕归顐ゅ姺闂佽鍠曢崡铏繆閹间礁惟闁挎洍鍋撴繛鎳峰洦鍊?`|params| body`
    /// Lambda 闂傚倷鐒﹂惇褰掑礉瀹€鈧埀顒佸嚬閸犳岸骞冮鈧、鏇㈡晝閳ь剟宕归崒鐐村€甸柨婵嗛娴滄繄绮幋锔解拺闁告稑锕ら悘鍗炩攽椤斿搫鈧繂顕ｉ幎鑺ュ亜闁惧繒鎳撻弳妤呮煟閻樺弶绌块悘蹇旂懇閸┾偓妞ゆ帒鍊搁崢鎾煛娴ｅ摜肖濞寸媴绠撻幐濠冨緞鐎ｅ灚顥ら梻鍌欐祰濡椼劑鎮為敂鐣岀彾闁糕剝鐟﹂崑鏍ㄣ亜閹板墎鐣遍柛銊ュ€块幃妤呮晲閸屾稒鐝栫紓浣瑰姈椤ㄥ﹪骞冪捄渚僵闁绘挸绨肩花濂告煟閵忊晛鐏犵紓宥咃工椤曪綁顢楅崟鍨櫍濠电娀娼ч敃锕偹囨导瀛樷拺闁告繂瀚埀顒冾潐缁旂喖宕卞缁樼亖闂佺懓顕慨椋庝焊閻㈠憡鍋ｉ柛銉簻閻ㄦ椽鏌嶈閸撴盯宕楀Ο铏规殾闁挎繂鎷嬪銊╂煃瑜滈崜鐔风暦?
    fn check_lambda(&mut self, params: &[Ident], body: &Expr) -> TyResult<Ty> {
        // 婵犵數鍋為崹鍫曞箰妤ｅ啫纾块柕鍫濐槹閸庡﹪鏌嶉埡浣告殶闁崇粯姊归妵鍕疀閹炬剚浠煎┑鈽嗗亝閿曘垽寮婚埄鍐╁閻熸瑥瀚崙锟犳⒑閹肩偛濡肩€规洦鍓熼、姘枎閹炬潙鈧粯淇婇婵嗗惞闁告ɑ鍔欏鍝勑ч崶褍顬堥柣搴㈠嚬閸犳岸骞冮鈧、鏇㈡晝閳ь剟宕归崒鐐村€甸柨婵嗙凹缁ㄨ崵绱撳鍕獢婵?
        let param_tys: Vec<Ty> = params.iter().map(|_| self.infer.fresh_ty_var()).collect();

        // 闂傚倷绀侀幉锛勬暜濡ゅ啰鐭欓柟瀵稿Х绾句粙鏌熼幑鎰靛殭婵☆偅锕㈤弻鐔封枔閸喗鐏嶉梺浼欑到瀹曨剟婀侀梺鎸庣箓濡盯鎯屽畝鍕厸濞达綁娼ч埀顒佺箓閻ｅ嘲煤椤忓懎浜滈梺鍛婄☉閿曨亜顬婃搴ｇ＝闁稿本鐟ч崝宥夋煕閵娧勬毈闁诡喚鍋撻妶锝夊礃閵娧呭幀闂備胶顭堥張顒傜矙閹烘垟鏋?
        self.env.push_scope();

        // 闂備浇顕х换鎰崲閹邦儵娑樜旈埀顒勵敋閿濆鏁嗛柛鏇ㄥ亝閻庮剟姊虹憴鍕靛晱闁哥姵宀搁獮蹇涘Ω閳哄倸鈧敻鎮峰▎蹇擃伂濠㈣锕㈤弻娑㈠煛娴ｈ鎷遍梺鐟板槻閹冲酣鈥﹂妸鈺佸窛妞ゆ棁濮ら褰掓⒒娴ｈ櫣銆婇柡鍛箞瀹曟澘顓兼径瀣畼?
        for (param, ty) in params.iter().zip(param_tys.iter()) {
            self.env.insert_var(param.name.clone(), ty.clone());
        }

        // 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?body 闂傚倷鐒﹂惇褰掑礉瀹€鈧埀顒佸嚬閸犳岸骞冮鈧、鏇㈡晝閳ь剟宕?
        let body_ty = self.check_expr(body)?;

        // 闂佽瀛╅鏍窗濮樿泛绠犻柟鎹愵嚙閸氳銇勯幘鍗炵仼闁活厽顨婇弻娑氫沪閸撗€濮囧┑鐐茬墣濞夋盯婀侀梺鎸庣箓濡盯鎯屽畝鍕厸濞达綁娼ч埀顒佺箓閻?
        self.env.pop_scope();

        // Lambda 闂傚倷鐒﹂惇褰掑礉瀹€鈧埀顒佸嚬閸犳岸骞冮鈧、鏇㈡晝閳ь剟宕归崒鐐村€甸柨婵嗛娴滄繄绮幋锔解拺闁告稑锕ら悘鍗炩攽椤斿搫鈧繂顕ｉ幎鑺ュ亜闁惧繒鎳撻弳妤呮煟閻樺弶绌块悘蹇旂懇閸┾偓?(params -> ret)
        Ok(self.env.fn_ty(param_tys, body_ty))
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺戭潛婵?
    fn check_block(&mut self, block: &Block) -> TyResult<Ty> {
        self.env.push_scope();

        let mut result_ty = self.env.unit_ty();
        for stmt in &block.stmts {
            if let Some(ty) = self.check_stmt(stmt)? {
                result_ty = ty;
            }
        }

        self.env.pop_scope();
        Ok(result_ty)
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡宀嬬磿娴狅妇鎷犻幓鎺懶撴俊鐐€栧ú蹇涘垂閽樺鏆?
    fn check_stmt(&mut self, stmt: &Stmt) -> TyResult<Option<Ty>> {
        match &stmt.kind {
            StmtKind::Let {
                name, ty, value, ..
            } => {
                let var_ty = if let Some(ty) = ty {
                    self.check_type(ty)?
                } else {
                    self.infer.fresh_ty_var()
                };

                // value 闂?Option<Box<Expr>>
                let value_ty = match value {
                    Some(v) => self.check_expr(v)?,
                    None => self.env.unit_ty(),
                };
                self.infer.unify(&var_ty, &value_ty)?;

                self.env.insert_var(name.name.clone(), var_ty);
                Ok(None)
            }
            StmtKind::Const { name, ty, value } => {
                let var_ty = self.check_type(ty)?;
                let value_ty = self.check_expr(value)?;
                self.infer.unify(&var_ty, &value_ty)?;
                self.env.insert_var(name.name.clone(), var_ty);
                Ok(None)
            }
            StmtKind::Expr(expr) => {
                let ty = self.check_expr(expr)?;
                Ok(Some(ty))
            }
            StmtKind::Item(item) => {
                // check_decl 闂備礁鎼ˇ顐﹀疾濠婂牆钃熼柕濞垮剭?Result<()>闂傚倷鐒︾€笛呯矙閹达附鍎楅柛灞惧搸閳ь剚甯″畷婊勬媴閻熺増姣囧┑鐐舵彧缂嶁偓濠殿喓鍊楃划濠囶敋閳ь剟寮婚悢鑲╁祦闁割煈鍠氭导鍫ユ⒑鏉炴壆顦﹂柨鏇畵楠?
                self.check_decl(item)
                    .map_err(|e| TypeckError::Other(e.to_string()))?;
                Ok(None)
            }
        }
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?if 闂備浇宕甸崑鐐电矙韫囨稑绀夐煫鍥ㄧ☉缁犲灚銇勮箛鎾愁伌闁?
    fn check_if(
        &mut self,
        cond: &Expr,
        then_branch: &Block,
        else_branch: &Option<Box<Expr>>,
    ) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        let then_ty = self.check_block(then_branch)?;
        let else_ty = match else_branch {
            Some(e) => self.check_expr(e)?,
            None => self.env.unit_ty(),
        };

        self.infer.unify(&then_ty, &else_ty)?;
        Ok(then_ty)
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?while 闂佽娴烽弫濠氬磻婵犲啰顩查柣鎰瀹?
    fn check_while(&mut self, cond: &Expr, body: &Block) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?for 闂佽娴烽弫濠氬磻婵犲啰顩查柣鎰瀹?
    fn check_for(&mut self, pattern: &Pattern, iter: &Expr, body: &Block) -> TyResult<Ty> {
        self.check_expr(iter)?;
        let elem_ty = self.env.int_ty(IntKind::I64); // 婵犵數鍋犻幓顏嗙礊閳ь剚绻涙径瀣鐎?I64 闂傚倷绀侀崥瀣熆濡崵闄勯柡鍐ㄥ€荤粻鏂款熆閼搁潧濮囬柛?I32

        self.env.push_scope();

        // 婵?pattern 婵犵數鍋為崹鍫曞箹閳哄懎鍌ㄩ柛濠勫枂娴滅懓銆掑锝呬壕閻庤娲╃紞浣割嚕閸婄噥妲荤紓鍌氱С缁舵艾顫忓ú顏勭闁圭儤姊婚鍥⒑?
        let var_name = match &pattern.kind {
            crate::ast::pattern::PatternKind::Ident(name) => name.name.clone(),
            crate::ast::pattern::PatternKind::Wildcard => "_loop".to_string(),
            _ => "_loop".to_string(),
        };

        self.env.insert_var(var_name, elem_ty);
        self.check_block(body)?;
        self.env.pop_scope();

        Ok(self.env.unit_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?loop 闂佽娴烽弫濠氬磻婵犲啰顩查柣鎰瀹?
    fn check_loop(&mut self, body: &Block) -> TyResult<Ty> {
        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?match 闂備浇宕甸崑鐐电矙韫囨稑绀夐煫鍥ㄧ☉缁犲灚銇勮箛鎾愁伌闁?
    fn check_match(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> TyResult<Ty> {
        self.check_expr(scrutinee)?;

        let mut arm_types = Vec::new();
        for arm in arms {
            if let Some(guard) = &arm.guard {
                self.check_expr(guard)?;
            }
            let arm_ty = self.check_expr(&arm.body)?;
            arm_types.push(arm_ty);
        }

        let result_ty = arm_types
            .first()
            .cloned()
            .unwrap_or_else(|| self.env.unit_ty());
        for arm_ty in &arm_types {
            self.infer.unify(&result_ty, arm_ty)?;
        }

        Ok(result_ty)
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?return 闂備浇宕甸崑鐐电矙韫囨稑绀夐煫鍥ㄧ☉缁犲灚銇勮箛鎾愁伌闁?
    fn check_return(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                self.check_expr(v)?;
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?break 闂備浇宕甸崑鐐电矙韫囨稑绀夐煫鍥ㄧ☉缁犲灚銇勮箛鎾愁伌闁?
    fn check_break(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                self.check_expr(v)?;
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 濠电姷顣藉Σ鍛村磻閳ь剟鏌涚€ｎ偅宕岄柡?continue 闂備浇宕甸崑鐐电矙韫囨稑绀夐煫鍥ㄧ☉缁犲灚銇勮箛鎾愁伌闁?
    fn check_continue(&mut self) -> TyResult<Ty> {
        Ok(self.env.never_ty())
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
