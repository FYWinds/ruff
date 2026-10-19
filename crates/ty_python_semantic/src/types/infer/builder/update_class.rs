//! Type-expression inference for experimental [PEP 827] class updates (`UpdateClass[*Ms]`).
//!
//! `UpdateClass[...]` may only be written as the whole return annotation of a function (a class
//! decorator or `__init_subclass__`, P5-23 / P5-28); see [`crate::types::class::update_class`]
//! for how classes are updated.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_python_ast as ast;

use super::TypeInferenceBuilder;
use crate::types::Type;
use crate::types::diagnostic::{self, INVALID_TYPE_FORM};
use crate::types::infer::InferenceFlags;
use crate::types::type_operator::TypeOperatorKind;

impl<'db> TypeInferenceBuilder<'db, '_> {
    /// Infers `UpdateClass[*Ms]` in a type expression.
    pub(super) fn infer_update_class_special_form(
        &mut self,
        subscript: &ast::ExprSubscript,
    ) -> Type<'db> {
        let flags = self.inference_flags();
        let in_return_annotation = flags.contains(InferenceFlags::IN_RETURN_TYPE)
            && !flags.contains(InferenceFlags::IN_NESTED_TYPE_EXPRESSION);

        let arguments_slice = &*subscript.slice;
        let arguments = if let ast::Expr::Tuple(tuple) = arguments_slice {
            &*tuple.elts
        } else {
            std::slice::from_ref(arguments_slice)
        };
        let arguments = self.infer_variadic_type_arguments(arguments);

        if !in_return_annotation {
            if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, subscript) {
                diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                    "`UpdateClass` is only allowed as the return annotation of a function",
                ));
            }
            return Type::unknown();
        }
        // An `Unknown` argument stems from an invalid type expression that has already been
        // reported.
        if arguments.iter().any(|argument| argument.ty().is_unknown()) {
            return Type::unknown();
        }
        self.evaluate_type_operator(TypeOperatorKind::UpdateClass(arguments), subscript)
    }
}
