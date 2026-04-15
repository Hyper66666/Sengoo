//! 缂傚倷绶￠崑澶愵敋瑜旈幃妤€鐣￠幏鏃€鐩崺鈧い鎺戝閽冪喖鏌曟径鍫濆姕闁绘縿鍊濋幃妤呭捶椤撶偘鎴峰Δ鐘靛仜濞差參寮鍛殕闁告劦浜跺Σ褰掓煟閻樺啿濮х紒韫矙椤㈡岸鎮虹粈顢洪梺鍝勵槴閺呮粎绮欓弽顓溾偓渚€骞嬮悩闈涙殫闁荤姴娲╃亸娆撳焵椤掑嫭鏁辨い銊ｅ劚閻ｆ繈宕橀鍜佹綌闂備線娼уΛ鏃堟倿閿旂偓鏆滅憸鐗堝笒濡ɑ銇勯幘璺轰沪閻庢俺灏欑槐鎺楀箻閸涘瓨顎嶅┑鐐碘拡閸嬪嫯鐏嬮梺閫炲苯澧寸€殿喖顭锋俊鐑芥晲閸屾矮澹?
//! 闂佽楠稿﹢閬嶅磻閻旇偐宓侀柛銉ｅ妿椤╂煡鏌涘┑鍫濈€琻goo闂佽崵濮村ù鍌炲矗閳ь剟鏌嶇拠鑼ⅵ闁诡垰鍟村畷鐔碱敇閳藉棙钑夐梻浣规偠閸斿矂顢栭崶顒佺劸閻庨潧鎽滅壕濂告煟閺囩偛鈧崵绮堟径鎰厱闁哄诞鍕闁诲孩鍑归崢濂稿煝閺傚簱妲堟俊顖氱毞閸嬫捇鎮滈挊澹┿儱鈽夐弮鍌滐紘ait缂傚倷鐒﹂幏婵嬫⒔閸曨偒鐒芥い鎰剁畱杩濇繛杈剧秮缂傛岸锝為敃鍌涚厱闁靛鍎洪崕鎾寸箾閺夋垶顥㈡鐐村灴婵偓闁靛繈鍨洪幆锝夋⒑閸濆嫮肖妞ゎ厼鐗忕紓鎾淬偅閸愩劎顦梺缁樻椤曆囧储閹绢喗鐓?
use crate::ast::pattern::Pattern;
use crate::ast::Visibility;
use crate::ast::*;
use crate::error::CompileError;
use crate::method_resolution::{
    ambiguous_method_error, select_method_candidate, MethodCandidate, MethodCandidateMatch,
};
use crate::typeck::env::{Symbol, SymbolKind, TypeEnv};
use crate::typeck::ffi as ffi_check;
use crate::typeck::infer::TypeInfer;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplRegistry, TraitRegistry};
use crate::typeck::ty::{FloatKind, IntKind, Ty, TyKind, TyVarId, TypeckError};
use crate::Result;
use std::collections::{BTreeSet, HashMap, HashSet};

type TyResult<T> = std::result::Result<T, TypeckError>;

mod stmt_helpers;
mod expr_helpers;
mod decl_helpers;

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

/// 缂傚倷绶￠崑澶愵敋瑜旈幃妤€鐣￠幏鏃€鐩崺鈧い鎺戝閽冪喖鏌曟径鍫濆姕闁绘縿鍊濋弻銊モ槈濡偐鍔紓浣插亾閻庯綆鍓氱€氬鏌ㄥ┑鍡樺闁搞倗鍤凷T闂佸搫顦弲婊呯矙閺嶎厹鈧線骞嬮悩闈涙殫闁荤姴娲╃亸娆撳焵椤掑嫭鏁辨い銊ｅ劚閻ｆ繈宕橀鍜佹綌闂備線娼уΛ鏃堟倿閿旂偓鏆滅憸鐗堝笒濡ɑ銇勯幘璺轰沪閻庢俺灏欑槐鎺楀箻閸涘瓨顎嶅┑鐐碘拡閸嬪嫯鐏嬮梺閫炲苯澧寸€殿喖顭锋俊鐑芥晲閸屾矮澹?
pub struct TypeChecker {
    /// 缂傚倷绶￠崑澶愵敋瑜旈幃妤呮倻閼恒儱浠㈠銈嗙墬閸戝綊寮抽弮鍫熺叆婵炴垶顭堢€氫即鏌ｆ幊閸旀垵鐣烽幒妤€惟闁靛鍎崇粔顕€姊绘担鐟扮祷缂佺粯鍔欓獮蹇涘箥椤旀儳鏆楅柣鐘叉穿鐏忔瑩鍩€椤掑倸鍘撮柟顖氬暣瀹曠喖顢橀悩鍐差仱闂佽姘﹂～澶愭嚌妤ｅ啫鐭楃憸蹇涘礌閺嶎灝褔宕堕敐鍛?
    env: TypeEnv,
    /// 缂傚倷绶￠崑澶愵敋瑜旈幃妤呮倻閽樺顔婇梺闈涱煭闂勫嫰鎮炴禒瀣厱闁挎繂娴烽崢婊呯磼鏉堛劎绠為柟顔荤矙婵℃悂濡堕崶顏勵棟缂傚倷绶￠崑澶愵敋瑜旈幃妤呮倻閽樺鐓戝┑顔姐仜閸嬫捇鏌涢敐鍡樸仢闁诡垰鍟村畷鐔碱敆閳ь剟鎳欒ぐ鎺撶厸闁稿被鍊曢獮鏍煙闁垮顥嬬紒杈ㄥ浮閹垽宕滄笟鍥︾礃闂?
    infer: TypeInfer,
    /// Trait婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑妯汇亜椤撶喎绗х紒鈧径鎰€垫繛鎴烆仾椤忓牆纾荤€广儱顦粻銉╂煃瑜滈崜鐔奉嚕閸偄绶炲┑鐘插€稿▓銉︾節閵忕姵璐″褎顨呭嵄闁硅揪闄勯崕宥夋煕閹捐櫕顎檃it濠电儑绲藉ú鐘诲礈濠靛洤顕遍柛娑欐綑杩?
    trait_registry: TraitRegistry,
    /// Impl婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑妯汇亜椤撶喎绗х紒鈧径鎰€垫繛鎴烆仾椤忓牆纾荤€广儱顦粻銉╂煃瑜滈崜鐔奉嚕閸偆绠欑紒鍗炲劖it闂佽楠稿﹢閬嶅磻閻旇偐宓侀柛銉墯閸庡秹鏌涢弴銊ょ胺缂佽妫濋弻鐔碱敇瑜嶉悘娑㈡煃?
    impl_registry: ImplRegistry,
    struct_field_defs: HashMap<String, Vec<(String, Type)>>,
    struct_type_params: HashMap<String, Vec<TypeParam>>,
    class_decls: HashMap<String, ClassDeclInfo>,
    generic_function_metas: HashMap<String, GenericFunctionMeta>,
    generic_type_metas: HashMap<String, GenericTypeMeta>,
    async_context_depth: usize,
    async_functions: HashSet<String>,
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
            struct_type_params: HashMap::new(),
            class_decls: HashMap::new(),
            generic_function_metas: HashMap::new(),
            generic_type_metas: HashMap::new(),
            async_context_depth: 0,
            async_functions: HashSet::new(),
        }
    }

    pub fn async_function_names(&self) -> &HashSet<String> {
        &self.async_functions
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲冀椤愩倕鏆楅柣鐘叉穿鐏忔瑩鍩€椤掑倸鍘撮柟顔藉▕椤㈡稑顫濋崗鍏碱吘闂備焦鐪归崝宀€鈧凹浜炵划顓熷緞閹邦剛鐓戝銈嗙墬閼瑰墽绱炴担琛″亾濞堝灝鏋熸繛鍙夌矒瀵偊濡舵径濞?
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲冀椤愩倕鏆楅柣鐘叉穿鐏忔瑩鍩€椤掑倸鍘存鐐村浮婵℃悂鏁傞悾灞藉Ш闂備線娼荤徊濠氬礉鎼粹檧鏋旈柟瀵稿仧閳绘梻鈧箍鍎遍幊搴ｆ媼閺屻儲鐓曟繛鍡樺厧椤忓棙顫曢柍鍝勬噺閸嬨劑鏌曟繝蹇曞矝闁?
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲级椤ｆit婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑姗€鏌曟繛褍瀚弳鐘崇箾閹寸偞灏紒澶岊棎閵囨劙寮婚妷銉х厬濠碘槅鍨靛▍锝囩矚閸ф鐓熸い顐墮婵℃悂鏌?
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲级婵Ιl婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑姗€鏌曟繛褍瀚弳鐘崇箾閹寸偞灏紒澶岊棎閵囨劙寮婚妷銉х厬濠碘槅鍨靛▍锝囩矚閸ф鐓熸い顐墮婵℃悂鏌?
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲级椤ｆit婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑姗€鏌曟繛褍瀚弳鐘绘⒑閸涘﹦鎳愰柛銊﹀閵囨劖寰勬繝搴℃闂佸搫娲ㄦ慨鐢稿汲韫囨稒鐓?
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// 闂佸搫顦弲婊堝蓟閵娿儍娲级婵Ιl婵犵數鍋涢ˇ顓㈠礉瀹€鍕埞闁伙絽鏈崑姗€鏌曟繛褍瀚弳鐘绘⒑閸涘﹦鎳愰柛銊﹀閵囨劖寰勬繝搴℃闂佸搫娲ㄦ慨鐢稿汲韫囨稒鐓?
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// 闂佽绨肩徊濠氾綖婢舵劕姹查柨婵嗘閳绘梹銇勮箛鎾搭棏闁哄被鍊濋獮鏍级閹寸姷顔戠紓浣虹帛椤洭骞冮弶璺ㄦ殕闁逞屽墰閹噣顢曢敃鈧崹鍌炴倵閿濆骸浜芥慨濠囩畺閺岋紕浠︾拠鎻掑Х缂備浇椴哥换鍫濈暦閻樿宸濇い鏃傗拡娴犵偓绻濋姀鐘宠础濠⒀勵殔鍗遍柟瀵稿剳缁憋綁鏌涢弴銊ヤ航婵℃煡浜堕弻锝夋倷閸欏妫為梺纭呮腹閸楀啿鐣峰Δ鍛╅柍杞扮劍椤忊€斥攽閳藉棗浜愰柛瀣崌閺岋紕浠︾拠鎻掑闂?
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

    /// 濠碘槅鍋呭妯尖偓姘煎墴閺佸秹骞囬弶鍨挤闁瑰吋鐣崝濠囧焵椤掆偓閿曪附绂掗敃鍌氱＜婵ê澧介惌妤呮⒑閸濆嫬顏╅柛鎴犳櫕濡叉劕鈻庨幘宕囶槱闂佸吋浜介崕鎻掆枔閸洘鐓忛柛鈩冩礉閸忓瞼绱掗鍝勫鐎殿喗鎮傚畷鐔碱敂閸℃顏堟⒑闂堟稒顥欓柣鎺炵畵閹﹢宕堕妸褉鏋栭梺鍛婃寙閸涱喗鍎梻浣瑰缁嬫垿篓閳ь剛绱掓潏銊х畼濞存粍鎮傚畷鍗炍旈埀顒€顕ラ幘鍓佺＜妞ゆ劑鍎洪崵娆撴煟鎼淬劍鏁辩紒宀勪憾閸╁嫰宕樿缁€鈧繝鐢靛仜椤︻參宕濆畝鍕埞濞寸姴顑呯粈鍡涙煙濞堝灝鏋涚紒鐘宠壘椤儻顦查柛鐔叉櫊閹虫瑩骞嬮敃鈧繚?
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

                // 濠电姰鍨煎▔娑氣偓姘煎櫍楠炲啯绻濋崒妤佺亙濠电偞鍨堕…鍥焵椤掑倸鍘寸€规洘鐟╅幊鐐哄Ψ閿旂瓔鍟€闂備礁鎲￠悷銉╁磹瑜版帒姹查柣鏃囨鐏忕敻鎮归崶顏勭毢闁逞屽墴椤ユ挾妲愰幒妤€绠ｆ繝闈涚墛濮ｅ酣姊洪幐搴ｂ槈闁活厺鑳堕幑銏犖熼崗鐓庝粧閻庡厜鍋撻柍褜鍓涢幃顕€顢曢敃鈧崹鍌炴倵閿濆骸浜芥慨濠囩畺閺岋紕浠︾拠鎻掑闂?
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
                    // 婵犵數鍋涢ˇ鎵矙閹达附鍊甸柣鎴ｆ閻鏌涚仦鍓р姇婵炲牆澧庣槐鎾存媴閻熸壆浠撮梺浼欑悼閸嬫挸顕ラ崟顒佺秶妞ゆ劑鍎涢弴銏＄厸闁告劦鍘界涵鐐箾閸欏澧甸柡灞芥嚇閸┾偓妞ゆ帊绀佺欢鐐烘煕閺囥劌骞楅柛濠勫仱閺?
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
                self.struct_type_params
                    .insert(struct_decl.name.name.clone(), struct_decl.type_params.clone());
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

    /// 闂佽娴烽弫鎼佹偩閻栧穬it濠电姰鍨瑰﹢杈ㄦ櫠濡も偓鍗遍柟瀵稿仦娴溿倖绻涢幋鐐电煠闁衡偓閻ｅ瞼纾兼い鎰╁労閸ゆ瑩鏌ｆ惔鈥虫Щ閾荤偤鏌嶈閸撶喎顕ｆ繝姘ㄦい鏍ㄧ矌椤︻喗淇婇妶鍥㈤柣顓濈窔閹焦寰勯幇顒侇棟闁荤喐鐟ョ€氼噣濡存繝鍕＜婵炴垶鐟ラ崝銈夋煙椤旇姤绀嬬€规洦浜炴禒锕傛寠婢跺﹦绋戦梻浣藉妞寸顫忛懡銈嗗仏妞ゆ劧绠戦崹鍌炴倵閿濆啫濡烽柛?
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

        // 婵犵妲呴崑鈧柛瀣崌閺屸€崇暆婵犲喚浠璻ait濠电偞鍨堕幖鈺呭储婵傜绠栭柛銉ｅ妿閳绘梹銇勮箛鎾村櫤闁绘挻鐟︾换娑欏緞鐎ｎ偆顦┑鐐茬墛閸ㄥ湱鍒掗悽绋跨畾鐟滃酣宕愰幎鑺ョ厱婵犲﹤鍟崇€氫即鏌ｉ妶鍛伃闁诡喗濞婇幊婊堫敄閽樺澹?
        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    self.env.push_scope();
                    let method_generic_meta = self.bind_type_params_with_meta(&method.type_params)?;
                    // 缂傚倸鍊烽悞锕傚垂閻㈠憡鍋╁Δ锝呭暙濡﹢鎮峰▎蹇擃伀闁靛棗锕︾槐鎺楁偑濞嗗繐鏆欓柣鎺戙偢閺岋綁濡搁妷銉痪缂佺虎鍙冪粻鏍х暦閸︻厸鍋撻敐搴″⒉妞ゎ偓绠撻弻娑㈠Ω閵夘喖鍓伴悗娈垮櫘閸撶喎顕ｉ锔藉殝妞ゆ垼妫勬禍?
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

                    // 婵犵妲呴崑鈧柛瀣崌閺岋紕浠︾拠鎻掑闂佸搫鐬奸崰搴ㄥ煝閺冨牆鍗抽柣妯虹－閺嗙娀鏌℃径鍡樻珕闁哄被鍔岀叅闁哄稁鍋嗙亸鐢告偣閸ヮ亜鐨洪柍褜鍓涢崗妯讳繆?
                    let ret_ty = if let Some(ret) = &method.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };

                    // A trait method has a default implementation if its body is non-empty
                    let has_default = !method.body.stmts.is_empty();
                    let sig = if has_default {
                        MethodSig::with_default(
                            has_self,
                            param_types,
                            ret_ty,
                            method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                        )
                    } else {
                        MethodSig::new(
                            has_self,
                            param_types,
                            ret_ty,
                            method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                        )
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

    /// 闂佽娴烽弫鎼侇敆閿曞窋l闂備胶顫嬮崟鍨暯缂備胶绮…鍥箖閺夎法鏆嗛柍褜鍓涢幃顕€顢曢敃鈧崹鍌炴倵閿濆骸浜芥慨濠囩畺閺岋紕浠︾拠鎻掑Х缂備浇椴哥换鍐€冮妷銉ф殕闁告劦浜濋～宥夋煟鎼粹剝璐￠柛瀣枛閻涱噣宕堕鈧崣濠冦亜閺嶃劎顣查柛搴★功缁辨帒螖鐎ｎ剛鏆犻梺璇″枛缁愭仧ait缂傚倷鐒﹂幏婵嬫⒔閸曨偒鐒芥い鎰剁畱杩?
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

        // 婵犵妲呴崑鈧柛瀣崌閺屸€崇暆婵犲啰褰瞞pl闂備胶顫嬮崟鈺€娌梺鐓庣仛閸ㄥ灝鐣烽妷鈺佺妞ゅ繐鐗嗗▓婵嬫⒑閸濆嫬鈧粙锝炴径灞惧厹闁割偅娲橀崕宥夋煕閺囥劌浜為柡澶夌矙閺岋絽顭ㄩ崱妤冨姺闂?
        for item in &impl_decl.items {
            self.env.push_scope();
            let method_generic_meta = self.bind_type_params_with_meta(&item.type_params)?;
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
                FunctionTy::with_generic_params(
                    has_self,
                    param_types,
                    ret_ty,
                    method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                ),
            );
            self.env.pop_scope();
        }

        // 濠德板€楁慨鎾儗娓氣偓閹焦绻呴惇鎶猯闂備礁鎼€氱兘宕规导鏉戠畾濞达絽澹婇悡銉╂⒔閸ヮ剙绀冪紒顐ょ殾ait闂備焦鐪归崝宀€鈧凹鍠氶埀顒冾潐婵炲﹤顕ｉ崼鏇熷亜婵炶尙绮惌妤€鈹戦鐐殌闁稿﹤鐖奸崺鈧?
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
                            // 闂佽崵濮村ú銏ゅ磿閺屻儱钃熼柣鏂跨殱閺嬫牠鏌￠崶鈺佹瀾缂佺姵甯掗～妤€鈽夊▎妯煎姼濡炪倖鎹佸畷鐢稿焵椤掆偓濠€閬嶅磻閻旇偐宓侀柛銉墯閺咁剟鎮橀悙鏉戝姢婵炲吋鍨块弻娑㈠籍閸屾銏ゆ煕閳轰緡鍚檓pl濠电儑绲藉ú鐘诲礈濠靛洤顕遍柛娑卞枤閳绘梹銇勯幘鏉戭棜闁?
                            impl_info.add_method(
                                method_name.clone(),
                                FunctionTy::with_generic_params(
                                    method_sig.has_self,
                                    method_sig.param_types.clone(),
                                    method_sig.return_type.clone(),
                                    method_sig.generic_params.clone(),
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

    /// 闂佽绻愮换鎰涘鍫熷剹闁诡垎灞芥闂佸憡娲栨晶搴ｇ矆閸戠浛th闂備焦瀵х粙鎴λ囩€圭媭鏆伴梻浣告惈椤戝棝宕濋幒鏃€瀚婚柣鏃囥€€閸嬫挸鈽夊▎妯荤暭濡炪倖鍨抽悞锔剧矙婢舵劕绠甸柟鐑樻尭閳ь剙纾槐鎺撳緞鎼达紕鍔烽梺?
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

    fn generic_lookup_key(&self, ty: &Ty) -> String {
        match &ty.kind {
            TyKind::Adt { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    format!("{}<{}>", name, vec!["?"; args.len()].join(","))
                }
            }
            TyKind::Ref(_, inner) => format!("&{}", self.generic_lookup_key(inner)),
            TyKind::Ptr(inner) => format!("*{}", self.generic_lookup_key(inner)),
            TyKind::Array(elem, len) => format!("[{}; {}]", self.generic_lookup_key(elem), len),
            TyKind::Slice(elem) => format!("[{}]", self.generic_lookup_key(elem)),
            TyKind::Tuple(types) => {
                if types.is_empty() {
                    "()".to_string()
                } else {
                    format!(
                        "({})",
                        types
                            .iter()
                            .map(|ty| self.generic_lookup_key(ty))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            _ => type_key(ty),
        }
    }

    fn match_generic_impl_target(
        &self,
        pattern: &Ty,
        concrete: &Ty,
        subst: &mut HashMap<TyVarId, Ty>,
    ) -> bool {
        match (&pattern.kind, &concrete.kind) {
            (TyKind::Var(var_id), _) => {
                if let Some(bound) = subst.get(var_id) {
                    bound == concrete
                } else {
                    subst.insert(*var_id, concrete.clone());
                    true
                }
            }
            (
                TyKind::Adt {
                    name: lhs_name,
                    args: lhs_args,
                },
                TyKind::Adt {
                    name: rhs_name,
                    args: rhs_args,
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args
                        .iter()
                        .zip(rhs_args.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
            }
            (TyKind::Ref(lhs_mut, lhs_inner), TyKind::Ref(rhs_mut, rhs_inner)) => {
                lhs_mut == rhs_mut && self.match_generic_impl_target(lhs_inner, rhs_inner, subst)
            }
            (TyKind::Ptr(lhs_inner), TyKind::Ptr(rhs_inner)) => {
                self.match_generic_impl_target(lhs_inner, rhs_inner, subst)
            }
            (TyKind::Array(lhs_elem, lhs_len), TyKind::Array(rhs_elem, rhs_len)) => {
                lhs_len == rhs_len && self.match_generic_impl_target(lhs_elem, rhs_elem, subst)
            }
            (TyKind::Slice(lhs_elem), TyKind::Slice(rhs_elem)) => {
                self.match_generic_impl_target(lhs_elem, rhs_elem, subst)
            }
            (TyKind::Tuple(lhs_types), TyKind::Tuple(rhs_types)) => {
                lhs_types.len() == rhs_types.len()
                    && lhs_types
                        .iter()
                        .zip(rhs_types.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
            }
            (
                TyKind::Fn {
                    params: lhs_params,
                    ret: lhs_ret,
                    is_variadic: lhs_variadic,
                },
                TyKind::Fn {
                    params: rhs_params,
                    ret: rhs_ret,
                    is_variadic: rhs_variadic,
                },
            ) => {
                lhs_variadic == rhs_variadic
                    && lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
                    && self.match_generic_impl_target(lhs_ret, rhs_ret, subst)
            }
            _ => pattern.kind == concrete.kind,
        }
    }

    fn instantiate_method_function_ty(
        &mut self,
        fn_ty: &FunctionTy,
        subst: &HashMap<TyVarId, Ty>,
    ) -> FunctionTy {
        let mut call_subst = subst.clone();
        for generic_param in &fn_ty.generic_params {
            call_subst.insert(*generic_param, self.env.new_ty_var());
        }
        FunctionTy::new(
            fn_ty.has_self,
            fn_ty
                .param_types
                .iter()
                .map(|param| self.substitute_ty_vars(param, &call_subst))
                .collect(),
            self.substitute_ty_vars(&fn_ty.return_type, &call_subst),
        )
    }
    fn lookup_generic_inherent_method(
        &mut self,
        receiver_ty: &Ty,
        method_name: &str,
    ) -> Option<FunctionTy> {
        let lookup_key = self.generic_lookup_key(receiver_ty);
        let impls = self.impl_registry.get_inherent_impls(&lookup_key);

        for impl_info in impls {
            let mut subst = HashMap::new();
            if !self.match_generic_impl_target(&impl_info.target_type, receiver_ty, &mut subst) {
                continue;
            }
            if let Some(fn_ty) = impl_info.get_method(method_name).cloned() {
                return Some(self.instantiate_method_function_ty(&fn_ty, &subst));
            }
        }
        None
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

    /// 闂佽绻愮换鎰板箰濞ｇ寘缂傚倷绶￠崑澶愵敋瑜旈幃妤呮倻閼恒儱娈熼梺绋挎湰缁本绂掗鐐茬骇闁绘垵妫楁晶顖溾偓瑙勬礀瀵墎绮欐径鎰紶闁告洦鍋呴弳銉╂⒒娓氬洤鏋旈柛鏂挎捣閹噣顢曢敃鈧崹鍌炴倵閿濆骸骞戦柍褜鍓欓崯鎵垝閸儲鍋栭悗闈涙憸椤︺彆y闂備焦瀵х粙鎴λ囬銏犵劦?
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
            ExprKind::Await(expr) => {
                if self.async_context_depth == 0 {
                    return Err(TypeckError::Other(
                        "await is only allowed in async contexts".to_string(),
                    ));
                }
                let inner_ty = self.check_expr(expr)?;
                match &inner_ty.kind {
                    TyKind::Future(result_ty) => Ok(result_ty.as_ref().clone()),
                    _ => Err(TypeckError::Other(
                        "await requires a Future value (call to an async function)".to_string(),
                    )),
                }
            }
            ExprKind::AsyncBlock(block) => {
                self.async_context_depth += 1;
                let result = self.check_block(block);
                self.async_context_depth = self.async_context_depth.saturating_sub(1);
                let inner_ty = result?;
                Ok(Ty::new(0, TyKind::Future(Box::new(inner_ty))))
            }
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
                let type_params = self.struct_type_params.get(&name).cloned().unwrap_or_default();

                if !type_params.is_empty() {
                    self.env.push_scope();
                    let result = (|| -> TyResult<Ty> {
                        let generic_meta = self
                            .bind_type_params_with_meta(&type_params)
                            .map_err(|err| TypeckError::Other(err.to_string()))?;

                        let mut field_types: HashMap<String, Ty> = HashMap::new();
                        for (field_name, field_ty) in &field_defs {
                            field_types.insert(field_name.clone(), self.check_type(field_ty)?);
                        }

                        self.check_struct_literal_fields(&name, fields, &field_types)?;

                        let mut args = Vec::with_capacity(generic_meta.len());
                        for param in &generic_meta {
                            let placeholder = Ty::new(0, TyKind::Var(param.var_id));
                            let mut concrete_ty = self.infer.apply_subst(&placeholder);
                            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                                if let Some(default_ty) = &param.default {
                                    concrete_ty =
                                        self.substitute_ty_vars(default_ty, &HashMap::new());
                                } else {
                                    return Err(TypeckError::Other(format!(
                                        "cannot infer generic argument `{}` for struct `{}` literal",
                                        param.name, name
                                    )));
                                }
                            }
                            for bound in &param.bounds {
                                let concrete_key = type_key(&concrete_ty);
                                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                                    return Err(TypeckError::Other(format!(
                                        "generic constraint violated in struct `{}` literal: `{}` does not implement `{}` for `{}`",
                                        name, concrete_key, bound, param.name
                                    )));
                                }
                            }
                            args.push(concrete_ty);
                        }

                        Ok(self.env.new_ty(TyKind::Adt { name, args }))
                    })();
                    self.env.pop_scope();
                    result
                } else {
                    let mut field_types: HashMap<String, Ty> = HashMap::new();
                    for (field_name, field_ty) in field_defs {
                        field_types.insert(field_name, self.check_type(&field_ty)?);
                    }

                    self.check_struct_literal_fields(&name, fields, &field_types)?;

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
            }
            _ => Ok(self.env.error_ty()),
        }
    }

    fn check_struct_literal_fields(
        &mut self,
        struct_name: &str,
        fields: &[FieldValue],
        field_types: &HashMap<String, Ty>,
    ) -> TyResult<()> {
        let mut seen = HashSet::new();
        let mut provided_known = HashSet::new();
        let mut missing = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        let mut unknown = BTreeSet::new();

        for field_value in fields {
            let field_name = match &field_value.name {
                crate::ast::FieldName::Ident(ident) => ident.name.clone(),
                crate::ast::FieldName::String(name) => name.clone(),
            };

            let is_first = seen.insert(field_name.clone());
            if !is_first {
                duplicates.insert(field_name.clone());
            }

            let Some(expected_ty) = field_types.get(&field_name).cloned() else {
                unknown.insert(field_name);
                continue;
            };

            if !is_first {
                continue;
            }

            provided_known.insert(field_name);
            let value_ty = self.check_expr(&field_value.value)?;
            if self.contains_future_escape_ty(&value_ty) {
                return Err(Self::future_escape_error());
            }
            self.infer.unify(&expected_ty, &value_ty)?;
        }

        for field_name in field_types.keys() {
            if !provided_known.contains(field_name) {
                missing.insert(field_name.clone());
            }
        }

        if missing.is_empty() && duplicates.is_empty() && unknown.is_empty() {
            return Ok(());
        }

        Err(Self::invalid_struct_literal_error(
            struct_name,
            &missing,
            &duplicates,
            &unknown,
        ))
    }

    fn invalid_struct_literal_error(
        struct_name: &str,
        missing: &BTreeSet<String>,
        duplicates: &BTreeSet<String>,
        unknown: &BTreeSet<String>,
    ) -> TypeckError {
        let mut parts = Vec::new();
        if !missing.is_empty() {
            parts.push(format!(
                "missing fields: {}",
                Self::format_struct_field_names(missing)
            ));
        }
        if !duplicates.is_empty() {
            parts.push(format!(
                "duplicate fields: {}",
                Self::format_struct_field_names(duplicates)
            ));
        }
        if !unknown.is_empty() {
            parts.push(format!(
                "unknown fields: {}",
                Self::format_struct_field_names(unknown)
            ));
        }

        TypeckError::Other(format!(
            "invalid struct literal `{}`: {}",
            struct_name,
            parts.join("; ")
        ))
    }

    fn format_struct_field_names(fields: &BTreeSet<String>) -> String {
        fields
            .iter()
            .map(|field| format!("`{}`", field))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn future_escape_error() -> TypeckError {
        TypeckError::Other("future values cannot escape; await the async call directly".to_string())
    }

    fn contains_future_escape_ty(&self, ty: &Ty) -> bool {
        let resolved = self.infer.apply_subst(ty);
        Self::ty_contains_future_escape(&resolved)
    }

    fn ty_contains_future_escape(ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Future(_) => true,
            TyKind::Tuple(types) => types.iter().any(Self::ty_contains_future_escape),
            TyKind::Array(elem, _) | TyKind::Slice(elem) => Self::ty_contains_future_escape(elem),
            TyKind::Ref(_, inner) | TyKind::Ptr(inner) => Self::ty_contains_future_escape(inner),
            TyKind::Fn { params, ret, .. } => {
                params.iter().any(Self::ty_contains_future_escape)
                    || Self::ty_contains_future_escape(ret)
            }
            TyKind::Adt { args, .. } => args.iter().any(Self::ty_contains_future_escape),
            _ => false,
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
        let builtin_name = match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.as_str()),
            ExprKind::Path(path) if path.segments.len() == 1 => Some(path.segments[0].name.as_str()),
            _ => None,
        };

        if builtin_name == Some("spawn") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "spawn is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "spawn requires a Future value".to_string(),
                ));
            }

            return Ok(future_ty);
        }

        if builtin_name == Some("spawn_task") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "spawn_task is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "spawn_task requires a Future value".to_string(),
                ));
            }

            return Ok(self.env.int_ty(IntKind::I64));
        }

        if builtin_name == Some("sleep") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "sleep is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let duration_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&duration_ty, &i64_ty)?;
            return Ok(Ty::new(0, TyKind::Future(Box::new(self.env.unit_ty()))));
        }

        if builtin_name == Some("timeout") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "timeout is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "timeout requires a Future value".to_string(),
                ));
            }

            let duration_ty = self.check_expr(&args[1])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&duration_ty, &i64_ty)?;
            return Ok(Ty::new(0, TyKind::Future(Box::new(self.env.bool_ty()))));
        }

        if builtin_name == Some("join") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "join is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            for arg in args {
                let future_ty = self.check_expr(arg)?;
                if !future_ty.is_future() {
                    return Err(TypeckError::Other(
                        "join requires Future values".to_string(),
                    ));
                }
            }

            return Ok(self.env.unit_ty());
        }

        if builtin_name == Some("cancel_task") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "cancel_task is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let task_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&task_ty, &i64_ty)?;
            return Ok(self.env.bool_ty());
        }

        if builtin_name == Some("task_status") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "task_status is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let task_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&task_ty, &i64_ty)?;
            return Ok(i64_ty);
        }

        if builtin_name == Some("select") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "select is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let left_future = self.check_expr(&args[0])?;
            let right_future = self.check_expr(&args[1])?;
            let TyKind::Future(left_inner) = &left_future.kind else {
                return Err(TypeckError::Other(
                    "select requires Future values".to_string(),
                ));
            };
            let TyKind::Future(right_inner) = &right_future.kind else {
                return Err(TypeckError::Other(
                    "select requires Future values".to_string(),
                ));
            };

            self.infer.unify(left_inner, right_inner)?;
            let result_ty = self.infer.apply_subst(left_inner);
            if !matches!(result_ty.kind, TyKind::Int(_) | TyKind::Bool | TyKind::Float(_)) {
                return Err(TypeckError::Other(
                    "select currently only supports Future values whose results are bool, integer, or float scalars"
                        .to_string(),
                ));
            }
            return Ok(result_ty);
        }

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
                // Passing an unawaited Future as a function argument is an escape.
                // The caller must `await` it at the call-site first.
                if self.contains_future_escape_ty(&actual_ty) {
                    return Err(TypeckError::Other(
                        "future values cannot be passed as arguments; await the async call first"
                            .to_string(),
                    ));
                }
                self.infer.unify(arg_ty, &actual_ty)?;
            }

            if let Some((name, meta, var_map)) = generic_ctx.as_ref() {
                self.enforce_generic_function_constraints(name, meta, var_map)?;
            }

            let resolved_ret = self.infer.apply_subst(ret);

            let is_async_call = match &func.kind {
                ExprKind::Ident(ident) => self.async_functions.contains(&ident.name),
                ExprKind::Path(path) if path.segments.len() == 1 => {
                    self.async_functions.contains(&path.segments[0].name)
                }
                _ => false,
            };
            if is_async_call {
                Ok(Ty::new(0, TyKind::Future(Box::new(resolved_ret))))
            } else {
                Ok(resolved_ret)
            }
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

    /// 婵犵妲呴崑鈧柛瀣崌閺岋紕浠︾拠鎻掑闂佸搫鐬奸崰搴ㄥ煝閺冨牆鍗抽柨娑樺椤斿秹姊哄ú缁樺▏闁告柨绻樻俊鎾礃閳哄倻绉堕梺璇″瀻閸涱垼鏀ㄩ梻浣瑰缁嬫垿鎯屾担鐑樺床婵☆垰鍚嬮崑姗€鎮橀悙闈涗壕闁绘挻鐟︾换娑欏緞鐎ｎ偆顦悶姘懇閺岋紕鈧綆鍋勬慨鍥煙閾氬倸宓嗙€规洩缍佸畷鎺戔槈濞嗘劦鍟€缂傚倷绶￠崑澶愵敋瑜旈幃妤€鐣￠幏鏃€鐩崺鈧い鎺戝閽冪喖鏌曟径娑氬矝闁?
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
        let exact_inherent = self
            .impl_registry
            .lookup_inherent_method(&receiver_key, method_name)
            .cloned();
        if let Some(fn_ty) = exact_inherent
            .map(|fn_ty| self.instantiate_method_function_ty(&fn_ty, &HashMap::new()))
            .or_else(|| self.lookup_generic_inherent_method(&receiver_ty, method_name))
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
        if let Some(fn_ty) =
            self.select_trait_method_call_candidate(&receiver_key, method_name, args.len())?
        {
            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }
            return Ok(self.infer.apply_subst(&fn_ty.return_type));
        }

        Err(TypeckError::MethodNotFound {
            type_name: receiver_key,
            method_name: method_name.clone(),
        })
    }

    fn select_trait_method_call_candidate(
        &mut self,
        receiver_key: &str,
        method_name: &str,
        arg_count: usize,
    ) -> TyResult<Option<FunctionTy>> {
        let mut candidates = Vec::new();
        for trait_name in self.trait_registry.all_traits() {
            if let Some(fn_ty) = self
                .impl_registry
                .lookup_trait_method(&trait_name, receiver_key, method_name)
                .cloned()
            {
                let instantiated = self.instantiate_method_function_ty(&fn_ty, &HashMap::new());
                candidates.push(MethodCandidate {
                    label: trait_name,
                    param_count: instantiated.param_types.len(),
                    value: instantiated,
                });
            }
        }

        match select_method_candidate(candidates, arg_count) {
            MethodCandidateMatch::None => Ok(None),
            MethodCandidateMatch::WrongArity { expected } => {
                Err(TypeckError::ArgumentCountMismatch {
                    expected,
                    found: arg_count,
                })
            }
            MethodCandidateMatch::One(fn_ty) => Ok(Some(fn_ty)),
            MethodCandidateMatch::Ambiguous { labels } => Err(TypeckError::Other(
                ambiguous_method_error(method_name, receiver_key, &labels),
            )),
        }
    }

}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::TypeChecker;
    use crate::typeck::ty::{IntKind, Ty, TyKind};

    fn mk(id: usize, kind: TyKind) -> Ty {
        Ty::new(id, kind)
    }

    #[test]
    fn ty_contains_future_escape_rejects_ref_wrapped_future() {
        let future = mk(1, TyKind::Future(Box::new(mk(2, TyKind::Int(IntKind::I64)))));
        let wrapped = mk(3, TyKind::Ref(false, Box::new(future)));
        assert!(TypeChecker::ty_contains_future_escape(&wrapped));
    }

    #[test]
    fn ty_contains_future_escape_rejects_ptr_wrapped_future() {
        let future = mk(1, TyKind::Future(Box::new(mk(2, TyKind::Int(IntKind::I64)))));
        let wrapped = mk(3, TyKind::Ptr(Box::new(future)));
        assert!(TypeChecker::ty_contains_future_escape(&wrapped));
    }

    #[test]
    fn ty_contains_future_escape_rejects_fn_returning_future() {
        let future = mk(3, TyKind::Future(Box::new(mk(4, TyKind::Int(IntKind::I64)))));
        let fn_ty = mk(
            1,
            TyKind::Fn {
                params: vec![mk(2, TyKind::Int(IntKind::I32))],
                ret: Box::new(future),
                is_variadic: false,
            },
        );
        assert!(TypeChecker::ty_contains_future_escape(&fn_ty));
    }
}



