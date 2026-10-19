//! Type-expression inference for experimental PEP 827 type operators: the type booleans
//! `IsAssignable`, `IsEquivalent` and `Bool`, `not`/`and`/`or` over type booleans, and
//! conditional types (`X if C else Y`).

use ruff_python_ast as ast;

use super::TypeInferenceBuilder;
use crate::types::diagnostic::{
    self, INVALID_TYPE_FORM, report_invalid_argument_number_to_special_form,
};
use crate::types::type_operator::{TypeOperatorKind, is_type_bool_operand};
use crate::types::{SpecialFormType, Type, TypeContext, TypeOperatorType};

/// Returns `true` if `expression` has the syntactic shape of a type boolean: a name, an
/// attribute, a subscript, a quantifier (`any(...)` / `all(...)` over a generator, see
/// [`type_quantifier_generator`]), or `not`/`and`/`or` applied to such expressions.
///
/// `if`/`and`/`or`/`not` expressions in a type expression are only interpreted as PEP 827 type
/// operators if all of their condition operands have this shape; other shapes (such as
/// `1 if True else 2`) keep reporting the generic "not allowed in type expressions" errors.
pub(super) fn is_type_bool_operand_syntax(expression: &ast::Expr) -> bool {
    match expression {
        ast::Expr::Name(_) | ast::Expr::Attribute(_) | ast::Expr::Subscript(_) => true,
        ast::Expr::Call(call) => type_quantifier_generator(call).is_some(),
        ast::Expr::UnaryOp(ast::ExprUnaryOp {
            op: ast::UnaryOp::Not,
            operand,
            ..
        }) => is_type_bool_operand_syntax(operand),
        ast::Expr::BoolOp(bool_op) => bool_op.values.iter().all(is_type_bool_operand_syntax),
        _ => false,
    }
}

/// Returns the generator of a call with the syntactic shape of a PEP 827 quantifier: `any(...)`
/// or `all(...)` with a single generator argument and no keyword arguments.
///
/// The semantic index uses the same purely syntactic test
/// (`SemanticIndexBuilder::type_quantifier_generator` in `ty_python_core`) to decide that such a
/// generator in a type expression gets no scope of its own; keep the two in sync. Whether the
/// name actually refers to the builtin is checked during inference.
pub(super) fn type_quantifier_generator(call: &ast::ExprCall) -> Option<&ast::ExprGenerator> {
    let ast::Expr::Name(name) = &*call.func else {
        return None;
    };
    if !matches!(name.id.as_str(), "any" | "all") || !call.arguments.keywords.is_empty() {
        return None;
    }
    match &*call.arguments.args {
        [ast::Expr::Generator(generator)] => Some(generator),
        _ => None,
    }
}

impl<'db> TypeInferenceBuilder<'db, '_> {
    /// Infer a condition operand (an operand of `not`/`and`/`or`, or the condition of a
    /// conditional type). Returns `None` if it is not a type boolean.
    ///
    /// The operand is first evaluated speculatively. If it is not a type boolean, it is not
    /// inferred as a type expression at all (it is inferred as an ordinary value expression, like
    /// other invalid type-expression syntax), so that a single diagnostic is reported by the
    /// caller instead of follow-up type-expression diagnostics for the operand.
    pub(super) fn infer_type_bool_condition(&mut self, operand: &ast::Expr) -> Option<Type<'db>> {
        let speculative = self
            .speculate_without_diagnostics()
            .infer_type_expression(operand);
        if is_type_bool_operand(self.db(), self.program_environment(), speculative) {
            return Some(self.infer_type_expression(operand));
        }
        if !self.in_string_annotation() {
            self.infer_maybe_standalone_expression(operand, TypeContext::default());
        }
        None
    }

    fn report_invalid_type_bool_operand(&self, operand: &ast::Expr) {
        if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, operand) {
            diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                "Operands of `and`/`or`/`not` in a type expression must be type booleans",
            ));
        }
    }

    /// Infer `B1 and B2 ...` / `B1 or B2 ...` in a type expression.
    pub(super) fn infer_type_bool_op_type_expression(
        &mut self,
        bool_op: &ast::ExprBoolOp,
    ) -> Type<'db> {
        let mut operands = Vec::with_capacity(bool_op.values.len());
        let mut first_invalid = None;
        for value in &bool_op.values {
            match self.infer_type_bool_condition(value) {
                Some(ty) => operands.push(ty),
                None => {
                    first_invalid.get_or_insert(value);
                }
            }
        }
        if let Some(invalid) = first_invalid {
            // Report once per boolean operation.
            self.report_invalid_type_bool_operand(invalid);
            return Type::unknown();
        }
        let operands = operands.into_boxed_slice();
        let kind = match bool_op.op {
            ast::BoolOp::And => TypeOperatorKind::And(operands),
            ast::BoolOp::Or => TypeOperatorKind::Or(operands),
        };
        TypeOperatorType::evaluate_or_defer(self.db(), self.program_environment(), kind)
    }

    /// Infer `not B` in a type expression.
    pub(super) fn infer_type_not_type_expression(&mut self, unary: &ast::ExprUnaryOp) -> Type<'db> {
        if let Some(operand) = self.infer_type_bool_condition(&unary.operand) {
            TypeOperatorType::evaluate_or_defer(
                self.db(),
                self.program_environment(),
                TypeOperatorKind::Not(operand),
            )
        } else {
            self.report_invalid_type_bool_operand(&unary.operand);
            Type::unknown()
        }
    }

    /// Infer the conditional type `X if C else Y`.
    pub(super) fn infer_conditional_type_expression(
        &mut self,
        if_expression: &ast::ExprIf,
    ) -> Type<'db> {
        let condition = self.infer_type_bool_condition(&if_expression.test);
        let if_true = self.infer_type_expression(&if_expression.body);
        let if_false = self.infer_type_expression(&if_expression.orelse);

        let Some(condition) = condition else {
            if let Some(builder) = self
                .context
                .report_lint(&INVALID_TYPE_FORM, &*if_expression.test)
            {
                diagnostic::add_type_expression_reference_link(
                    builder
                        .into_diagnostic("Condition of a conditional type must be a type boolean"),
                );
            }
            return Type::unknown();
        };

        // An `Unknown` condition stems from an invalid type expression (which has already been
        // reported); don't guess a branch.
        if condition.is_unknown() {
            return Type::unknown();
        }

        TypeOperatorType::evaluate_or_defer(
            self.db(),
            self.program_environment(),
            TypeOperatorKind::Conditional {
                condition,
                if_true,
                if_false,
            },
        )
    }

    /// Infer a subscripted `IsAssignable[T, S]`, `IsEquivalent[T, S]` or `Bool[T]`.
    pub(super) fn infer_type_bool_special_form_type_expression(
        &mut self,
        subscript: &ast::ExprSubscript,
        special_form: SpecialFormType,
    ) -> Type<'db> {
        let arguments_slice = &*subscript.slice;
        let arguments = if let ast::Expr::Tuple(tuple) = arguments_slice {
            &*tuple.elts
        } else {
            std::slice::from_ref(arguments_slice)
        };
        let expected: u8 = match special_form {
            SpecialFormType::IsAssignable | SpecialFormType::IsEquivalent => 2,
            SpecialFormType::Bool => 1,
            _ => unreachable!("not a PEP 827 type-boolean special form: {special_form:?}"),
        };

        let ty = if arguments.len() == usize::from(expected) {
            let operands: Vec<Type<'db>> = arguments
                .iter()
                .map(|argument| {
                    if special_form == SpecialFormType::Bool {
                        self.infer_type_expression(argument)
                    } else {
                        self.infer_type_operator_class_argument(argument)
                    }
                })
                .collect();
            let kind = match (special_form, operands.as_slice()) {
                (SpecialFormType::IsAssignable, [source, target]) => {
                    TypeOperatorKind::IsAssignable(*source, *target)
                }
                (SpecialFormType::IsEquivalent, [left, right]) => {
                    TypeOperatorKind::IsEquivalent(*left, *right)
                }
                (SpecialFormType::Bool, [operand]) => TypeOperatorKind::Bool(*operand),
                _ => unreachable!("argument count was checked above"),
            };
            TypeOperatorType::evaluate_or_defer(self.db(), self.program_environment(), kind)
        } else {
            if !self.in_string_annotation() {
                for argument in arguments {
                    self.infer_expression(argument, TypeContext::default());
                }
            }
            report_invalid_argument_number_to_special_form(
                &self.context,
                subscript,
                special_form,
                arguments.len(),
                expected,
            );
            Type::unknown()
        };

        if arguments_slice.is_tuple_expr() {
            self.store_expression_type(arguments_slice, ty);
        }
        ty
    }
}
