use super::*;

pub(super) struct CallInvocationPlan {
    pub(super) actual_func: String,
    pub(super) local_ty: MIRType,
    pub(super) final_args: Vec<Local>,
    pub(super) future_origin: Option<String>,
    pub(super) struct_type_name: Option<String>,
}

pub(super) fn build_call_invocation_plan(
    func_name: &str,
    ret_type: &MIRType,
    env_ptr_local: Option<Local>,
    arg_locals: &[Local],
    async_functions: &HashSet<String>,
) -> CallInvocationPlan {
    let is_async_call = async_functions.contains(func_name);
    let local_ty = if is_async_call {
        MIRType::Future(Box::new(ret_type.clone()))
    } else {
        ret_type.clone()
    };

    let mut final_args = Vec::with_capacity(arg_locals.len() + usize::from(env_ptr_local.is_some()));
    if let Some(env_ptr) = env_ptr_local {
        final_args.push(env_ptr);
    }
    final_args.extend(arg_locals.iter().copied());

    let actual_func = if is_async_call {
        format!("{func_name}__start")
    } else {
        func_name.to_string()
    };

    let future_origin = is_async_call.then(|| func_name.to_string());
    let struct_type_name = match (is_async_call, ret_type) {
        (false, MIRType::Struct { name, .. }) => Some(name.clone()),
        _ => None,
    };

    CallInvocationPlan {
        actual_func,
        local_ty,
        final_args,
        future_origin,
        struct_type_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_call_invocation_plan_wraps_async_start_and_env_pointer() {
        let plan = build_call_invocation_plan(
            "worker",
            &MIR_BOOL,
            Some(Local::new(7, LocalKind::Temp)),
            &[Local::new(8, LocalKind::Temp)],
            &["worker".to_string()].into_iter().collect(),
        );

        assert_eq!(plan.actual_func, "worker__start");
        assert_eq!(plan.local_ty, MIRType::Future(Box::new(MIR_BOOL)));
        assert_eq!(
            plan.final_args,
            vec![Local::new(7, LocalKind::Temp), Local::new(8, LocalKind::Temp)]
        );
        assert_eq!(plan.future_origin.as_deref(), Some("worker"));
        assert_eq!(plan.struct_type_name, None);
    }

    #[test]
    fn build_call_invocation_plan_preserves_sync_struct_return_name() {
        let ret_type = MIRType::Struct {
            name: "Pair".to_string(),
            fields: vec![("value".to_string(), MIR_I64)],
        };
        let plan = build_call_invocation_plan("pair_make", &ret_type, None, &[], &HashSet::new());

        assert_eq!(plan.actual_func, "pair_make");
        assert_eq!(plan.local_ty, ret_type);
        assert_eq!(plan.struct_type_name.as_deref(), Some("Pair"));
        assert_eq!(plan.future_origin, None);
    }
}
