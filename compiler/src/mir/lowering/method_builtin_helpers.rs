use super::*;

pub(super) fn try_lower_string_len_method_call(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    receiver_ty: &MIRType,
    method: &str,
) -> Option<Local> {
    if let MIRType::Ptr(inner) = receiver_ty {
        if let MIRType::Int(8) = inner.as_ref() {
            if method == "len" {
                let result_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Call {
                    destination: result_local,
                    func: "sengoo_str_len".to_string(),
                    args: vec![receiver_local],
                });
                return Some(result_local);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_lower_string_len_method_call_emits_runtime_len() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();
        let start_block = mir_fn.start_block;

        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let receiver = ctx.add_local(
            None,
            LocalKind::Temp,
            MIRType::Ptr(Box::new(MIRType::Int(8))),
        );

        let receiver_ty = ctx.get_local_type(receiver).clone();
        let result = try_lower_string_len_method_call(&mut ctx, receiver, &receiver_ty, "len");

        let result = result.expect("expected string len helper to match");
        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver]
        )));
    }
}