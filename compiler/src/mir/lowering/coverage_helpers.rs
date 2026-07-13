use super::*;

pub(super) const COVERAGE_HIT_RUNTIME: &str = "sengoo_coverage_hit";
pub(super) const COVERAGE_REGISTER_RUNTIME: &str = "sengoo_coverage_register";

impl LoweringContext<'_> {
    pub(super) fn emit_coverage_hit(&mut self, site_lo: u32) {
        let Some(context) = self.options.coverage.clone() else {
            return;
        };
        let Some(line) = context.line_for_site(site_lo) else {
            return;
        };
        context.executable_lines.borrow_mut().insert(line);

        let line_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: line_local,
            value: MirConstant::Int(i64::from(line)),
        });
        let result = self.add_local(None, LocalKind::Temp, MIR_UNIT);
        self.push_inst(Instruction::Call {
            destination: result,
            func: COVERAGE_HIT_RUNTIME.to_string(),
            args: vec![line_local],
        });
    }
}

pub(super) fn inject_coverage_registration(
    functions: &mut [MirFunction],
    options: &MirLowerOptions,
) {
    let Some(context) = options.coverage.as_ref() else {
        return;
    };
    let lines = context
        .executable_lines
        .borrow()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }
    let Some(main) = functions
        .iter_mut()
        .find(|function| function.name == "main")
    else {
        return;
    };

    let mut registration = Vec::with_capacity(lines.len() * 2);
    for line in lines {
        let line_local = main.add_local(LocalKind::Temp, MIR_I64);
        let assign = main.alloc_inst(Instruction::Assign {
            destination: line_local,
            value: MirConstant::Int(i64::from(line)),
        });
        let result = main.add_local(LocalKind::Temp, MIR_UNIT);
        let call = main.alloc_inst(Instruction::Call {
            destination: result,
            func: COVERAGE_REGISTER_RUNTIME.to_string(),
            args: vec![line_local],
        });
        main.hide_instruction_from_debug(assign);
        main.hide_instruction_from_debug(call);
        registration.push(assign);
        registration.push(call);
    }

    main.basic_blocks[main.start_block]
        .instructions
        .splice(0..0, registration);
}
