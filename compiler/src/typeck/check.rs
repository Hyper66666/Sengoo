//! 缂傚倸鍊搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倐鍋撻柣锝忕節楠炲繘寮埀顒勬儎椤栫偛鏄ラ柍褜鍓氶妵鍕箳閹存繍浠奸梺钘夊暟閸犳牠寮婚弴鐔风窞闁割偅绻傛慨鏇㈡⒑缂佹缂氶柛濠冪箞楠炲啫螖閸涱厽宓嶅銈嗘尵閸嬫﹢骞嬪畡鎷旀棃鎮╅棃娑楃捕濠电偛妯婇崣鍐嚕椤愩埄鍚嬮柛娑卞灡濞堟洟姊洪崨濠傚Е濞存粏娉涘嵄鐟滅増甯楅悡鐔兼煟濡搫鏆卞┑顔ㄥ懐纾奸棅顐幘閻瑦銇勯姀鈥冲摵闁诡喛娅ｇ划鍫ャ€佸ú顏呪拺闁告繂瀚峰Σ鎾煛閸涱喚鐭岀紒顔界懇瀵粙顢樺┃鎯т壕濞撴埃鍋撴鐐差儔閹晠妫冨☉娆愵唫闂備浇宕垫慨鏉懨洪埡鍐х剨婵炲棙鎸搁悞鍨亜閹烘垵顏柡浣介哺閵囧嫰濡搁敐鍛闂佷紮绲剧换鍫濈暦濮椻偓椤㈡棃宕担鍦Ь闂傚倸鍊风欢姘焽瑜嶈灋闁哄啫鐗婇崐鍧楁煥閺冨倸浜鹃柡鍡樼矌閹叉悂鎮ч崼婵堢懆濠碘槅鍋呰摫闁靛洤瀚伴獮妯兼崉鏉炵増鍕冮梺璇插娣囪櫣浜稿▎鎴烆潟闁圭儤顨呯粻濠氭煕濞戞鎽犳い搴＄Т閳规垿鎮欑喊妯诲珱闂佺顑呯€氼垶鎮樼€ｎ喗鈷戦梺顐ゅ仜閼活垱鏅剁€电硶鍋撳▓鍨灈妞ゎ參鏀辨穱濠囨倻閼恒儲娅滈梺绋挎湰閻喗绔?
//! 闂傚倷娴囬褎顨ョ粙鍖¤€块梺顒€绉寸壕濠氭煟閺冨洤浜圭€规挷绶氶弻娑㈠Ψ閿濆懎顬嬪銈傛櫆閻擄繝寮诲☉妯锋斀闁割偅绻勯埀顒傛儮goo闂傚倷娴囧畷鍨叏閺夋嚚褰掑磼閻愯尙鐓戦梺閫炲苯澧撮柡灞界Ф閹风娀鎳犻鈧埅鐢告⒑鐠団€崇仩闁哥喐娼欓悾鐑芥偄绾拌鲸鏅ｉ梺瀹犳濡瑩鎷烘径鎰拻濞达綀顫夐崑鐘绘煕閺傝法鐒告い銏＄墵瀹曞爼顢楁担鍝勫Ъ闂佽閰ｅ褔骞楀鍛棜濠靛倸鎲￠悡鐔兼煛閸モ晛浠滈柍褜鍓濆畷鐢靛垝閸喎绶為柟閭﹀幘閸橀亶姊洪崫鍕檨闁告洦鍋呴锟犳⒑鐠囨彃顒㈤柛鎴濈秺瀹曘垺绺界粙璺ㄥ幒闂佸搫鍊哥花鍗炍ｉ崼鐔剁箚妞ゆ牗姘ㄥВ鐐烘煕鐎ｎ偅宕岄柟顔界矒閹稿﹥绔熼埞鍨姎闁宠棄顦靛顕€宕掑鎰ait缂傚倸鍊搁崐鐑芥倿閿曞倸绠繝闈涱儐閳锋棃鏌涢弴銊ヤ航闁绘帟濮ら妵鍕箛閸撲胶鏆犻弶鈺傜箖缁绘稒娼忛崜褏袦缂傚倸鍊瑰畝鎼佹晲閻愮儤鏅濋柛灞剧〒閸橀亶姊洪棃娑辩劸闁稿孩妞藉畷鏇㈠箻鐎靛摜顔曢梺鍝勵槹閸ㄥ爼藟閵忊槅娈介柣鎰綑閻忔潙鈹戦鐟颁壕闂備線娼х换鍫ュ垂濞差亜绠氶柨婵嗩槹閳锋垿鏌涘┑鍡楊伀閼叉牕顪冮妶搴″箹闁绘绻掔槐鎾诲箻濞ｎ剙浜濋梺鍛婂姂閸斿酣藝椤曗偓濮婅櫣绱掑Ο娲绘⒖濡炪倖娉﹂崶褍鍋嶉梺鍦檸閸犳鎮?
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
mod trait_impl_helpers;

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

/// 缂傚倸鍊搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倐鍋撻柣锝忕節楠炲繘寮埀顒勬儎椤栫偛鏄ラ柍褜鍓氶妵鍕箳閹存繍浠奸梺钘夊暟閸犳牠寮婚弴鐔风窞闁割偅绻傛慨鏇㈡⒑缂佹缂氶柛濠冪箞瀵濡搁妷銏☆潔濠碘槅鍨甸崑鎰板礉椤斿墽纾藉ù锝嗗絻娴滈箖鏌ｆ惔顖滅У闁告挻姘ㄩ埀顒佽壘椤兘寮婚妸銉㈡斀闁糕剝锚椤庢盯姊洪幖鐐测偓妤呭吹閸戠ǖ闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傛噽閻瑩鏌″搴″箲闁逞屽厸缁舵岸鐛€ｎ喗鍋愰梻鍫熺⊕濞堫偊姊洪懡銈呅㈡繛娴嬫櫇娴滅鈻庨幘宕囧姦濡炪倖甯掔€氼參寮潏銊ｄ簻闁靛绲介崝姘舵煟閿濆棛绠炵€规洘锕㈤、鏃堝椽娴ｅ湱绉鹃梻鍌氬€风欢姘焽瑜嶈灋闁哄啫鐗婇崐鍧楁煥閺冨倸浜鹃柡鍡樼矌閹叉悂鎮ч崼婵堢懆濠碘槅鍋呰摫闁靛洤瀚伴獮妯兼崉鏉炵増鍕冮梺璇插娣囪櫣浜稿▎鎴烆潟闁圭儤顨呯粻濠氭煕濞戞鎽犳い搴＄Т閳规垿鎮欑喊妯诲珱闂佺顑呯€氼垶鎮樼€ｎ喗鈷戦梺顐ゅ仜閼活垱鏅剁€电硶鍋撳▓鍨灈妞ゎ參鏀辨穱濠囨倻閼恒儲娅滈梺绋挎湰閻喗绔?
pub struct TypeChecker {
    /// 缂傚倸鍊搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倕鎳忛崐濠氭煠閹帒鍔氬ù鐘灩椤啴濡堕崱娆忣潷闂佸憡鍨电紞濠傤嚕閹惰棄绫嶉柛顐ゅ枔閸欏棗鈹戦悙鏉戠仸妞ゎ厼鐗忛埀顒佺煯閸楁娊寮婚敐鍡楃疇闂佸憡姊归崹鐢告偩閻戣棄绠虫俊銈傚亾閹喖姊洪棃娑辨Ф闁稿骸纾划鏃堫敋閳ь剙顫忕紒妯诲闁荤喐澹嗙粊椋庣磽娴ｈ櫣甯涢柛鏃€鐟╅悰顔跨疀濞戞顓煎銈嗘⒐閸庢娊寮冲Δ鍛厽闁绘ê寮剁粚鍧楁倶韫囨梻鎳囬柛鈹惧亾濡炪倖甯掗崐鎼佸储閹绢喗鐓欐い鏍ㄨ壘閺嗭絿鈧娲滈崰鏍€佸鈧幃鈺呭礃瀹割喕妲愰梻鍌欐祰椤曆冾潩閿曞偊缍栧鑸靛姈閸ゅ苯螖閿濆懎鏆欓柣顓燁殘閹叉瓕绠涘☉妯碱槺闂佸搫绋侀悘婵婎樄鐎规洖鐖奸弫鎰板川椤掆偓椤?
    env: TypeEnv,
    /// 缂傚倸鍊搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倕鎳忛崐濠氭煢濡警妲烘い鏂匡躬濮婃椽妫冨☉杈╁彋闂傚倸瀚€氫即骞冮悙瀵割浄閻庯綆鍋嗛崢閬嶆⒑閹稿海绠撴繛瀵稿厴瀹曘垹顭ㄩ崨顖滐紲闁哄鐗勯崝搴ｇ不閻愮儤鐓欐い鏃囧吹閻瑥鈹戦埄鍐╁€愬┑鈥崇埣瀹曞爼顢旈崟鍨棨缂傚倸鍊搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倕鎳忛崐濠氭煢濡警妲洪柣鎾村灥閳规垿顢欐慨鎰捕闂佺顑嗛幑鍥蓟濞戙垺鏅查柛鈩兦滄禒銏ゆ⒑鐠団€崇仩闁哥喐娼欓悾鐑芥偄绾拌鲸鏅㈤梺閫炲苯澧撮柟铏懆閵囨劙骞掗幘璺哄箰闂備胶顭堢悮顐﹀磹閺囥垻宓侀柡宥庡幗閻撴瑩姊洪崹顕呭剰妞ゃ儱顑囩槐鎺撴綇閵娿儲璇為梺璇″枔閸ㄨ棄鐣峰鍕懝闁搞儻濡囩粈鍐⒒?
    infer: TypeInfer,
    /// Trait婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵螣濮瑰洣绨诲銈嗘尵閸犲海绮缁辨帡鍩€椤掍礁绶為柟閭﹀幖閳ь剙鐏氱换娑㈠箣閻戝棔鐥銈呯箰閻楀棛澹曢懡銈傚亾楠炲灝鍔氭い锔诲灣缁濡烽埡鍌滃帗閻熸粍绮撳畷婊堟偄婵傚娈ㄩ梺鍓茬厛閸嬪嫮娆㈤悙娴嬫斀闁绘ɑ褰冮埀顒傤焾閳绘捇濡烽敂鍓х槇闂侀潧绻掓慨鐢垫嫻閳ユ剚鐔嗙憸搴ㄣ€冮崨顓炵カ闂備胶顢婇幓顏堟⒔閸曨垰纾跨€广儱顦伴悡鏇㈡煙閹规劖鐝い搴㈢崒it濠电姷鏁搁崕鎴犲緤閽樺娲偐鐠囪尙顦┑鐘绘涧濞层倝顢氶柆宥嗙厱婵炴垶鐟︾紞鎴炴交?
    trait_registry: TraitRegistry,
    /// Impl婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵螣濮瑰洣绨诲銈嗘尵閸犲海绮缁辨帡鍩€椤掍礁绶為柟閭﹀幖閳ь剙鐏氱换娑㈠箣閻戝棔鐥銈呯箰閻楀棛澹曢懡銈傚亾楠炲灝鍔氭い锔诲灣缁濡烽埡鍌滃帗閻熸粍绮撳畷婊堟偄婵傚娈ㄩ梺鍓茬厛閸嬪棛绮诲▎鎴犵＜闁告鍋涢崝鏉媡闂傚倷娴囬褎顨ョ粙鍖¤€块梺顒€绉寸壕濠氭煟閺冨洤浜圭€规挷绶氶弻娑㈠Ψ椤旂厧顫梺绋块缁夊綊寮诲☉銏犲嵆闁靛鍊楅懗铏圭磽娴ｇ瓔鍤欐俊顐ｇ箞瀵鎮㈢喊杈ㄦ櫍閻熸粌绉归幃妯衡槈閵忥紕鍘?
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

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滈崘鈧銈嗗姂閸婃洟寮冲Δ鍛厽闁绘ê寮剁粚鍧楁倶韫囨梻鎳囬柛鈹惧亾濡炪倖甯掗崐鎼佸储閹绢喗鐓欐い鏃囨閳绘洘銇勯姀锛勨槈妞ゎ偅绻堝畷妤呭礂绾板崬鎮╅梻鍌氬€烽悞锕傛儑瑜版帒绀夌€光偓閳ь剟鍩€椤掍礁鍤ù婊呭仧閸掓帡顢橀悢椋庣獮闂佸綊鍋婇崜娑㈡偂閹存繍娓婚柕鍫濇婢ь剟鏌ら悷鏉库挃缂佽京鍋為幏鍛偘閳ュ厖澹曞┑鐐茬墕閻忔繈寮搁悢鍝ョ闁告瑥顦遍惌鎺斺偓娈垮櫘閸嬪﹥淇婇懜闈涚窞濠?
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滈崘鈧銈嗗姂閸婃洟寮冲Δ鍛厽闁绘ê寮剁粚鍧楁倶韫囨梻鎳囬柛鈹惧亾濡炪倖甯掗崐鎼佸储鐎涙﹩娈介柣鎰綑濞搭喖鈹戦埄鍐╁€愰柡浣稿€块幃鍓т沪閽樺－銊╂⒒閸屾瑧绐旀繛鑹板吹瀵板﹥绻濆顒傤槷闁硅偐琛ュ褔寮搁弮鍫熺厵閻庣數顭堟禒褔鏌嶇紒妯烩拻闁逞屽墮缁犲秹宕曢柆宥呯疇閹艰揪绲炬刊濂告煛鐏炶鍔滈柣鎾存礃缁绘盯宕卞Ο鍝勫付濡炪倕绻愬Λ娆擄綖閺囥垺鐓冮柛婵嗗閸ｆ椽鏌涚€ｃ劌濮傞柡灞炬礃缁绘繆绠涢弴鐐电厳闂?
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滅痪褎銇勯敐鍡╂祲it婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵顫滈埀顒勫蓟閺囩喓绠剧憸宥団偓姘煎櫍瀵娊鎮╁畷鍥╊啎闂佺懓顕崑鐐典焊椤撶姷纾煎璺虹焾濡酣鏌曢崶銊ュ鐎殿喖顭锋俊鐑藉Ψ瑜忛崢顒佺節绾版ɑ顫婇柛銊╂涧閳诲秹鏁愰崶鈺冪厰闂佺鍕垫畷闁绘挾鍠愰妵鍕敃椤愩垹顫╂繝纰樺墲閹倿寮?
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滅痪褍鈹戦娴虫獟婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵顫滈埀顒勫蓟閺囩喓绠剧憸宥団偓姘煎櫍瀵娊鎮╁畷鍥╊啎闂佺懓顕崑鐐典焊椤撶姷纾煎璺虹焾濡酣鏌曢崶銊ュ鐎殿喖顭锋俊鐑藉Ψ瑜忛崢顒佺節绾版ɑ顫婇柛銊╂涧閳诲秹鏁愰崶鈺冪厰闂佺鍕垫畷闁绘挾鍠愰妵鍕敃椤愩垹顫╂繝纰樺墲閹倿寮?
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滅痪褎銇勯敐鍡╂祲it婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵顫滈埀顒勫蓟閺囩喓绠剧憸宥団偓姘煎櫍瀵娊鎮╃紒妯锋嫼闂佸憡绋戦敃锕傚箠閹扮増鐓曢柕濠忕畱椤庢粓鏌曢崶銊ュ鐎垫澘瀚换婵囨償閳╁喚鍚欓梻鍌欑閹碱偄煤閵婏附鍙忛柣銏㈩焾濮规煡鐓崶銊р姇闁?
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// 闂傚倷绀侀幖顐λ囬锕€鐤炬繝濠傜墕閽冪喖鏌曟繛鍨壄婵炲樊浜滅痪褍鈹戦娴虫獟婵犵數濮烽弫鎼佸磻濞戔懞鍥敇閵忕姷顦悗鍏夊亾闁告洦鍋夐崺鐐烘⒑娴兼瑧鍒伴柡鍫墴瀹曟垵顫滈埀顒勫蓟閺囩喓绠剧憸宥団偓姘煎櫍瀵娊鎮╃紒妯锋嫼闂佸憡绋戦敃锕傚箠閹扮増鐓曢柕濠忕畱椤庢粓鏌曢崶銊ュ鐎垫澘瀚换婵囨償閳╁喚鍚欓梻鍌欑閹碱偄煤閵婏附鍙忛柣銏㈩焾濮规煡鐓崶銊р姇闁?
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// 闂傚倷娴囬褏鑺遍懖鈺佺筏濠电姵鐔紞鏍ь熆閼搁潧濮囨慨瑙勭叀閺屻劌鈹戦崱姗嗘￥闂佸磭绮褰掑Φ閸曨喚鐤€闁圭偓鎯屽Λ蹇涙⒑閸濆嫯顫﹂柛濠冪箞閻涱噣寮介鐐甸獓闂佺懓顕慨鐑筋敊閹寸姷纾藉ù锝堟鐢稒銇勯鐐村枠妤犵偛鍟村鍓佹崉閵婏附鐣烽梻渚€鈧偛鑻晶浼存煙椤曞懎娅嶆い銏℃礋閺佸啴鍩€椤掑嫬鍨傞柛宀€鍋為崐鐢告煥濠靛棝顎楀ù婊嗗Г閹便劍绻濋崶鈺冩毇闂佸搫鐭夌槐鏇熺閿旂偓瀚氶柟缁樺笒鍟哥紓鍌氬€峰ù鍥ㄣ仈閸濄儲宕查柛顐ｇ箘閺嗭箓鏌ｅΟ娆惧殭鐎瑰憡绻冮妵鍕籍閸屾瀚涙繛瀵稿Х閸嬫挾鎹㈠┑瀣潊闁绘ê鐤囩涵鈧┑鐘媰閸曞灚鐣堕柛妤呬憾閺岀喓鈧數顭堥崜宕囩磼閹插绉柡灞剧洴瀵挳濡搁妷銈堝焻婵犵鍓濋悡鈩冪閸洖钃熼柨婵嗩槹閸婄兘鏌涘▎蹇ｆ▓婵☆偆鍋ゅ铏规兜閸涱喛鍚傞梺鍛婎殔閸熷潡鎮惧畡鎷旀棃宕ㄩ鍥ｆ櫊閺屽秵娼幍顔煎濡炪倕绻嗛埀顒佹灱閺€浠嬫煃閽樺顥滃ù婊勫姍閺屾稓鈧綆鍋呭畷宀勬煛瀹€瀣？濞寸媴濡囬幏鐘诲箵閹烘埈娼欓梻?
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

    /// 濠电姷顣藉Σ鍛村磻閸涱収鐔嗘俊顖氱毞閸嬫挸顫濋悡搴♀拫闂佽桨绀佺粔褰掔嵁閸ヮ剙绾ч柛顭戝枛閹搞倝姊洪悷鏉挎倯闁伙綆浜畷婵囩節閸パ呭姦濡炪倖宸婚崑鎾绘煥閺囶亪妾紒鍌涘浮閺佸啴宕掑鎲嬬床婵犳鍠楅敋濠⒀傜矙閹苯螖閸涱喒鎷洪梺鍛婄箓鐎氼剟顢旈埡鍛厱闁瑰濮靛▍鏇熶繆閸欏濮囬柍璇查叄楠炴ê鐣烽崶鑸敌熼梻鍌欑閸氬绂嶆禒瀣？闁圭粯宸婚弸鏃堟煕椤愶絾绀冮柣鎾崇箻閺屾盯鍩勯崘鈺冾槷闂佺绻愰惉鑲╂閹烘鏁婇柛婵嗗椤洭鎮楀▓鍨灍闁诡喖鍊搁悾鐑芥偄绾拌鲸鏅滈梺绯曞墲椤忕兘顢旈崼鐔叉嫼闂傚倸鐗婄粙鎺椝夊▎鎾寸厽闁硅櫣鍋熼悾鐢告煙椤旀枻鑰跨€规洖鐖兼俊姝岊槷闁哄鐗犲娲川婵犲啫鐦烽梺鍛婁緱閸犳宕曢鐐粹拻濞达絿鎳撻婊呯磼鐎ｎ偄鐏寸弧鎾绘煃瑜滈崜娑氭閹烘挻缍囬柕濞у懐鏆┑鐐茬摠缁秹骞冮崒姘辨殾闁告鍊ｉ弮鍫濈劦妞ゆ巻鍋撴い鏇樺劦楠炴﹢宕滄担鐚寸床婵＄偑鍊栭崝鎴﹀磿濞差亜鍚规繛鍡樻尰閻撶喖骞栧ǎ顒€濡介柡浣戒含缁辨帒鐣濋崟顏呭枤闂佺硶鏅涚€氭澘鐣峰Ο娆炬Ь缂備讲鍋撻柍褜鍓氱换婵嬫偨闂堟稐绮跺銈忓瘜閸欏啫鐣峰┑鍡欐殕闁告洦鍋夐崺鐐寸箾鐎电孝妞ゆ垵鎳愮划鍫ュ幢濞戞瑧鍘靛┑鐐茬墕閻忔繈寮稿☉姘辩＜闁绘ê鐤囨竟妯汇亜椤撯€冲姷妞わ附鐓￠弻娑㈡偄閸欏鐝氶梺纭呮珪閻熲晠鐛€ｎ喗鏅濋柍褜鍓濈换?
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

                // 濠电姷鏁告慨浼村垂閻撳簶鏋栨繛鎴炲焹閸嬫挸顫濋悡搴㈢彎濡ょ姷鍋涢崯顖滄崲濠靛纾兼俊銈勮兌娴滄瑦绻濋悽闈涗沪闁搞劌鐖奸垾锕傚炊椤掆偓閻掑灚銇勯幒鎴濃偓鎼佸储鐎电硶鍋撶憴鍕闁荤啿鏅犻獮濠囨倷閸濆嫀銊╂煥閺冨倻鎽傞柛鐔插亾闂傚倸鍊风粈渚€骞夐敓鐘冲仭闁靛鏅涚壕鍦喐閻楀牆绗掓慨瑙勭叀閺岋綁寮崶顭戜哗闁诲繐绻掗弫濠氬箖瑜版帒鐐婃い蹇撳濮ｃ垽姊洪柅鐐茶嫰婢у瓨銇勯妷锔藉暗婵″弶鍔欓獮鎺懳旈埀顒傜不閿濆棛绡€闂傚牊绋撴晶娑欐叏閿濆懘鍙勬慨濠冩そ楠炴劖鎯旈敐鍌涱潔闂備焦妞块崢娲嚄閸洖绠查柕蹇曞閻旂厧绀傞柣鎾崇凹缁囨煟鎼粹€冲辅闁稿鎹囬弻宥堫檨闁告挻鐩獮鍐敋閳ь剟銆侀弴銏℃櫇闁逞屽墴瀹曞綊宕掗悙鏉戔偓鐢告煥濠靛棝顎楀ù婊嗗Г閹便劍绻濋崶鈺冩毇闂佸搫鐭夌槐鏇熺閿旂偓瀚氶柟缁樺笒椤垿姊?
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
                    // 婵犵數濮烽弫鎼佸磻濞戔懞鍥箥椤斿墽鐓撻梺纭呮彧闂勫嫰宕愰悽鍛婄厽闁硅揪绲鹃ˉ澶愭煟椤撶噥娈滈柡灞剧〒娴狅箓宕滆婵洤鈹戦悙鑼濠⒀冩捣濡叉劙骞樼€涙ê顎撻梺鑽ゅ枑婢瑰棙绂掗幘顔解拺濞村吋鐟ч幃濂告煕鐎ｎ偅灏い鏇樺劦瀹曠喖顢楁担铏剐ゆ俊鐐€栭崝鎴﹀磿濞戙垹鍗抽柕蹇ョ磿閸樻悂姊洪崨濠傚Е闁告鏅☉鐢告倷椤掑倻顔曢梺鍛婄懃椤﹁鲸鏅堕悽鍛婄厸閻忕偠濮ら崵鍥煕閳规儳浜炬俊鐐€栫敮濠勭矆娴ｇ儤顐介柣鎰劋閻撴洟鏌￠崶銉ュ妤犵偞顨婇弻娑欑節閸曨偂妲愰梺?
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

    /// 闂傚倷娴囬褏鎹㈤幇顔藉床闁归偊鍎靛☉姗嗙叆闁割偆鍠庨崜褰掓⒑鐠団€崇€婚悘鐐跺Г椤斿倿姊绘担鍛婂暈婵炲弶鐗楅弲鑸垫償閿濆洨鐒奸梺鍛婂灩濞存硟h闂傚倸鍊烽悞锔锯偓绗涘懐鐭欓柟娆¤娲ㄩ埀顒€婀辨刊顓㈠汲娴煎瓨鈷掑ù锝呮啞閹牊銇勯幋婵囶棦鐎规洘绻堥獮鎺楀籍閳ь剛鈧艾顭烽弻锝夊籍閸ャ儮鍋撻埀顒勬煕鐎ｎ偅灏柍钘夘槸閳诲骸螣閼姐倖姣夊┑锛勫亼閸婃牠宕归幎鑺ュ€块柨鏂垮⒔閻瑥顭块懜闈涘缂佺姷鏁婚弻鐔兼倻濡纰嶉梺閫炲苯澧痪缁㈠幘濡叉劙骞掗幘宕囩獮闁硅壈鎻槐鏇㈠礉閻戣姤鈷?
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

    /// 闂傚倷娴囬褏鎹㈤幇顔藉床闁圭増婢樼粻鐗堢箾閿濆洤鐦剁紓鍌氬€搁崐椋庢閿熺姴纾诲鑸靛姦閺佸鎲搁弮鍫濈畺婵°倕鎳忛崐濠氭煠閹帒鍔氭繛鍫㈠枛濮婅櫣绮欓幐搴㈡嫳缂備緡鍠栭張顒傜矉閹烘鏁嶉柣鎰皺妤犲洭姊虹紒妯虹仴婵☆偅顨嗛弲鍫曨敍濠ф儳浜鹃悷娆忓缁€鈧悗娈垮枛婢у海鍒掑▎鎰窞闁归偊鍏涚槐鍫曟⒑閸涘﹥澶勯柛瀣嚇瀵娊濡烽埡鍌楁嫽婵炴挻鑹惧ú銈夊几閺冨牊鐓曢柡鍌涘閹癸綁鏌熼鍛珝妞ゃ垺娲熼弫鍐焵椤掑嫬鍨傞柛宀€鍋為崐鐢告煥濠靛棝顎楁鐐村灴閺屽秷顧侀柛鎾寸懇瀹曨垶骞嶉鐓庣亰闂佸壊鍋侀崕鏌ュ磻閺嶎厽鍊甸梻鍫熺⊕閹插憡銇勯敂鍝勭稻y闂傚倸鍊烽悞锔锯偓绗涘懐鐭欓柟娆¤娲、姗€濮€閻橀潧濮?
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

    /// 婵犵數濮烽。钘壩ｉ崨鏉戠；闁逞屽墴閺屾稓鈧綆鍋呭畷宀勬煛瀹€瀣？濞寸媴濡囬幏鐘诲箵閹烘埈娼ラ梻鍌欑閹碱偊鎯夋總绋跨獥閹兼番鍔岄悡婵嬫煛閸愩劎澧曢柛妤佸▕閺屻劌鈽夊Ο渚痪濡炪倖鏌ㄧ粔鐟邦潖閸濆娊铏圭磼濡　鏋忛梻浣告啞閺屻劎鎹㈠Ο璁崇箚闁圭虎鍠栫粈鍐煃閸濆嫬鈧崵绮旈崼鏇熲拺閻犲洠鈧磭鈧鏌涘☉鍗炵伇闁衡偓閵娾晜鈷掑ù锝囨嚀椤曟粎绱掔€ｎ偄鐏撮柟顖氭湰閹峰懘鎮滃Ο鍝勭哎婵犵妲呴崹浼村触鐎ｎ喖纾绘慨妞诲亾闁诡喗锕㈤幃娆撴濞戞顥氶梻浣虹帛閹稿鎮烽敂鐐床婵炴垶鐟ョ欢鐐烘倵閿濆骸浜炴い锔诲櫍閹泛顫濋鐘冲櫚闂佸搫鐭夌槐鏇㈠焵椤掑﹦绉甸柛瀣閹便劑宕堕浣哄幍闂佺偓鑹鹃崐绋跨暤閸℃瑢鍋撶憴鍕┛缂傚秳绀侀悾鐑藉箳閹存梹顫嶅┑鐐叉閸旓箓宕埀顒傜磽閸屾艾鈧娆㈤敓鐘茬；濠㈣埖鍔﹂弫瀣喐閺冨牆绠栨俊銈傚亾闁伙綇绻濋獮蹇涘籍閳ь剟鎯勯鐐叉槬闁逞屽墯閵囧嫰骞掗幋婵愪患闂佽棄鍟伴崰鏍蓟閺囩喎绶炴繛鎴炶壘閻繈姊?
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



