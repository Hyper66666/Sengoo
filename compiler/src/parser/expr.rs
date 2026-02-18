//! 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾绾惧鏌ｉ幇顔芥毄闁活厽鐟╅悡顐﹀炊閵娧€妲堢紓浣插亾濠㈣埖鍔楅崣鎾绘煕閵夛絽濡块柍钘夘槺缁辨帡鎮╅懠顑跨驳闂侀潧娲ょ€氼垳绮诲☉銏犵闁归妞掔槐婵嬫⒒娴ｈ鍋犻柛濠冪墪鐓ら柣鏂垮悑閸嬪倹銇勯幇鍓佺暠闁绘劕锕ラ妵鍕敇閻旈浠村┑顕嗙岛閸嬫捇姊婚崒娆戭槮婵犫偓闁秴纾块柕鍫濐槶閳ь剙鍟村畷銊╁级閹寸媭妲?
use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;
use miette::SourceSpan;

use super::Parser;

/// 闂傚倸鍊搁崐椋庣矆娓氣偓楠炴牠顢曚綅閸ヮ剦鏁冮柨鏇楀亾闁汇倗鍋撶换婵囩節閸屾粌顣虹紓浣插亾濠㈣泛顑勭换鍡涙煏閸繃鍣洪柛锝嗘そ閺屾稓鈧急鍕彋闂佸搫琚崐鏍箞閵娾晛绠涙い鎴ｆ娴滅偓淇婇妶鍛殶濞戞挸绉归弻锟犲礃閵娧冾杸闂佺粯鎸堕崕鐢稿蓟閿熺姴绀冮柕濠忕畱椤︹晝绱掔紒銏犲箹闁绘牕銈稿璇测槈閵忊€充汗缂備焦绋撻、濠囧焵椤掆偓濞硷繝寮婚敍鍕勃闁告挆宀€浼囩紓鍌欒兌婵敻鎯勯鐐靛祦婵☆垰鍚嬪畷澶愭偠濞戞巻鍋撻崘鑼▏婵?
const PREC_ASSIGN: u8 = 1;
const PREC_OR: u8 = 2;
const PREC_AND: u8 = 3;
const PREC_COMPARE: u8 = 4;
const PREC_BIT_OR: u8 = 5;
const PREC_BIT_XOR: u8 = 6;
const PREC_BIT_AND: u8 = 7;
const PREC_SHIFT: u8 = 8;
const PREC_ADD: u8 = 9;
const PREC_MUL: u8 = 10;
const PREC_UNARY: u8 = 11;
const PREC_CALL: u8 = 12;

impl<'source> Parser<'source> {
    pub(super) fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_expr_prec(0)
    }

    fn parse_simple_expr(&mut self) -> Result<Expr> {
        self.parse_simple_expr_prec(0)
    }

    fn parse_simple_expr_prec(&mut self, precedence: u8) -> Result<Expr> {
        let mut left = self.parse_simple_prefix()?;

        loop {
            let token = self.current().cloned();
            let next_prec = match &token {
                Some(t) => self.get_infix_precedence(&t.kind),
                None => 0,
            };

            if next_prec <= precedence {
                break;
            }

            left = self.parse_infix(left, next_prec)?;
        }

        Ok(left)
    }

    fn parse_simple_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                TokenKind::Int(Some(n)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Int(*n))
                }
                TokenKind::Float(Some(f)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Float(*f))
                }
                TokenKind::String(Some(s)) => {
                    self.advance();
                    ExprKind::Literal(Literal::String(s.clone()))
                }
                TokenKind::Char(Some(c)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Char(*c))
                }
                TokenKind::TrueKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(true))
                }
                TokenKind::FalseKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(false))
                }
                TokenKind::NullKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Null)
                }
                TokenKind::Not => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Minus => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitAnd => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Ref,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Star => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Deref,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_simple_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    ExprKind::Path(path)
                }
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    self.advance();
                    ExprKind::Ident(self.intern_ident(span))
                }
                _ => {
                    return Err(CompileError::ParseError(
                        ParseError::unexpected_token_in_expression(),
                    ));
                }
            },
            None => {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 濠电姷鏁告慨鐑藉极閹间礁纾婚柣妯款嚙缁犲灚銇勮箛鎾搭棤缂佲偓婵犲洦鐓冪憸婊堝礈濮樿鲸宕叉繛鎴炵懃缁剁偤鎮楅敐搴′簽妞わ缚鍗抽幃妤€鈻撻崹顔界彯闂佺顑呴敃銉︾┍婵犲洤閱囬柡鍥╁仜閼板灝鈹戞幊閸婃洟鏁冮妶鍛灁闁稿繗鍋愮弧鈧紒鍓у钃辨い顐躬閺屾盯濡搁妶鍛ギ濡ょ姷鍋為悧鐘汇€侀弴銏犖ч柛鈩兩戦鍥⒒娓氣偓濞佳囨晬韫囨稑宸濇い鏍ュ€楁惔濠囨⒒閸屾瑨鍏岀紒顕呭灦閹兘鏁冮崒娑樹画闂侀潧顦弲娑㈡嫅閻斿皝鏀介柣妯哄级閹兼劗绱掗悩鐑樼彧濞ｅ洤锕俊鍫曞磼濮橆偄顥氶梻鍌欑閹诧繝宕圭憴鍕闁逞屽墴閺屸€崇暆閳ь剟宕伴弽顓炶摕闁搞儺鍓氶弲婵嬫煃瑜滈崜鐔煎箖濡　妲堥柕蹇ョ磿閸樻捇鎮峰鍕煉鐎规洘绮岄埢搴ㄥ箻瀹曞洦鐒炬俊鐐€栭悧妤呭礄瑜版帗鍊垮ù鐘差儑閸欐捇鏌涢妷锝呭闁宠棄顦辩槐鎺楁偐閾忣偆娈ら梺鐟板级閻℃洜绮诲☉銏犲嵆闁绘柨鍢查獮鎴︽⒒?
    fn parse_expr_prec(&mut self, precedence: u8) -> Result<Expr> {
        let mut left = self.parse_prefix()?;

        loop {
            let token = self.current().cloned();
            let next_prec = match &token {
                Some(t) => self.get_infix_precedence(&t.kind),
                None => 0,
            };

            if next_prec <= precedence {
                break;
            }

            left = self.parse_infix(left, next_prec)?;
        }

        Ok(left)
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴娲﹀☉褔妫佹径鎰厽婵☆垳鍎ら埢鏇㈡煕鎼达紕绠崇紒杈ㄥ浮閸┾偓妞ゆ帒瀚柋鍥煃閸ㄦ稒娅呭ù婊呭亾椤ㄣ儵鎮欓懠顑胯檸闂佸憡姊圭喊宥囨崲濞戞矮娌柛灞捐壘椤绻濋姀锝庢綈婵炲弶顭囬幑銏犫攽閸♀晜鍍靛銈嗘尵閸嬫﹢宕Δ浣虹瘈闁汇垽娼ф禒锕傛煕閵娿儳鍩ｉ柟顔ㄥ洤鍗抽柣鏃傤焾瀵潡姊洪柅鐐茶嫰婢у瓨鎱ㄦ繝鍛仩闁瑰弶鎸冲畷鐔碱敄閸欍儲鍩涙繝鐢靛仜閻°劎鍒掑鍥у灊闁圭偓鐪归埀?
    fn parse_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                TokenKind::Int(Some(n)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Int(*n))
                }
                TokenKind::Float(Some(f)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Float(*f))
                }
                TokenKind::String(Some(s)) => {
                    self.advance();
                    ExprKind::Literal(Literal::String(s.clone()))
                }
                TokenKind::Char(Some(c)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Char(*c))
                }
                TokenKind::TrueKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(true))
                }
                TokenKind::FalseKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(false))
                }
                TokenKind::NullKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Null)
                }

                // 濠电姷鏁告慨鐑藉极閹间礁纾婚柣鎰惈閸ㄥ倿鏌涢锝嗙缂佺姳鍗抽弻鐔虹磼閵忕姵鐏堢紒鐐劤椤兘寮婚悢鐓庣鐟滃繒鏁☉銏＄厽闁规儳鐡ㄧ粈瀣煟閹垮啫浜扮€规洖鐖兼俊鎼佹晝閳ь剛鍠婂澶嬧拺闁告繂瀚ˉ鎾绘煕閵娿劌鍚规俊鍙夊姍楠炴帡寮埀顒傗偓姘哺閺屻倗鍠婇崡鐐插О闂侀潧鐗嗗ú銊у閸忕浜滈柡鍐ｅ亾妞ゆ垶鐟╁畷闈涒枎閹惧瓨鍤夐梺鍝勭▉閸樹粙鎮￠弴銏＄厵闂侇叏绠戦獮鏍煕閺傝鈧繈寮?
                TokenKind::Minus => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Not => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitNot => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::BitNot,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitAnd => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Ref,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Star => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Deref,
                        operand: Box::new(operand),
                    }
                }

                // 闂傚倸鍊搁崐鎼佸磹閻戣姤鍤勯柛顐ｆ礀缁犵娀鏌熼悙顒傛菇闁逞屽墮閸燁垶骞嗛弮鍫澪╅柕澹本肖濠电姷鏁搁崑娑樜涘▎鎴炴殰闁搞儯鍔庨々?
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }

                // 闂傚倸鍊搁崐鎼佸磹瀹勬噴褰掑炊瑜滃ù鏍煏婵炵偓娅嗛柛濠傛健閺屻劑寮崒娑欑彧闂佺粯绻傞悥濂稿蓟濞戙垹鐒洪柛鎰典簼閸Ｑ囨⒑?`[a, b, c]`
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }

                // 闂?`{ ... }`
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }

                TokenKind::IfKw => {
                    return self.parse_if_expr();
                }

                // while 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
                TokenKind::WhileKw => {
                    return self.parse_while_expr();
                }

                // for 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
                TokenKind::ForKw => {
                    return self.parse_for_expr();
                }

                // loop 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
                TokenKind::LoopKw => {
                    return self.parse_loop_expr();
                }

                TokenKind::MatchKw => {
                    return self.parse_match_expr();
                }

                // Lambda 闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳顭烽弻娑㈠焺閸愵亖妲堥梺绋胯閸旀垿寮婚妶鍚ゅ湱鈧綆鍋呭鎺楁⒑?`|args| expr`
                TokenKind::BitOr => {
                    return self.parse_lambda_expr();
                }

                // return
                TokenKind::ReturnKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Return(value.map(Box::new))
                }

                // break
                TokenKind::BreakKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Break(value.map(Box::new))
                }

                // continue
                TokenKind::ContinueKw => {
                    self.advance();
                    ExprKind::Continue
                }

                // yield
                TokenKind::YieldKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Yield(value.map(Box::new))
                }

                TokenKind::AsyncKw => {
                    return self.parse_async_block();
                }

                TokenKind::ParallelKw => {
                    return self.parse_parallel_block();
                }

                // 闂傚倸鍊搁崐鎼佸磹妞嬪海鐭嗗〒姘ｅ亾妤犵偞鐗犻、鏇㈠Χ閸モ晝鍘犻梻浣告惈椤︿即宕靛顑炴椽顢旈崟顓炲箰闂備礁鎲℃笟妤呭窗閺嶎厼绀堥柕濞炬櫆閳锋垿鏌ｉ悢鐓庝喊闁搞倗鍠庨埞鎴︻敋閳ь剟藟閹捐泛鍨濆┑鐘宠壘缁秹鏌涢銈呮灁闁告瑥妫濆娲传閸曨偅娈┑鐐额嚋缁犳捁妫㈡繝銏ｅ煐閸旀牠鎮￠悢鑲╁彄闁搞儯鍔嶉ˉ鏃€绻涢崼鐔虹煉闁哄瞼鍠栭、娆忊枎閻愵剛绉风紓鍌欑劍椤ㄥ牓宕伴弽顓溾偓浣糕槈濮楀棙鍍甸梺鍛婄懄閿氶柟钘夘儑缁?
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            TokenKind::LBrace => {
                                // 闂傚倸鍊搁崐鎼佸磹妞嬪孩顐芥慨姗嗗墻閻掍粙鏌ゆ慨鎰偓鏍偓姘煼閺岋綁寮崒姘粯缂備讲鍋撳璺哄閸嬫捇鐛崹顔煎闂佺懓鍢查澶愬箖閻愮儤鍤嶉柕澶涚导缁ㄥ姊洪崫鍕犻柛鏂块叄婵℃挳骞掗幘鍙樼盎闂佸啿鎼崯顐﹀储閹绢喗鐓涚€光偓鐎ｎ剛鐦堥悗瑙勬礀閵堝憡鎱ㄩ埀顒勬煃閳轰礁鏆為柣锕€閰ｅ缁樻媴閽樺鎯為梺鍝ュУ閸旀瑥鐣峰┑瀣嵆闁靛繒濮烽崣鈧┑鐘灱閸╂牠宕濋弴鐘典笉濠电姵纰嶉悡娆徫涙０浣藉厡妞わ讣绠撻弻娑㈠箻鐠佽櫕鍠氬┑顔硷攻濡炶棄鐣烽锕€绀嬮梻鍫熺☉婢瑰牓姊绘担鍛婃喐闁革絻鍎靛畷鐟扮暦閸パ冪亰濠电偛妫欓崝鏇犳閻愮儤鐓欓柡澶庢硶娴犳盯鏌￠崪鍐М婵﹥妞藉Λ鍐归妶鍡欐创鐎规洦浜濋幏鍛村礈閹邦喖澹榝/while闂傚倸鍊搁崐鎼佸磹閻戣姤鍊块柨鏃堟暜閸嬫挾绮☉妯诲櫧闁活厽鐟╅弻鐔封枎閳ュ磭婀撮柛鏃€鍨垮畷娲焵椤掍降浜滈柟鐑樺灥閳ь剝宕垫竟鏇熺附缁嬭法楠囬梺鍓插亝缁嬫垶淇婇搹鍦＜?闂傚倸鍊搁崐鎼佸磹妞嬪海鐭嗗〒姘ｅ亾妤犵偞鐗犻、鏇㈠Χ閸屾矮澹曞┑顔矫畷顒勫储鐎电硶鍋撶憴鍕缂傚秴锕ら锝囨崉鐞涒剝鐎婚梺鍛婃寙閸愩剱銊╂⒒閸屾瑧鍔嶉柡瀣偢瀵彃鈽夐姀鐘垫焾濡炪倖鐗楃粙鎴﹀垂閺冨牊鐓欑紓浣靛灩閺嬬喖鏌ｉ幘瀵告噰闁哄瞼鍠栭、娑㈠幢濡や礁鐝旂紓浣戒含閸嬬喓妲愰幒妤佸€锋い鎺嗗亾缂佲偓閸愨斂浜滈柡鍥ф濞层倝鎮″鈧弻鐔告綇閹呮В闂佽桨绀侀敃锕傛儉椤忓牆绾ч柛顭戝枦閸╃偤姊洪崨濠冪厸闁稿鎸剧槐鎾诲磼濮橆兘鍋撳畡鎳婂綊宕堕妸锝勭矒闂佸憡绺块崕鎻掔暦閺屻儲鐓曢柟鏉垮悁缁ㄤ粙鏌ｉ鐔烘噧妞ゎ叀娉曢幑鍕瑹椤栨粌褰嗛梻浣告啞閼归箖顢栨径鎰摕婵炴垯鍨归悞娲煕閹邦剙鈷旈柛鏃€鎸冲娲箰鎼淬垻鍙嗘繝鈷€鍛珪濠㈣娲熼幐濠冪珶濠靛棛绉洪柡浣瑰姍瀹曞ジ顢曢敐鍥┬ラ梻浣筋嚙缁绘劗鎹㈢€ｎ喖纭€闁规儼妫勯拑鐔兼煥濠靛棙顥為柛搴ｅ枑閵囧嫰寮崶褌姹楁繛瀵稿閸欏啫顫忓ú顏呭仭闁哄瀵т簺闂備胶顢婂▍鏇㈡晝閵忋倖鍋樻い鏂挎閻斿吋鎯為悷娆忓椤旀洟姊绘担瑙勫仩闁稿寒鍨跺畷婵嗩吋婢跺﹦鐛ラ梺鍝勬储閸ㄦ椽鍩涢幒妤佺厱閻忕偞宕樻竟姗€鏌嶈閸撴岸骞冮崒鐐茬畺鐟滄柨鐣烽悢纰辨晬婵﹩鍓涢埀顒夊幗缁绘稓鈧數顭堝瓭濡炪倖鍨甸幊妯侯潖?
                                if !self.in_condition_context {
                                    return self.parse_struct_expr(path);
                                }
                            }
                            TokenKind::LParen => {
                                return self.parse_call_or_tuple_expr(path);
                            }
                            _ => {}
                        }
                    }
                    ExprKind::Path(path)
                }

                // self 闂傚倸鍊搁崐鎼佸磹閻戣姤鍤勯柛顐ｆ磵閳ь剨绠撳畷濂稿閳ュ啿绨ラ梻浣烘嚀椤曨參宕戦悢铏逛笉闁诡垎鈧弨浠嬫煟濡搫绾ч柟鍏煎姍閺岋紕鈧絺鏅濋崝宥夋煏閸パ冾伃妤犵偞顭囬幑鍕儎閹烘挾娲撮柡灞剧〒閳ь剨缍嗛崑鍛焊椤撱垺鐓冮柦妯侯樈濡叉悂鏌嶇拠鏌ヮ€楅摶锝嗙箾閼奸鍤欓柟顔肩墢缁辨捇宕掑顑藉亾瀹勬噴褰掑炊閵婏絼绮撻梺褰掓？閻掞箓宕戦敓鐘崇厓闁告繂瀚崳鍦磼閻樺磭澧棁澶愭煟濡儤鈻曢柛搴＄箻楠炲棝鎮㈤崗灏栨嫼闂傚倸鐗婄粙鎾存櫠閺囩喓绠炬繛鏉戭儐濞呭棛绱?impl 闂傚倸鍊搁崐鎼佸磹閻戣姤鍤勯柛顐ｆ穿缂嶆牠鎮楅敐搴℃灈缂佲偓鐎ｎ喗鐓曟い顓熷灥娴滅偤鏌￠崱顓犵暤婵﹦绮幏鍛村川婵犲啫鍓甸梻浣规た閸撴瑩濡剁粙璺ㄦ殾闁绘鐗婇崕鐔兼煏婵炲灝鍔ら柛妯绘そ濮婃椽宕崟顐ｆ闂佺粯顨呭Λ婵嗙暦閿熺姵鎯炴い鎰С缁ㄥ妫呴銏″闁圭顭疯棢闁绘鍋ㄦ禍?
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    self.advance();
                    ExprKind::Ident(self.intern_ident(span))
                }

                _ => {
                    return Err(CompileError::ParseError(
                        ParseError::unexpected_token_in_expression(),
                    ));
                }
            },

            None => {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴瀚弳顒勬煛鐏炶鈧繈鐛笟鈧獮鎺楀箣濠靛柊鎴︽⒒娴ｅ摜锛嶇紒顕呭灠椤繗銇愰幒鎳筹箓鏌涢弴銊ョ仩缂佺姴顭烽幃褰掓惞鐟欏嫮锛婇梺闈涳紡閸涱噯绱￠梻浣筋嚃閸ㄥ骸鐣濋埀顒勫触鐎ｎ喗鈷戦悹鍥ｂ偓铏仌濠电偛顦伴惄顖炲春閵夛箑绶為柟閭﹀墮閸炪劑鎮峰鍐ч柨婵堝仱瀹曘劎鈧稒菤閹锋椽鏌ｉ悢鍝ユ噧閻庢凹鍙冩俊鐢告偄鐠佽法鎳撻…銊╁礋椤撶姷鍘滄俊?
    fn parse_infix(&mut self, left: Expr, precedence: u8) -> Result<Expr> {
        let lo = left.span.lo;
        let token = self.advance().unwrap();

        let kind = match &token.kind {
            TokenKind::Plus => ExprKind::Binary {
                op: BinOp::Add,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Minus => ExprKind::Binary {
                op: BinOp::Sub,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Star => ExprKind::Binary {
                op: BinOp::Mul,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Slash => ExprKind::Binary {
                op: BinOp::Div,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Percent => ExprKind::Binary {
                op: BinOp::Mod,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitAnd => ExprKind::Binary {
                op: BinOp::BitAnd,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitOr => ExprKind::Binary {
                op: BinOp::BitOr,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitXor => ExprKind::Binary {
                op: BinOp::BitXor,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Shl => ExprKind::Binary {
                op: BinOp::Shl,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Shr => ExprKind::Binary {
                op: BinOp::Shr,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::And => ExprKind::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Or => ExprKind::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Eq => ExprKind::Binary {
                op: BinOp::Eq,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::NotEq => ExprKind::Binary {
                op: BinOp::NotEq,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Lt => ExprKind::Binary {
                op: BinOp::Lt,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Le => ExprKind::Binary {
                op: BinOp::Le,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Gt => ExprKind::Binary {
                op: BinOp::Gt,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Ge => ExprKind::Binary {
                op: BinOp::Ge,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },

            TokenKind::Assign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::Assign {
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::AddAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::AddAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::SubAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::SubAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::MulAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::MulAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::DivAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::DivAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::ModAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::ModAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }

            // 闂傚倸鍊搁崐鎼佸磹瀹勬噴褰掑炊椤掆偓绾惧鏌熼悧鍫熺凡闁稿被鍔庨幉鎼佸棘鐠恒劍娈惧銈嗙墱閸庢劙寮崘顔界厓?
            // 闂傚倸鍊搁崐鎼佸磹瀹勬噴褰掑炊椤掆偓绾惧鏌熼悧鍫熺凡闁稿被鍔庨幉鎼佸棘鐠恒劍娈惧銈嗙墱閸庢劙寮崘顔界厓?
            TokenKind::DotDot => {
                let inclusive = self.consume(TokenKind::Eq).is_some();
                let end = if self.check_range_end() {
                    Some(self.parse_expr_prec(PREC_OR)?)
                } else {
                    None
                };
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: end.map(Box::new),
                    inclusive,
                }
            }

            // 闂傚倸鍊搁崐宄懊归崶顒夋晪鐟滃繘鍩€椤掍胶鈻撻柡鍛箘閸掓帒鈻庨幘宕囶唺濠碉紕鍋涢惃鐑藉磻閹捐绀冩い鏃傚帶閼板灝鈹戦悙鏉戠伇濡炲瓨鎮傚鏌ュ煛閸涱喖鈧敻鏌涜箛鎿冩Ц濞存粓绠栧娲焻閻愯尪瀚板褍鐡ㄩ〃銉╂倷閼碱剛顔婇梺閫炲苯澧剧紓宥呮瀹曚即寮介鐔哄弳闂侀潧鐗嗗Λ鏃傛崲閸℃稒鐓忛柛顐ｇ箓椤忣偆绱掑☉妯肩缂?`obj.field` 闂傚倸鍊搁崐鎼佸磹閻戣姤鍤勯柛顐ｆ礀缁犵娀鏌熼崜褏甯涢柛瀣ㄥ€濋弻鏇熺箾閻愵剚鐝曢梺绋款儏椤戝棙绌辨繝鍥ч柛娑卞枛濞咃綁姊洪挊澶婃殶闁哥姵鐗犲濠氭晲婢跺﹥顥濋柣鐘充航閸斿秹顢欓崼銉︹拺闁告繂瀚烽崕鎰版煟濡ゅ啫鈻堢€殿喖顭烽弫鎾绘偐閼碱剙鈧偞淇婇悙宸剰婵炲鍏橀弫宥夋偄鐏忎焦鏂€闂佺粯鍔樼亸娆愭櫠閺囥垺鐓熼柡宓礁浠悗?`obj.method(args)`
            TokenKind::Dot => {
                let field = self.expect_ident()?;

                // 婵犵數濮烽弫鍛婃叏閻戝鈧倿鎸婃竟鈺嬬秮瀹曘劑寮堕幋鐙呯幢闂備線鈧偛鑻晶鎾煛鐏炲墽銆掗柍褜鍓ㄧ紞鍡涘磻閸涱厾鏆︾€光偓閸曨剛鍘搁悗鍏夊亾閻庯綆鍓涢敍鐔哥箾鐎电顎撳┑鈥虫喘楠炲繘鎮╃拠鑼唽闂佸湱鍎ら崺鍫濐焽閵夈儮鏀介柣妯活問閺嗩垶鏌嶈閸撴瑩宕捄銊ф／鐟滄棃寮婚悢纰辨晩闁绘挸绨堕崑鎾诲箹娴ｇ懓浠奸梺缁樺灱濡嫬鏁梻浣稿暱閹碱偊宕愰悷鎵虫瀺闁糕剝绋掗埛鎴︽煕韫囨稒锛熼柤鍓蹭邯閺屾稒鎯旈姀銏″垱闂佽桨绀侀崯鏉戠暦閹烘垟妲堟慨姗嗗墮缁犱即姊婚崒娆掑厡妞ゎ厼鐗撳鐢割敆閸屾稑搴婇悗骞垮劚閹峰鎮炴禒瀣厵闂侇叏绠戦弸銈嗕繆椤愵偄鐏￠柕鍥у楠炴鎹勬潪鎵崟婵＄偑鍊х徊鐐箾婵犲洤钃熼柣鏂挎憸闂勫嫬顭跨捄鐚存闁哥姴锕幃妤冩喆閸曨剛顦ㄩ梺鎸庢磸閸ㄤ粙濡存笟鈧顕€宕煎┑鍡欑崺婵＄偑鍊栧濠氭偤閺冨牆瑙?`obj.method(...)`
                if self.check(TokenKind::LParen) {
                    let mut args = Vec::new();
                    self.advance(); // 婵犵數濮烽弫鍛婃叏閻戣棄鏋侀柟闂寸绾惧鏌ｉ幇顒佹儓闁搞劌鍊块弻锝夊閻樺啿鏆堥梺绋款儏椤戝寮婚悢鍏煎亱闁割偆鍠撻崙锟犳⒑?(
                    while !self.is_eof() {
                        if self.consume(TokenKind::RParen).is_some() {
                            break;
                        }
                        args.push(self.parse_expr()?);
                        self.consume(TokenKind::Comma);
                    }

                    ExprKind::MethodCall {
                        receiver: Box::new(left),
                        method: field,
                        args,
                    }
                } else {
                    // 闂傚倸鍊搁崐宄懊归崶顒夋晪鐟滃繘鍩€椤掍胶鈻撻柡鍛箘閸掓帒鈻庨幘宕囶唺濠碉紕鍋涢惃鐑藉磻閹捐绀冩い鏃傚帶閼板灝鈹戦悙鏉戠伇濡炲瓨鎮傚鏌ュ煛閸涱喖鈧敻鏌涜箛鎿冩Ц濞存粓绠栧娲焻閻愯尪瀚板褍鐡ㄩ〃銉╂倷閼碱剛顔婇梺閫炲苯澧剧紓宥呮瀹曚即寮介鐔哄弳闂侀潧鐗嗗Λ鏃傛崲閸℃稒鐓忛柛顐ｇ箓椤忣偆绱掑☉妯肩缂?
                    ExprKind::Field {
                        base: Box::new(left),
                        field,
                    }
                }
            }

            // 缂傚倸鍊搁崐鎼佸磹閹间礁纾圭€瑰嫰鍋婂〒濠氭煙閻戞ɑ鈷掗柣顓熷哺閺屾盯顢曢敐鍡欘槬闂佹悶鍔岄崐鍧楀蓟濞戙垺鏅滈悹鍥ㄥ絻缁犲搫顪?`arr[index]`
            TokenKind::LBracket => {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                ExprKind::Index {
                    base: Box::new(left),
                    index: Box::new(index),
                }
            }


            // 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柛顭戝亝閸欏繘鏌熺紒銏犳珮闁轰礁瀚伴弻娑樷槈濞嗘劗绋囬梺姹囧€ら崰鏍箒闂佺绻愰崥瀣礊閹达附鐓涢悗锝傛櫇缁愭棃鏌＄仦鐐鐎规洜鍘ч埞鎴﹀箛椤撳／鍥ㄢ拺闁告繂瀚刊濂告煕閹捐泛鏋涚€殿喛顕ч埥澶愬閳哄倹娅囬梻浣瑰缁诲倸煤閵娾晜鍋╁┑鍌氭啞閳锋垿鏌涘┑鍡楊伌婵℃煡顥撶槐鎺楊敊閻ｅ本鍣у銈庡亜缁绘垹鎹㈠┑鍡╂僵妞ゆ帒鍋嗗Σ瑙勪繆閻愵亜鈧牠宕濊瀵板﹪鎸婃径鍡樼€洪梺闈涚箞閸ㄨ崵澹曢挊澹濆綊鏁愰崼顐㈡異濠电偛鐗婂Λ鍐蓟濞戞瑧绡€闁稿本绮堥搹搴ㄦ⒑娴兼瑧鍒伴柛銏＄叀閳ワ箓濡搁埡浣侯槰闂佽鍨庨崟顐熷亾椤栫偞鈷掑ù锝堫潐閸嬬娀鏌涙惔顔兼珝鐎殿喗褰冮埞鎴犫偓锝庡亝濞呮牕鈹戦悙鏉戠仸闁荤啙鍛К闁逞屽墴濮婃椽妫冨☉銏㈠椽缂備焦褰冩晶钘夆槈閸偁鍋呴柛鎰ㄦ櫅閳ь剙鐏氶幈銊ノ熼悡搴′粯濠电偛鎳庣粔鐢搞€冮妷鈺傚€烽柟缁樺笚濞堣尙绱撴担铏瑰笡缂佽鐗嗛悾宄邦潨閳ь剟銆侀弬娆惧悑闁糕剝绋掗弳鍛存⒒閸屾艾鈧兘鎳楅崜浣瑰厹閻犺桨缍嶉敐澶婂窛闁哄鍨甸崑宥夋⒑閸涘﹥瀵欓柛娑卞灣閸?
            _ => {
                return Err(CompileError::ParseError(
                    ParseError::unexpected_token_in_infix(),
                ));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴娲﹀☉褔妫佹径鎰厽婵☆垳鍎ら埢鏇㈡煕鎼达紕绠婚柡宀嬬磿閳ь剨绲洪弲婵堢玻閺冨牊鐓涚€光偓閳ь剟宕版惔銊ョ厺闁规崘顕ч崹鍌涖亜閺冨倹娅曞ù婊堢畺濮婄粯鎷呯憴鍕╀户闂佸憡锚閵堟悂銆侀弽銊ョ窞闁归偊鍓涢悾娲煟閻樿崵绱伴柕鍡忓亾濠碘剝褰冮悧鎾诲蓟閳ュ磭鏆ゆい鏃傚帶閺嬨倖绻涢悡搴含婵﹦绮幏鍛驳鐎ｎ偆绉锋繝纰夌磿閸嬬偟鈧瑳鍛焿鐎广儱鎷嬮悡銉╂煕椤愵偄浜濋柡?
    fn parse_array_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LBracket)?;

        let mut elements = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBracket).is_some() {
                break;
            }

            elements.push(self.parse_expr()?);

            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(ExprKind::Array(elements), self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴娲﹀☉褔妫佹径鎰厽婵☆垳鍎ら埢鏇㈡煕鎼达紕绠绘慨濠呮閹叉濡堕崨顏勪壕闁哄秲鍔庨埞宥呪攽閻樺弶鎼愰崶鎾⒑閸涘﹣绶遍柛娆忓缁傛帡骞橀瑙ｆ嫼闂備緡鍋嗛崑娑㈡嚐椤栨稒娅犳い鏇楀亾闁哄本鐩幃銈嗘媴閸濄儰鍝楁俊鐐€ら崢鐓幟洪埡鍐笉婵炴垶菤濡插牊淇婇婵嗕汗闁哄棔鍗抽弻锝夋偄閸濄儳鐓€缂備胶绮敮鎺楀煡婢舵劖鍋ㄧ紒瀣硶閸?
    fn parse_block_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let block = self.parse_block()?;
        Ok(Expr::new(ExprKind::Block(block), self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?if 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾绾惧鏌ｉ幇顔芥毄闁活厽鐟╅悡顐﹀炊閵娧€妲堢紓浣插亾濠㈣埖鍔楅崣鎾绘煕閵夛絽濡块柍钘夘槺缁辨帡鎮╅懠顑跨驳闂侀潧娲ょ€氼垳绮诲☉銏犵闁归妞掔槐婵嬫⒒?
    fn parse_if_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::IfKw)?;

        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let cond = self.parse_expr()?;
        self.in_condition_context = prev;
        let then_branch = self.parse_block()?;

        let else_branch = if self.consume(TokenKind::ElseKw).is_some() {
            if self.check(TokenKind::IfKw) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let lo = self.current_span().lo;
                let block = self.parse_block()?;
                Some(Box::new(Expr::new(
                    ExprKind::Block(block),
                    self.span_at(lo),
                )))
            }
        } else {
            None
        };

        Ok(Expr::new(
            ExprKind::If {
                cond: Box::new(cond),
                then_branch,
                else_branch,
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?while 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
    fn parse_while_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::WhileKw)?;

        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let cond = self.parse_expr()?;
        self.in_condition_context = prev;
        let body = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::While {
                cond: Box::new(cond),
                body,
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?for 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
    fn parse_for_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::ForKw)?;

        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::InKw)?;
        let iter = self.parse_simple_expr()?;
        let body = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::For {
                pattern,
                iter: Box::new(iter),
                body,
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?loop 闂傚倸鍊峰ù鍥敋瑜嶉湁闁绘垼妫勯弸浣糕攽閻樺疇澹樼痪鎹愵嚙閳规垿鎮╅幓鎺撴濡炪倐鏅滈悡锟犲蓟閿濆绠涙い鏍ㄤ緱娴犫晠鎮?
    fn parse_loop_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LoopKw)?;

        let body = self.parse_block()?;

        Ok(Expr::new(ExprKind::Loop(body), self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?match 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾绾惧鏌ｉ幇顔芥毄闁活厽鐟╅悡顐﹀炊閵娧€妲堢紓浣插亾濠㈣埖鍔楅崣鎾绘煕閵夛絽濡块柍钘夘槺缁辨帡鎮╅懠顑跨驳闂侀潧娲ょ€氼垳绮诲☉銏犵闁归妞掔槐婵嬫⒒?
    fn parse_match_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::MatchKw)?;

        // In `match <scrutinee> { ... }`, the following `{` always starts
        // arm blocks and must not be parsed as a struct literal.
        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let scrutinee = self.parse_expr()?;
        self.in_condition_context = prev;
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 婵犵數濮烽弫鍛婃叏閻戝鈧倿鎸婃竟鈺嬬秮瀹曘劑寮堕幋婵堚偓顓烆渻閵堝懐绠伴柣妤€妫濋幃鐐哄垂椤愮姳绨婚梺鐟版惈濡绂嶉崜褏纾奸柛鎾楀棙顎楅梺鍛娚戦崕鎶藉煡婢舵劖鍋ㄧ紒瀣硶閸?
            let mut patterns = vec![self.parse_pattern()?];

            // `A | B` 婵犵數濮烽弫鍛婃叏閻戝鈧倿鎸婃竟鈺嬬秮瀹曘劑寮堕幋婵堚偓顓烆渻閵堝懐绠伴柣妤€妫濋幃鐐哄垂椤愮姳绨婚梺鐟版惈濡绂嶉崜褏纾奸柛鎾楀棙顎楅梺鍛娚戦崕鎶藉煡婢舵劖鍋ㄧ紒瀣硶閸?
            while self.consume(TokenKind::BitOr).is_some() {
                patterns.push(self.parse_pattern()?);
            }

            // 闂傚倸鍊搁崐鎼佸磹妞嬪海鐭嗗〒姘ｅ亾妤犵偛顦甸弫鎾绘偐閸愯弓鐢绘俊鐐€栭悧婊堝磻濞戙垹鍨傞柛宀€鍋為悡鏇熴亜閹板墎绋荤紒鈧埀顒勬⒑缂佹ê绗掗柣蹇斿哺婵＄敻宕熼姘鳖唺闂佺硶鍓濋妵鐐寸珶閺囩喍绻嗛柣鎰典簻閳ь儸鍛笉闁瑰瓨绻勯弳锔姐亜閹烘垵顏╅柣鎾寸箞閺岀喖骞戦幇闈涙缂備胶濯寸紞渚€寮婚敐鍜佹建闁糕剝銇炵花缁樼箾鐎涙鐭掔紒鐘崇墵瀵鏁愭径瀣簻缂備礁顑嗛娆徫涢崱妯肩瘈闁冲皝鍋撻柛鏇ㄥ墰椤︻參姊?
            let guard = if self.consume(TokenKind::IfKw).is_some() {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };

            self.expect(TokenKind::FatArrow)?;

            let body = self.parse_expr()?;

            self.consume(TokenKind::Comma);

            let mut arm = MatchArm::new(patterns, body, self.current_span());
            if let Some(guard) = guard {
                arm = arm.with_guard(*guard);
            }
            arms.push(arm);
        }

        Ok(Expr::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?Lambda 闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳顭烽弻娑㈠焺閸愵亖妲堥梺绋胯閸旀垿寮婚妶鍚ゅ湱鈧綆鍋呭鎺楁⒑缁嬫鍎愰柟鐟版搐閻ｇ柉銇愰幒婵囨櫇濡炪倖鍔戦崐鏇熸叏濞差亝鈷掑ù锝囩摂閸ゅ啴鏌涢悩铏鐎规洩绻濋幖褰掝敃閿涘嫮鐣鹃梻浣哥秺濡法绮堟担鐟板姅闂傚倷鐒︾€笛兠哄澶婄；闁规儳顕粻楣冩煙閸愭彃妲绘繛璇х畵瀹?`|args| body`
    fn parse_lambda_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;

        // 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴娲﹀☉褔妫佹径鎰厽婵☆垳鍎ら埢鏇㈡煕鎼达紕绠婚柡宀嬬秮椤㈡﹢鎮ゆ担鍦澒闂備礁鎼張顒傜矙閹惧顩烽柨鏇炲€哥粈鍫㈡喐婢跺鍙忛柛銉戔偓閺€浠嬫煟閹邦垰鐨哄褎娲熼弻锝夊冀瑜嬮崑銏⑩偓娈垮枦椤曆囧煡婢舵劕顫呴柣妯活問閸炴椽姊绘担鐑樺殌闁诲繑绻堝畷顖烆敍閻愭潙浠兼繛瀵稿Т椤戝棝鍩?`|arg1, arg2, ...|`
        let mut params = Vec::new();
        self.expect(TokenKind::BitOr)?;

        while !self.is_eof() {
            if self.consume(TokenKind::BitOr).is_some() {
                break;
            }

            let name = self.expect_ident()?;
            params.push(name);

            self.consume(TokenKind::Comma);
        }

        let body = self.parse_expr()?;

        Ok(Expr::new(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?async 闂?
    fn parse_async_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::AsyncKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::AsyncBlock(block), self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱?parallel 闂?
    fn parse_parallel_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::ParallelKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::ParallelBlock(block), self.span_at(lo)))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴瀚弳顒勬煛瀹€瀣М闁轰焦鍔欏畷銊╊敊閼恒儱顏伴梻浣筋嚙鐎涒晠宕欒ぐ鎺戠闁绘梻鍘ч拑鐔哥箾閹存瑥鐏╃紒鐘崇洴閺屾稖绠涢幘瀛樺枑闂佸憡鍨电紞濠傤潖濞差亜浼犻柛鏇ㄥ墻濡偟绱撴担铏瑰笡闁挎洏鍨归锝嗙節濮橆厼浜滈柣鐐寸▓閳ь剙鍘栨竟鏇炩攽閻愭潙鐏﹂柣鐕佸灦閹偤鎸婃径妯煎數閻熸粍绮嶇粋宥夘敂閸繆鎽曞┑鐐村灦椤倿鎮㈤搹鍦紲濠碘槅鍨抽崕銈呪枔椤愩倗纾介柛灞剧懄缁佹澘顪冮弶鎴炴喐闁轰緡鍣ｉ崹鎯х暦閸ャ劍顔曢梻浣虹帛濮婂鍩涢崼銉ユ瀬濠电姴娲﹂崑鐘崇箾閹寸偛鍧婇柛瀣崌瀹曟宕ㄩ褎顥￠梻鍌氬€风粈渚€骞夐敓鐘茬闊洦绋戦悿鐐節婵犲倹鍣界紒鈧繝鍥ㄧ厱闁靛鍠栨晶顖炴煟閹惧鎳勯柕鍥у瀵€燁槼妞ゃ儲绮撻弻鐔兼惞椤愩垹顫掗梺?
    fn parse_call_or_tuple_expr(&mut self, path: Path) -> Result<Expr> {
        let lo = path.span.lo;
        self.expect(TokenKind::LParen)?;

        let mut args = Vec::new();
        while !self.is_eof() {
            if self.consume(TokenKind::RParen).is_some() {
                break;
            }
            args.push(self.parse_expr()?);
            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(
            ExprKind::Call {
                func: Box::new(Expr::new(ExprKind::Path(path.clone()), path.span)),
                args,
            },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐宄懊归崶褏鏆﹂柣銏㈩焾缁愭鏌熼幍顔碱暭闁稿绻濆鍫曞醇濮橆厽鐝旂紓浣界堪閸婃洝鐏冮梺鎸庣箓閹冲酣寮抽悙鐑樼厱濠电姴瀚弳顒勬煛瀹€瀣М闁轰焦鍔欏畷銊╊敍濠娾偓缁辨绱撴担鍝勪壕闁稿孩濞婃俊鍫曞箹娴ｆ瓕鎽曢梺缁樻⒒閸樠呯不濮樿鲸鍠愭繝濠傜墕閸氬綊鏌ｉ弬鍨倯闁绘挸鍟伴幉绋款煥閸繄顦梺鎸庢礀閸婃悂宕归崒鐐粹拺妞ゆ巻鍋撶紒澶婎嚟缁顢涢悙瀵稿幈濠电偞鍨堕悷锔剧礊閹寸姷纾奸柣妯兼暩鐢稒銇勯妸锝呭姦闁诡喗鐟╅幊鐘活敆娴ｇ儤顓婚梻鍌欒兌椤牓顢栭崱娑樼闁搞儜鍛濠殿喗銇涢崑鎾绘煕閳哄绡€鐎规洘甯掗…銊╁川閸涱偄鈧繂顫忕紒妯诲缂佹稑顑呭▓鎰版⒑閹肩偛濡奸梺甯到閻ｇ兘濡搁埡濠冩櫖闂佹寧绻傚Λ娆撳汲?
    fn parse_struct_expr(&mut self, path: Path) -> Result<Expr> {
        let lo = path.span.lo;
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut base = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // `..base` 闂傚倸鍊峰ù鍥х暦閻㈢绐楅柟閭﹀枛閸ㄦ繈鐓崶銊р槈闁哄嫨鍎甸弻娑㈠Ψ椤旂厧顫╅梺鎼炲妼閸婂潡寮诲☉銏℃櫆閻犲洦褰冪粻鍝勵渻?
            if self.consume(TokenKind::DotDot).is_some() {
                base = Some(Box::new(self.parse_expr()?));
                self.consume(TokenKind::RBrace);
                break;
            }

            let (name, name_span) = if let Some(token) = self.current() {
                match &token.kind {
                    TokenKind::Ident => {
                        let span = token.span;
                        self.advance();
                        (FieldName::Ident(self.intern_ident(span)), span)
                    }
                    TokenKind::String(Some(s)) => {
                        let span = token.span;
                        let s = s.clone();
                        self.advance();
                        (FieldName::String(s), span)
                    }
                    _ => {
                        return Err(CompileError::ParseError(ParseError::InvalidStructField {
                            found: format!("{:?}", token.kind),
                            span: source_span(token.span),
                        }));
                    }
                }
            } else {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            };

            if self.consume(TokenKind::Colon).is_some() {
                let value = self.parse_expr()?;
                fields.push(FieldValue::new(name, value, self.current_span()));
            } else {
                // 缂傚倸鍊搁崐鎼佸磹閹间礁纾归柣鎴ｅГ閸ゅ嫰鏌ょ粙璺ㄤ粵闁告瑥绻戦妵鍕箻閸楃偟浠肩紒鐐劤椤兘寮婚悢鐓庣鐟滃繒鏁☉銏＄厽闁规儳鐡ㄧ粈瀣煛瀹€鈧崰鏍蓟閸ヮ剚鏅濋柍褜鍓熼悰顔碱潨閳ь剟寮婚悢鐓庣闁圭粯甯楀▓濠氭⒑閸濆嫯顫﹂柛濠冪箓閻ｇ兘濡搁敂鍓х槇闂佸憡鍔︽禍婊堝吹閹烘挷绻?`{ x }` 缂傚倸鍊搁崐鎼佸磹閹间礁纾归柣鎴ｅГ閸婂潡鏌ㄩ弬鍨挃闁活厽鐟╅弻鐔封枎闄囬褍煤椤擃潿鈧礁顫濈捄铏瑰姦濡炪倖甯掔€氼剛绮婚弽顓熺厵闁告挷鑳堕幗鍌炴煛娴ｅ摜校缂佺粯鐩獮瀣倷閹绘帗鐦撴繝?`{ x: x }`
                if let FieldName::Ident(ref ident) = name {
                    fields.push(FieldValue::shorthand(ident.clone(), self.current_span()));
                } else {
                    return Err(CompileError::ParseError(
                        ParseError::InvalidStructFieldShorthand {
                            span: source_span(name_span),
                        },
                    ));
                }
            }

            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(
            ExprKind::Struct { path, fields, base },
            self.span_at(lo),
        ))
    }

    /// 闂傚倸鍊搁崐鎼佸磹妞嬪海鐭嗗〒姘ｅ亾鐎规洏鍎抽埀顒婄秵閸犳牜澹曢崸妤佺厵闁诡垳澧楅ˉ澶愬箹閺夋埊韬柡灞诲€濋幊婵嬪箥椤旇偐澧┑鐐茬摠缁瞼绱炴繝鍥ц摕婵炴垯鍨瑰敮闂佹寧绻傞幊搴ㄢ€栫€ｎ喗鈷戠€规洖娲ㄧ敮娑欐叏婵犲倻绉哄┑锛勬暬瀹曠喖顢涘槌栧晪闂佽崵濮惧▍锝吤洪崨鏉懳ㄧ憸蹇撐涢鐐寸厵妞ゆ牕妫楅惉濂稿触鐎ｎ喗鍊垫繛鍫濈仢閺嬫盯鏌ｉ弽褋鍋㈢€规洘妞介弫鎾绘偐閼碱剨绱叉繝娈垮枟閿曗晠宕滈敃鈧…鍥即閵忊檧鎷绘繛杈剧秮椤ユ挻绋夐懠顒傜＝鐎广儰绀佹禍楣冩煟鎼淬値娼愭繛鍙夌矒瀵偊骞栨担鍝ヮ槴闂佸湱鍎ら〃蹇涘极閸ヮ剚鐓忛煫鍥э工婢у弶绻涢崨顓燁棦婵﹨娅ｇ槐鎺懳熼崫鍕戞洟姊洪崨濠冨鞍闁烩晩鍨伴锝夊箮閼恒儱浜归悗瑙勬礀濞层劑顢欓弴銏♀拺闁告繂瀚峰Σ褰掓煕閵娧勬毈妤犵偛鍟撮獮瀣晜閻ｅ苯寮虫繝娈垮枟椤牓宕洪弽顓熷亗闁哄洨鍠嶇换鍡樸亜閺嶃劎绠撻柛姘秺閺屾洟宕卞Ο鐑樿癁闂?
    fn get_infix_precedence(&self, kind: &TokenKind) -> u8 {
        match kind {
            TokenKind::Assign
            | TokenKind::AddAssign
            | TokenKind::SubAssign
            | TokenKind::MulAssign
            | TokenKind::DivAssign
            | TokenKind::ModAssign => PREC_ASSIGN,

            TokenKind::Or => PREC_OR,
            TokenKind::And => PREC_AND,

            TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::Le
            | TokenKind::Gt
            | TokenKind::Ge => PREC_COMPARE,

            TokenKind::BitOr => PREC_BIT_OR,
            TokenKind::BitXor => PREC_BIT_XOR,
            TokenKind::BitAnd => PREC_BIT_AND,
            TokenKind::Shl | TokenKind::Shr => PREC_SHIFT,

            TokenKind::Plus | TokenKind::Minus => PREC_ADD,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => PREC_MUL,

            TokenKind::Dot | TokenKind::LBracket | TokenKind::LParen => PREC_CALL,

            TokenKind::DotDot => PREC_OR,

            _ => 0,
        }
    }

    /// 婵犵數濮烽弫鍛婃叏閻戝鈧倿鎸婃竟鈺嬬秮瀹曘劑寮堕幋鐙呯幢闂備線鈧偛鑻晶鎾煛鐏炲墽銆掗柍褜鍓ㄧ紞鍡涘磻閸涱厾鏆︾€光偓閸曨剛鍘搁悗鍏夊亾閻庯綆鍓涢敍鐔哥箾鐎电顎撳┑鈥虫喘楠炲繘鎮╃拠鑼唽闂佸湱鍎ら崺鍫濐焽閵夈儮鏀介柣妯活問閺嗩垶鏌嶈閸撴瑩宕捄銊ф／鐟滄棃寮婚悢纰辨晩闁绘挸绨堕崑鎾诲箹娴ｇ懓浠奸梺缁樺灱濡嫬鏁梻浣稿暱閹碱偊宕愰悷鎵虫瀺闁糕剝绋掗埛鎴︽煕韫囨稒锛熼柤鍓蹭邯閺屾稒鎯旈姀銏″垱闂佽桨绀侀崯鏉戠暦閹烘垟妲堟俊顖滄嚀閹藉鈹戦悩鍨毄闁稿鍋ゅ畷褰掑醇閺囩喎浜遍梺鍝勬川閸犳劙宕ｈ箛娑欑厓鐟滄粓宕滈悢濂夋綎闁惧繗顫夐崰鍡涙煕閺囥劌鍘靛ù鐘欏嫮绠鹃悗娑欘焽閻绱掗鑺ュ磳闁靛棔绀侀～婵嬫嚋閸偅鐝抽梻浣稿閸嬩線宕规繝姘骇闁归棿鐒﹂埛鎴︽偡濞嗗繐顏╅柛鏂诲€曢…鑳槻闂佸府缍佸顐㈩吋閸涱亝顫嶅┑鐐叉閸ㄥ綊鎯?
    fn check_expr(&self) -> bool {
        if let Some(token) = self.current() {
            matches!(
                &token.kind,
                TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::String(_)
                    | TokenKind::Char(_)
                    | TokenKind::TrueKw
                    | TokenKind::FalseKw
                    | TokenKind::NullKw
                    | TokenKind::Ident
                    | TokenKind::LParen
                    | TokenKind::LBrace
                    | TokenKind::LBracket
                    | TokenKind::IfKw
                    | TokenKind::WhileKw
                    | TokenKind::ForKw
                    | TokenKind::LoopKw
                    | TokenKind::MatchKw
                    | TokenKind::ReturnKw
                    | TokenKind::BreakKw
                    | TokenKind::ContinueKw
                    | TokenKind::YieldKw
                    | TokenKind::AsyncKw
                    | TokenKind::ParallelKw
                    | TokenKind::Minus
                    | TokenKind::Not
                    | TokenKind::BitNot
                    | TokenKind::And
                    | TokenKind::Star
            )
        } else {
            false
        }
    }

    /// 婵犵數濮烽弫鍛婃叏閻戝鈧倿鎸婃竟鈺嬬秮瀹曘劑寮堕幋鐙呯幢闂備線鈧偛鑻晶鎾煛鐏炲墽銆掗柍褜鍓ㄧ紞鍡涘磻閸涱厾鏆︾€光偓閸曨剛鍘搁悗鍏夊亾閻庯綆鍓涢敍鐔哥箾鐎电顎撳┑鈥虫喘楠炲繘鎮╃拠鑼唽闂佸湱鍎ら崺鍫濐焽閵夈儮鏀介柣妯活問閺嗩垶鏌嶈閸撴瑩宕捄銊ф／鐟滄棃寮婚悢纰辨晩闁绘挸绨堕崑鎾诲箹娴ｇ懓浠奸梺缁樺灱濡嫬鏁梻浣稿暱閹碱偊宕愰悷鎵虫瀺闁糕剝绋掗埛鎴︽煕韫囨稒锛熼柤鍓蹭邯閺屾稒鎯旈姀銏″垱闂佽桨绀侀崯鏉戠暦閹烘垟妲堟慨姗嗗墮缁犲姊婚崒娆掑厡缂侇噮鍨堕獮鎰板川婵犲孩顔旈梺褰掓？缁€浣感ч崣澶岀闁糕剝锚婵绱掗悩鑽ょ暫闁哄被鍊濋幊婵嬪级鐠恒劌甯跨紓浣鸿檸閸樺吋鏅舵惔锝嗩潟闁圭儤鎸荤紞鍥煏婵炲灝鍔滈悹鍥╁仜铻栭柣姗€娼ф禒锔姐亜椤撶偞鍠橀柛鈹惧亾濡炪倖甯婇懗鍫曞煀閺囥垺鐓ユ慨妯垮煐閻?
    fn check_range_end(&self) -> bool {
        self.check_expr()
    }
}

/// 闂?AST Span 闂傚倸鍊搁崐椋庣矆娓氣偓楠炴牠顢曚綅閸ヮ剚鐒肩€广儱鎳愰敍鐔兼⒑閸︻厼顣兼繝銏☆焽缁牓宕奸悢绋垮伎濠殿喗顨呭Λ妤佹櫠娴煎瓨鐓涢柛鈽嗗幘閻ｇ敻鏌＄仦鍓р槈闁宠姘︾粻娑㈡晲閸犺埇鍔戝?miette SourceSpan
fn source_span(span: crate::lexer::Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
