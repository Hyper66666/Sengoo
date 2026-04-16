use super::*;

impl TypeChecker {
    pub(super) fn set_generic_function_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_function_metas.remove(&name);
        } else {
            self.generic_function_metas
                .insert(name, GenericFunctionMeta { params });
        }
    }

    pub(super) fn set_generic_type_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_type_metas.remove(&name);
        } else {
            self.generic_type_metas
                .insert(name, GenericTypeMeta { params });
        }
    }

    pub(super) fn collect_generic_type_meta(
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
}
