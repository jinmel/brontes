use quote::{quote, ToTokens};
use syn::ExprClosure;

pub struct ClosureDispatch {
    logs:        bool,
    call_data:   bool,
    return_data: bool,
    closure:     ExprClosure,
}

impl ClosureDispatch {
    pub fn new(logs: bool, call_data: bool, return_data: bool, closure: ExprClosure) -> Self {
        Self { closure, call_data, return_data, logs }
    }
}

impl ToTokens for ClosureDispatch {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let closure_expr = &self.closure;

        let call_data_token = self
            .call_data
            .then_some(quote!(call_data,))
            .unwrap_or_default();

        let return_data_token = self
            .return_data
            .then_some(quote!(return_data,))
            .unwrap_or_default();

        let log_data_token = self.logs.then_some(quote!(log_data,)).unwrap_or_default();

        let param_count = self.closure.inputs.len();
        let base_param_count =
            2 + self.call_data as usize + self.return_data as usize + self.logs as usize;
        let expects_tracer = param_count > base_param_count;
        let tracer_param_token = if expects_tracer { quote!(, tracer.clone()) } else { quote!() };

        // Check if the closure contains Box::pin by converting to string and checking
        // for patterns
        let closure_string = format!("{}", closure_expr.to_token_stream());
        let returns_future =
            closure_string.contains("Box :: pin") || closure_string.contains("Box::pin");

        if returns_future {
            tokens.extend(quote!(
                let fixed_fields = call_info.get_fixed_fields();
                let closure_result = (#closure_expr)(
                    fixed_fields,
                    #call_data_token
                    #return_data_token
                    #log_data_token
                    db_tx
                    #tracer_param_token
                );
                let result: ::eyre::Result<_> = closure_result.await;
            ));
        } else {
            tokens.extend(quote!(
                let fixed_fields = call_info.get_fixed_fields();
                let result: ::eyre::Result<_> = (#closure_expr)(
                    fixed_fields,
                    #call_data_token
                    #return_data_token
                    #log_data_token
                    db_tx
                    #tracer_param_token
                );
            ));
        }

        tokens.extend(quote!(
            // metrics
            if result.is_err() {
                if let Ok(protocol) = db_tx.get_protocol(call_info.target_address) {
                    crate::CLASSIFICATION_METRICS.get_or_init(||
                        brontes_metrics::classifier::ClassificationMetrics::default())
                        .bad_protocol_classification(protocol);
                }
            }
            let result = result?; // Unwraps the result for use in #dex_price_return by the parent macro
        ));
    }
}
