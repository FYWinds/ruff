//! Type-expression inference for experimental [PEP 827] generic callables
//! (`GenericCallable[tuple[V1, ...], lambda T1, ...: Ty]`) and for other lambdas in type
//! expressions.
//!
//! A lambda in an annotation, a `type` statement or a string annotation does not create a scope
//! in the semantic index, and its parameters have no definitions (like type comprehensions, see
//! [`super::type_comprehension`]). Its parameters are synthetic type variables, kept in
//! [`TypeInferenceBuilder::type_binders`] while its body is inferred, that shadow any place
//! lookup of their names. A lambda in a position that the semantic index treats as a value
//! expression (an implicit type alias, `cast(...)`, ...) does have a scope; it is inferred as a
//! value and rejected before any name in it is looked up (P5-30).
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_core::scope::NodeWithScopeRef;

use super::TypeInferenceBuilder;
use crate::types::diagnostic::{
    self, INVALID_TYPE_FORM, report_invalid_argument_number_to_special_form,
};
use crate::types::generic_callable::generic_callable_binder;
use crate::types::infer::{InferenceFlags, TypeExpressionFlags};
use crate::types::tuple::Tuple;
use crate::types::type_operator::TypeOperatorKind;
use crate::types::typevar::TypeVarInstance;
use crate::types::{BoundTypeVarInstance, KnownInstanceType, SpecialFormType, Type, TypeContext};

impl<'db> TypeInferenceBuilder<'db, '_> {
    /// Returns `true` if the semantic index created a scope for `lambda`.
    ///
    /// This is the case for lambdas in type expressions that are indexed as value expressions
    /// (implicit type aliases, `cast`, `TypeVar` bounds, ...). Their parameters belong to the
    /// lambda scope, so they are not supported in a `GenericCallable`.
    fn type_lambda_has_value_scope(&self, lambda: &ast::ExprLambda) -> bool {
        !self.in_string_annotation()
            && self
                .index
                .try_node_scope(NodeWithScopeRef::Lambda(lambda))
                .is_some()
    }

    fn report_invalid_generic_callable(&self, node: impl Ranged, message: &str) {
        if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, node) {
            diagnostic::add_type_expression_reference_link(builder.into_diagnostic(message));
        }
    }

    /// Infers the sub-expressions of a lambda that appears in a type expression anywhere but as
    /// the second argument of `GenericCallable` (always an error, reported by the caller),
    /// exactly once. Does not store the type of the lambda itself.
    pub(super) fn infer_invalid_lambda_type_expression(&mut self, lambda: &ast::ExprLambda) {
        if self.in_string_annotation() {
            return;
        }
        if self.type_lambda_has_value_scope(lambda) {
            self.infer_lambda_expression(lambda, TypeContext::default());
        } else {
            // The lambda itself is reported; its body is not meant as a type expression, so
            // don't report it as one.
            self.infer_type_lambda_parts(lambda, None, true);
        }
    }

    /// Infers the default values (as value expressions) and the body (as a type expression) of a
    /// lambda without a scope of its own, with a binder for every parameter in scope while the
    /// body is inferred. The binder of the `i`-th positional parameter takes the bound,
    /// constraints and default of `sources[i]`, if any.
    ///
    /// If `suppress_body_diagnostics` is `true`, no diagnostics are reported for the body.
    ///
    /// Returns the binders of the positional parameters, the type of the body, and whether every
    /// parameter is a positional parameter without a default value.
    fn infer_type_lambda_parts(
        &mut self,
        lambda: &ast::ExprLambda,
        sources: Option<&[TypeVarInstance<'db>]>,
        suppress_body_diagnostics: bool,
    ) -> (Vec<BoundTypeVarInstance<'db>>, Type<'db>, bool) {
        let db = self.db();
        let env = self.program_environment();
        let previous_binders = self.type_binders.len();
        let mut positional = Vec::new();
        let mut plain = true;

        if let Some(parameters) = lambda.parameters.as_deref() {
            for parameter in parameters.iter_non_variadic_params() {
                if let Some(default) = parameter.default.as_deref() {
                    plain = false;
                    if !self.in_string_annotation() {
                        self.infer_expression(default, TypeContext::default());
                    }
                }
            }
            plain &= parameters.vararg.is_none()
                && parameters.kwonlyargs.is_empty()
                && parameters.kwarg.is_none();

            for (index, parameter) in parameters
                .posonlyargs
                .iter()
                .chain(&parameters.args)
                .enumerate()
            {
                let name = parameter.name().id.clone();
                let source = sources.and_then(|sources| sources.get(index).copied());
                let binder = generic_callable_binder(db, env, name.clone(), source);
                positional.push(binder);
                self.type_binders.push((name, binder));
            }
            for parameter in parameters
                .vararg
                .as_deref()
                .into_iter()
                .chain(
                    parameters
                        .kwonlyargs
                        .iter()
                        .map(|parameter| &parameter.parameter),
                )
                .chain(parameters.kwarg.as_deref())
            {
                let name = parameter.name.id.clone();
                let binder = generic_callable_binder(db, env, name.clone(), None);
                self.type_binders.push((name, binder));
            }
        }

        let suppress = suppress_body_diagnostics
            || self
                .context
                .inference_flags
                .contains(InferenceFlags::IN_NO_TYPE_CHECK);
        let previously_suppressed = self
            .context
            .inference_flags
            .replace(InferenceFlags::IN_NO_TYPE_CHECK, suppress);
        let body = self.infer_type_expression(&lambda.body);
        self.context
            .inference_flags
            .set(InferenceFlags::IN_NO_TYPE_CHECK, previously_suppressed);
        self.type_binders.truncate(previous_binders);
        (positional, body, plain)
    }

    /// Returns the type variables of the `Vs` argument of `GenericCallable` (a `tuple[...]` of
    /// type variables), or `None` if it is not of that form.
    fn generic_callable_type_variables(&self, vs: Type<'db>) -> Option<Vec<TypeVarInstance<'db>>> {
        let db = self.db();
        let tuple = vs.exact_tuple_instance_spec(db)?;
        let Tuple::Fixed(tuple) = tuple.as_ref() else {
            return None;
        };
        tuple
            .elements_slice()
            .iter()
            .map(|element| {
                let typevar = match element {
                    Type::TypeVar(bound_typevar) => bound_typevar.typevar(db),
                    Type::KnownInstance(KnownInstanceType::TypeVar(typevar)) => *typevar,
                    _ => return None,
                };
                (!typevar.is_paramspec(db) && !typevar.is_typevartuple(db)).then_some(typevar)
            })
            .collect()
    }

    /// Infer the `Vs` argument of `GenericCallable`.
    ///
    /// The type variables of `Vs` only provide bounds, constraints and defaults; they are not
    /// used as types, so they need not be bound to an enclosing generic context.
    fn infer_generic_callable_type_variables(&mut self, vs_node: &ast::Expr) -> Type<'db> {
        let previously_checking_unbound_typevars = self
            .context
            .inference_flags
            .replace(InferenceFlags::CHECK_UNBOUND_TYPEVARS, false);
        let vs = self.infer_type_expression(vs_node);
        self.context.inference_flags.set(
            InferenceFlags::CHECK_UNBOUND_TYPEVARS,
            previously_checking_unbound_typevars,
        );
        vs
    }

    /// Infer `GenericCallable[Vs, lambda <params>: <type>]` in a type expression (P5-14,
    /// P5-15, P5-30).
    ///
    /// Does not store the type of a tuple slice; the caller does.
    pub(super) fn infer_generic_callable_special_form(
        &mut self,
        subscript: &ast::ExprSubscript,
    ) -> Type<'db> {
        let arguments = match &*subscript.slice {
            ast::Expr::Tuple(tuple) => &*tuple.elts,
            slice => std::slice::from_ref(slice),
        };
        let [vs_node, body_node] = arguments else {
            if let Some((vs_node, rest)) = arguments.split_first() {
                self.infer_generic_callable_type_variables(vs_node);
                for argument in rest {
                    self.infer_type_expression(argument);
                }
            }
            report_invalid_argument_number_to_special_form(
                &self.context,
                subscript,
                SpecialFormType::GenericCallable,
                arguments.len(),
                2,
            );
            return Type::unknown();
        };

        let vs = self.infer_generic_callable_type_variables(vs_node);

        let ast::Expr::Lambda(lambda) = body_node else {
            self.infer_type_expression(body_node);
            self.report_invalid_generic_callable(
                body_node,
                "The second argument of `GenericCallable` must be a `lambda` whose parameters \
                 are the type parameters of the callable",
            );
            return Type::unknown();
        };

        // Rejected before any name in the lambda is looked up: its names belong to the lambda
        // scope that the semantic index created for it.
        if self.type_lambda_has_value_scope(lambda) {
            let ty = self.infer_lambda_expression(lambda, TypeContext::default());
            self.store_expression_type(body_node, ty);
            self.report_invalid_generic_callable(
                lambda,
                "`GenericCallable` is only supported in annotations, `type` statements and \
                 string annotations",
            );
            return Type::unknown();
        }

        let sources = self.generic_callable_type_variables(vs);
        let (binders, body, plain) =
            self.infer_type_lambda_parts(lambda, sources.as_deref(), false);

        let mut valid = true;
        if !self
            .type_expression_flags(ast::ExprRef::from(subscript))
            .contains(TypeExpressionFlags::GENERIC_CALLABLE_POSITION)
        {
            valid = false;
            self.report_invalid_generic_callable(
                subscript,
                "`GenericCallable` can only be used directly as the type argument of a `Member`",
            );
        }
        match &sources {
            Some(sources) => {
                if !plain || binders.len() != sources.len() {
                    valid = false;
                    let message = format!(
                        "The lambda of `GenericCallable` must have exactly one positional \
                         parameter without a default value for each type variable in its first \
                         argument (expected {}, got {})",
                        sources.len(),
                        lambda.parameters.as_deref().map_or(0, ast::Parameters::len),
                    );
                    let range = lambda
                        .parameters
                        .as_deref()
                        .map_or(lambda.range(), Ranged::range);
                    self.report_invalid_generic_callable(range, &message);
                }
            }
            None => {
                valid = false;
                if !vs.is_unknown() {
                    self.report_invalid_generic_callable(
                        vs_node,
                        "The first argument of `GenericCallable` must be a tuple of type \
                         variables, such as `tuple[T, U]`",
                    );
                }
            }
        }

        let ty = if valid && !body.is_unknown() {
            self.evaluate_type_operator(
                TypeOperatorKind::GenericCallable {
                    binders: binders.into_boxed_slice(),
                    body,
                },
                &*lambda.body,
            )
        } else {
            Type::unknown()
        };
        self.store_expression_type(body_node, ty);
        ty
    }
}
