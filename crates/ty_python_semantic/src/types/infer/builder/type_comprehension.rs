//! Type-expression inference for experimental [PEP 827] type comprehensions
//! (`*[body for x in Iter[T] if cond]`), dot access on `Member` types (`m.name`), and the
//! member-introspection and structural-synthesis special forms.
//!
//! Type comprehensions in annotations, `type` statements and string annotations do not create a
//! comprehension scope in the semantic index. They are inferred directly by the builder of the
//! enclosing type expression: the loop variable is a synthetic binder type variable, kept in
//! [`TypeInferenceBuilder::type_binders`], that shadows any place lookup of its name.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_python_ast::helpers::is_dotted_name;
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{self as ast};
use ruff_text_size::Ranged;
use ty_python_core::scope::NodeWithScopeRef;

use super::TypeInferenceBuilder;
use crate::types::diagnostic::{
    self, INVALID_TYPE_FORM, report_invalid_argument_number_to_special_form,
};
use crate::types::function::KnownFunction;
use crate::types::infer::TypeExpressionFlags;
use crate::types::member_introspection::member_args;
use crate::types::type_operator::{
    MemberField, Sequence, TypeOperatorKind, VariadicArg, comprehension_binder, evaluate_sequence,
    member_access, normalize_base,
};
use crate::types::{BoundTypeVarInstance, SpecialFormType, Type, TypeContext, TypeOperatorType};

/// The kind of element of a type comprehension.
#[derive(Clone, Copy)]
enum ComprehensionElement {
    /// The element of an unpacked list comprehension `*[elt for ...]`: a type expression,
    /// possibly `Unpack[...]`.
    Type,
    /// The element of the generator of `any(...)` (`all == false`) or `all(...)`: a type
    /// boolean.
    Quantifier { all: bool },
}

/// Collects every expression node of a comprehension target, in source order.
struct TargetExpressions<'a> {
    expressions: Vec<&'a ast::Expr>,
}

impl<'a> Visitor<'a> for TargetExpressions<'a> {
    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        self.expressions.push(expr);
        walk_expr(self, expr);
    }
}

/// Returns the leftmost name of a dotted name (`a` for `a.b.c`).
fn dotted_name_root(expression: &ast::Expr) -> Option<&ast::ExprName> {
    match expression {
        ast::Expr::Name(name) => Some(name),
        ast::Expr::Attribute(attribute) => dotted_name_root(&attribute.value),
        _ => None,
    }
}

impl<'db> TypeInferenceBuilder<'db, '_> {
    /// Returns the binder of the innermost enclosing type comprehension whose loop variable is
    /// named `name`.
    pub(super) fn type_binder(&self, name: &str) -> Option<BoundTypeVarInstance<'db>> {
        self.type_binders
            .iter()
            .rev()
            .find(|(binder_name, _)| binder_name.as_str() == name)
            .map(|(_, binder)| *binder)
    }

    /// Evaluates a PEP 827 type operator in an eager inference context, reporting an immediate
    /// evaluation failure as `invalid-type-form` at `node`.
    pub(super) fn evaluate_type_operator(
        &self,
        kind: TypeOperatorKind<'db>,
        node: impl Ranged,
    ) -> Type<'db> {
        TypeOperatorType::try_evaluate_or_defer(self.db(), self.program_environment(), kind)
            .unwrap_or_else(|error| {
                if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, node) {
                    diagnostic::add_type_expression_reference_link(
                        builder.into_diagnostic(error.message()),
                    );
                }
                Type::unknown()
            })
    }

    /// Returns `true` if the semantic index created a comprehension scope for `listcomp`.
    ///
    /// This is the case for list comprehensions in type expressions that are indexed as value
    /// expressions (implicit type aliases, `cast`, `TypeVar` bounds, ...). Such comprehensions
    /// are not supported as type comprehensions: their names belong to the comprehension scope.
    fn type_comprehension_has_value_scope(&self, listcomp: &ast::ExprListComp) -> bool {
        !self.in_string_annotation()
            && self
                .index
                .try_node_scope(NodeWithScopeRef::ListComprehension(listcomp))
                .is_some()
    }

    /// Infer `*[body for x in Iter[T] if cond ...]` in a variadic position of a type expression.
    pub(super) fn infer_unpacked_type_comprehension(
        &mut self,
        listcomp: &ast::ExprListComp,
    ) -> Type<'db> {
        if self.type_comprehension_has_value_scope(listcomp) {
            self.infer_list_comprehension_expression(listcomp, TypeContext::default());
            self.report_value_indexed_type_comprehension(listcomp);
            return Type::unknown();
        }

        match self.infer_type_comprehension_parts(
            &listcomp.elt,
            &listcomp.generators,
            ComprehensionElement::Type,
            true,
        ) {
            Some(kind) => self.evaluate_type_operator(kind, listcomp),
            None => Type::unknown(),
        }
    }

    /// Reports a type comprehension (or quantifier) in a position that the semantic index treats
    /// as a value expression (PEP 827 phase 2, rule 30).
    fn report_value_indexed_type_comprehension(&self, node: impl Ranged) {
        if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, node) {
            diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                "Type comprehensions are only supported in annotations, \
                `type` statements and string annotations",
            ));
        }
    }

    /// Infer a list comprehension that appears in a type expression without being unpacked.
    /// This is always an error; the sub-expressions are still inferred (exactly once).
    pub(super) fn infer_bare_list_comprehension_type_expression(
        &mut self,
        listcomp: &ast::ExprListComp,
    ) {
        if self.type_comprehension_has_value_scope(listcomp) {
            self.infer_list_comprehension_expression(listcomp, TypeContext::default());
        } else {
            self.infer_type_comprehension_parts(
                &listcomp.elt,
                &listcomp.generators,
                ComprehensionElement::Type,
                false,
            );
        }
    }

    /// Infer the PEP 827 quantifier `any(<type boolean> for x in Iter[T] ...)` or `all(...)` in
    /// a type expression. `generator` is the single argument of `call` (see
    /// [`type_quantifier_generator`](super::type_operator::type_quantifier_generator)).
    ///
    /// The semantic index gives such a generator no scope of its own in a type expression, so
    /// this is used for every call of this shape in a type expression, whatever its position.
    pub(super) fn infer_type_quantifier(
        &mut self,
        call: &ast::ExprCall,
        generator: &ast::ExprGenerator,
    ) -> Type<'db> {
        // Positions indexed as value expressions have a generator scope; infer them as values
        // before any name lookup (rule 30).
        if !self.in_string_annotation()
            && self
                .index
                .try_node_scope(NodeWithScopeRef::GeneratorExpression(generator))
                .is_some()
        {
            self.infer_call_expression(call, TypeContext::default());
            self.report_value_indexed_type_comprehension(call);
            return Type::unknown();
        }

        let db = self.db();
        let func = &*call.func;
        let all = func
            .as_name_expr()
            .is_some_and(|name| name.id.as_str() == "all");
        let (func_ty, _) = self.infer_type_expression_reference(func);
        self.finish_expression_type(func, func_ty, TypeContext::default());
        let expected = if all {
            KnownFunction::All
        } else {
            KnownFunction::Any
        };
        let is_builtin = func_ty
            .as_function_literal()
            .is_some_and(|function| function.is_known(db, expected));

        let kind = self.infer_type_comprehension_parts(
            &generator.elt,
            &generator.generators,
            ComprehensionElement::Quantifier { all },
            true,
        );
        let elements = match kind {
            Some(kind) if is_builtin => self.evaluate_type_operator(kind, generator),
            _ => Type::unknown(),
        };
        self.store_expression_type(&call.arguments.args[0], elements);

        if !is_builtin {
            if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, func) {
                let name = if all { "all" } else { "any" };
                diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                    format_args!("`{name}` in a type expression must be the builtin `{name}`"),
                ));
            }
            return Type::unknown();
        }
        if elements.is_unknown() {
            return Type::unknown();
        }
        self.evaluate_type_operator(TypeOperatorKind::Quantifier { all, elements }, call)
    }

    /// Infers every sub-expression of a type comprehension (the element `elt` and the `for`
    /// clauses `generators`) exactly once, and returns the `UnpackMap` operator it denotes, or
    /// `None` if the comprehension is invalid (in which case a diagnostic has been reported if
    /// `report` is `true`).
    ///
    /// Each `for` clause becomes an `UnpackMap` with the `if` clauses that follow it; the
    /// `UnpackMap` of a later clause is the (unpacked) body of the one before it. A later clause
    /// may refer to the loop variables of earlier ones, and shadows an earlier loop variable of
    /// the same name.
    fn infer_type_comprehension_parts(
        &mut self,
        elt: &ast::Expr,
        generators: &[ast::Comprehension],
        element: ComprehensionElement,
        report: bool,
    ) -> Option<TypeOperatorKind<'db>> {
        struct Clause<'db> {
            iterable: Option<Type<'db>>,
            binder: Option<BoundTypeVarInstance<'db>>,
            conditions: Vec<Type<'db>>,
        }

        let previous_binders = self.type_binders.len();
        let mut valid = true;
        let mut report_invalid = |builder: &Self, node: &dyn Ranged, message: &str| {
            valid = false;
            if report
                && let Some(diagnostic) = builder
                    .context
                    .report_lint(&INVALID_TYPE_FORM, node.range())
            {
                diagnostic::add_type_expression_reference_link(diagnostic.into_diagnostic(message));
            }
        };

        let mut clauses = Vec::with_capacity(generators.len());
        for generator in generators {
            let ast::Comprehension {
                range: _,
                node_index: _,
                target,
                iter,
                ifs,
                is_async,
            } = generator;

            if *is_async {
                report_invalid(
                    self,
                    &generator.range(),
                    "Type comprehensions cannot be asynchronous",
                );
            }
            let iterable = self.infer_type_comprehension_iterable(iter);
            if iterable.is_none() {
                report_invalid(
                    self,
                    iter,
                    "The iterable of a type comprehension must be `Iter[...]`",
                );
            }

            let binder = self.bind_type_comprehension_target(target);
            if binder.is_none() {
                report_invalid(
                    self,
                    target,
                    "The target of a type comprehension must be a single name",
                );
            }

            let mut conditions = Vec::with_capacity(ifs.len());
            for condition in ifs {
                match self.infer_type_bool_condition(condition) {
                    Some(ty) => conditions.push(ty),
                    None => report_invalid(
                        self,
                        condition,
                        "Condition of a type comprehension must be a type boolean",
                    ),
                }
            }
            clauses.push(Clause {
                iterable,
                binder,
                conditions,
            });
        }

        let body = match element {
            ComprehensionElement::Type => {
                let ty = self.infer_type_expression(elt);
                Some(self.variadic_type_argument(elt, ty))
            }
            ComprehensionElement::Quantifier { all } => {
                let ty = self.infer_type_bool_condition(elt);
                if ty.is_none() {
                    let name = if all { "all" } else { "any" };
                    report_invalid(
                        self,
                        elt,
                        &format!("The element of `{name}()` must be a type boolean"),
                    );
                }
                ty.map(VariadicArg::Single)
            }
        };
        self.type_binders.truncate(previous_binders);

        if !valid {
            return None;
        }
        let mut body = body?;
        for (index, clause) in clauses.into_iter().enumerate().rev() {
            let iterable = clause.iterable?;
            if iterable.is_unknown() || clause.conditions.iter().any(Type::is_unknown) {
                return None;
            }
            let kind = TypeOperatorKind::UnpackMap {
                iterable,
                binder: clause.binder?,
                body,
                conditions: clause.conditions.into_boxed_slice(),
            };
            if index == 0 {
                return Some(kind);
            }
            // The comprehension of a later clause usually depends on the loop variables of
            // earlier ones and is deferred; otherwise it is evaluated here.
            let inner = TypeOperatorType::try_evaluate_or_defer(
                self.db(),
                self.program_environment(),
                kind,
            );
            match inner {
                Ok(inner) if !inner.is_unknown() => body = VariadicArg::Unpacked(inner),
                Ok(_) => return None,
                Err(error) => {
                    if report
                        && let Some(builder) = self
                            .context
                            .report_lint(&INVALID_TYPE_FORM, &generators[index])
                    {
                        diagnostic::add_type_expression_reference_link(
                            builder.into_diagnostic(error.message()),
                        );
                    }
                    return None;
                }
            }
        }
        None
    }

    /// Infer the iterable of a type comprehension's `for` clause. Returns the type `T` if the
    /// iterable is `Iter[T]`, and `None` otherwise.
    ///
    /// An iterable that is not `Iter[...]` is inferred as a value expression (like other invalid
    /// type-expression syntax), so that only the missing `Iter` is reported.
    fn infer_type_comprehension_iterable(&mut self, iter: &ast::Expr) -> Option<Type<'db>> {
        if let ast::Expr::Subscript(subscript) = iter
            && is_dotted_name(&subscript.value)
            && self
                .speculate_without_diagnostics()
                .infer_type_expression_reference(&subscript.value)
                .0
                == Type::SpecialForm(SpecialFormType::Iter)
        {
            let (value_ty, _) = self.infer_type_expression_reference(&subscript.value);
            self.finish_expression_type(&subscript.value, value_ty, TypeContext::default());
            let ty = self
                .infer_type_operator_arguments(subscript, SpecialFormType::Iter, 1)
                .map_or_else(Type::unknown, |arguments| arguments[0]);
            if subscript.slice.is_tuple_expr() {
                self.store_expression_type(&subscript.slice, ty);
            }
            self.store_expression_type(iter, ty);
            return Some(ty);
        }
        if !self.in_string_annotation() {
            self.infer_expression(iter, TypeContext::default());
        }
        None
    }

    /// Stores the types of the comprehension target `target` and pushes a binder for every name
    /// it binds. Returns the binder if the target is a single name.
    fn bind_type_comprehension_target(
        &mut self,
        target: &ast::Expr,
    ) -> Option<BoundTypeVarInstance<'db>> {
        let db = self.db();
        let env = self.program_environment();
        let mut visitor = TargetExpressions {
            expressions: Vec::new(),
        };
        visitor.visit_expr(target);
        for expression in visitor.expressions {
            let ty = match expression {
                ast::Expr::Name(name) if name.ctx.is_store() => {
                    let binder = comprehension_binder(db, env, name.id.clone());
                    self.type_binders.push((name.id.clone(), binder));
                    Type::TypeVar(binder)
                }
                _ => Type::unknown(),
            };
            self.store_expression_type(expression, ty);
        }
        match target {
            ast::Expr::Name(_) => self.type_binders.last().map(|(_, binder)| *binder),
            _ => None,
        }
    }

    /// Infer the arguments of a fixed-arity PEP 827 special form as type expressions. Reports an
    /// arity error and returns `None` if the number of arguments is not `expected`.
    ///
    /// Does not store the type of a tuple slice; callers do.
    fn infer_type_operator_arguments(
        &mut self,
        subscript: &ast::ExprSubscript,
        special_form: SpecialFormType,
        expected: u8,
    ) -> Option<Vec<Type<'db>>> {
        self.infer_type_operator_arguments_with_classes(subscript, special_form, expected, &[])
    }

    /// Like [`Self::infer_type_operator_arguments`], but the arguments at `class_indexes` are
    /// inferred with [`Self::infer_type_operator_class_argument`].
    fn infer_type_operator_arguments_with_classes(
        &mut self,
        subscript: &ast::ExprSubscript,
        special_form: SpecialFormType,
        expected: u8,
        class_indexes: &[usize],
    ) -> Option<Vec<Type<'db>>> {
        let arguments_slice = &*subscript.slice;
        let arguments = if let ast::Expr::Tuple(tuple) = arguments_slice {
            &*tuple.elts
        } else {
            std::slice::from_ref(arguments_slice)
        };
        let types: Vec<Type<'db>> = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                if class_indexes.contains(&index) {
                    self.infer_type_operator_class_argument(argument)
                } else {
                    self.infer_type_expression(argument)
                }
            })
            .collect();
        if types.len() == usize::from(expected) {
            Some(types)
        } else {
            report_invalid_argument_number_to_special_form(
                &self.context,
                subscript,
                special_form,
                types.len(),
                expected,
            );
            None
        }
    }

    /// Infer the `T` or `Base` argument of `GetArg` / `GetArgs`, or an operand of
    /// `IsAssignable` / `IsEquivalent`. There, a bare generic class such as `list` or `Iterable`
    /// (or `Callable`) is intended: as `Base`, only the class matters, and otherwise it stands for
    /// its default specialization (P4). So no missing type arguments are reported for it; any
    /// other argument is an ordinary type expression.
    pub(super) fn infer_type_operator_class_argument(&mut self, argument: &ast::Expr) -> Type<'db> {
        let is_reference = is_dotted_name(argument)
            && dotted_name_root(argument).is_some_and(|root| self.type_binder(&root.id).is_none());
        let db = self.db();
        let is_bare_generic = is_reference
            && match self
                .speculate_without_diagnostics()
                .infer_type_expression_reference(argument)
                .0
            {
                Type::ClassLiteral(class) => class.generic_context(db).is_some(),
                Type::SpecialForm(
                    SpecialFormType::TypingCallable | SpecialFormType::CollectionsAbcCallable,
                ) => true,
                _ => false,
            };
        if !is_bare_generic {
            return self.infer_type_expression(argument);
        }
        let (value_ty, _) = self.infer_type_expression_reference(argument);
        let ty = value_ty
            .default_specialize(db, self.program_environment())
            .in_type_expression(
                db,
                self.scope(),
                self.typevar_binding_context,
                self.inference_flags(),
            )
            .unwrap_or_else(|error| {
                error.into_fallback_type(&self.context, argument, self.inference_flags())
            });
        self.store_expression_type(argument, ty);
        ty
    }

    /// Infer a subscripted `Attrs`, `Members`, `GetMember`, `GetMemberType`, `FromUnion`,
    /// `NewProtocol`, `NewTypedDict` or `Iter` in a type expression.
    pub(super) fn infer_type_operator_special_form_type_expression(
        &mut self,
        subscript: &ast::ExprSubscript,
        special_form: SpecialFormType,
    ) -> Type<'db> {
        let ty = self.infer_type_operator_special_form(subscript, special_form);
        if subscript.slice.is_tuple_expr() {
            self.store_expression_type(&subscript.slice, ty);
        }
        ty
    }

    fn infer_type_operator_special_form(
        &mut self,
        subscript: &ast::ExprSubscript,
        special_form: SpecialFormType,
    ) -> Type<'db> {
        match special_form {
            SpecialFormType::Iter => {
                self.infer_type_operator_arguments(subscript, special_form, 1);
                if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, subscript) {
                    diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                        "`Iter` can only be used as the iterable of a type comprehension",
                    ));
                }
                Type::unknown()
            }
            SpecialFormType::Attrs | SpecialFormType::Members | SpecialFormType::FromUnion => {
                let Some(arguments) =
                    self.infer_type_operator_arguments(subscript, special_form, 1)
                else {
                    return Type::unknown();
                };
                let operand = arguments[0];
                let kind = match special_form {
                    SpecialFormType::Attrs => TypeOperatorKind::Attrs(operand),
                    SpecialFormType::Members => TypeOperatorKind::Members(operand),
                    _ => TypeOperatorKind::FromUnion(operand),
                };
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::GetMember | SpecialFormType::GetMemberType => {
                let Some(arguments) =
                    self.infer_type_operator_arguments(subscript, special_form, 2)
                else {
                    return Type::unknown();
                };
                let (ty, name) = (arguments[0], arguments[1]);
                let kind = if special_form == SpecialFormType::GetMember {
                    TypeOperatorKind::GetMember(ty, name)
                } else {
                    TypeOperatorKind::GetMemberType(ty, name)
                };
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::NewProtocol | SpecialFormType::NewTypedDict => {
                let arguments_slice = &*subscript.slice;
                let arguments = if let ast::Expr::Tuple(tuple) = arguments_slice {
                    &*tuple.elts
                } else {
                    std::slice::from_ref(arguments_slice)
                };
                let arguments = self.infer_variadic_type_arguments(arguments);
                // An `Unknown` argument stems from an invalid type expression (e.g. a failed type
                // comprehension) that has already been reported.
                if arguments.iter().any(|argument| {
                    matches!(argument, VariadicArg::Single(ty) | VariadicArg::Unpacked(ty)
                        if ty.is_unknown())
                }) {
                    return Type::unknown();
                }
                let kind = if special_form == SpecialFormType::NewProtocol {
                    TypeOperatorKind::NewProtocol(arguments)
                } else {
                    TypeOperatorKind::NewTypedDict(arguments)
                };
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::GetArg | SpecialFormType::GetArgs => {
                let expected = if special_form == SpecialFormType::GetArg {
                    3
                } else {
                    2
                };
                let Some(arguments) = self.infer_type_operator_arguments_with_classes(
                    subscript,
                    special_form,
                    expected,
                    &[0, 1],
                ) else {
                    return Type::unknown();
                };
                let base_node = match &*subscript.slice {
                    ast::Expr::Tuple(tuple) => &tuple.elts[1],
                    _ => &*subscript.slice,
                };
                let Some(base) =
                    self.normalize_type_operator_base(special_form, arguments[1], base_node)
                else {
                    return Type::unknown();
                };
                let kind = if special_form == SpecialFormType::GetArg {
                    TypeOperatorKind::GetArg(arguments[0], base, arguments[2])
                } else {
                    TypeOperatorKind::GetArgs(arguments[0], base)
                };
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::Length => {
                let Some(arguments) =
                    self.infer_type_operator_arguments(subscript, special_form, 1)
                else {
                    return Type::unknown();
                };
                self.evaluate_type_operator(TypeOperatorKind::Length(arguments[0]), subscript)
            }
            SpecialFormType::Slice => {
                let Some(arguments) =
                    self.infer_type_operator_arguments(subscript, special_form, 3)
                else {
                    return Type::unknown();
                };
                let kind = TypeOperatorKind::Slice(arguments[0], arguments[1], arguments[2]);
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::GetSpecialAttr => {
                let Some(arguments) =
                    self.infer_type_operator_arguments(subscript, special_form, 2)
                else {
                    return Type::unknown();
                };
                let kind = TypeOperatorKind::GetSpecialAttr(arguments[0], arguments[1]);
                self.evaluate_type_operator(kind, subscript)
            }
            SpecialFormType::RaiseError => {
                let arguments_slice = &*subscript.slice;
                let arguments = if let ast::Expr::Tuple(tuple) = arguments_slice {
                    &*tuple.elts
                } else {
                    std::slice::from_ref(arguments_slice)
                };
                let arguments = self.infer_variadic_type_arguments(arguments);
                let Some((message, args)) = arguments.split_first() else {
                    if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, subscript) {
                        builder.into_diagnostic(
                            "Special form `RaiseError` expected at least 1 type argument, got 0",
                        );
                    }
                    return Type::unknown();
                };
                let VariadicArg::Single(message) = *message else {
                    if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, subscript) {
                        diagnostic::add_type_expression_reference_link(
                            builder
                                .into_diagnostic("The message of `RaiseError` cannot be unpacked"),
                        );
                    }
                    return Type::unknown();
                };
                if message.is_unknown() || args.iter().any(|arg| arg.ty().is_unknown()) {
                    return Type::unknown();
                }
                let kind = TypeOperatorKind::RaiseError {
                    message,
                    args: args.into(),
                };
                self.evaluate_type_operator(kind, subscript)
            }
            _ => unreachable!("not a PEP 827 type-operator special form: {special_form:?}"),
        }
    }

    /// Normalizes the `Base` argument of `GetArg` / `GetArgs` (see [`normalize_base`]), reporting
    /// an argument that does not denote a class at `node`.
    fn normalize_type_operator_base(
        &self,
        special_form: SpecialFormType,
        base: Type<'db>,
        node: &ast::Expr,
    ) -> Option<Type<'db>> {
        let normalized = normalize_base(self.db(), self.program_environment(), base);
        if normalized.is_none()
            && let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, node)
        {
            diagnostic::add_type_expression_reference_link(builder.into_diagnostic(format_args!(
                "The `Base` argument of `{}` must be a class",
                special_form.name()
            )));
        }
        normalized
    }

    /// Infer the arguments of a variadic position (`NewProtocol[...]`, `NewTypedDict[...]`),
    /// where arguments may be unpacked with `*`. An invalid unpack (already reported) becomes
    /// `Unknown`.
    fn infer_variadic_type_arguments(
        &mut self,
        arguments: &[ast::Expr],
    ) -> Box<[VariadicArg<'db>]> {
        arguments
            .iter()
            .map(|argument| {
                let ty = self.infer_type_expression(argument);
                if self
                    .type_expression_flags(argument)
                    .contains(TypeExpressionFlags::INVALID_UNPACK)
                {
                    VariadicArg::Single(Type::unknown())
                } else {
                    self.variadic_type_argument(argument, ty)
                }
            })
            .collect()
    }

    /// Classifies an inferred argument of a variadic position as a single or unpacked argument.
    /// An invalid unpack (already reported) is kept as a single argument of its recovery type.
    pub(super) fn variadic_type_argument(
        &self,
        argument: &ast::Expr,
        ty: Type<'db>,
    ) -> VariadicArg<'db> {
        let flags = self.type_expression_flags(argument);
        if flags.contains(TypeExpressionFlags::UNPACK)
            && !flags.contains(TypeExpressionFlags::INVALID_UNPACK)
        {
            VariadicArg::Unpacked(ty)
        } else {
            VariadicArg::Single(ty)
        }
    }

    /// Returns `true` if `ty`, the result of an unpacked type expression `*ty` that is neither a
    /// tuple nor a `TypeVarTuple`, denotes a sequence of types that is not known yet: a deferred
    /// PEP 827 operator, or (while a recursive alias such as
    /// `type Zip[T, S] = ... tuple[*Zip[...], ...]` is being defined) a cycle marker or a union
    /// of deferred operators. Such an unpack is kept as an unresolved `tuple[*...]` operator.
    pub(super) fn is_unpackable_type_operator(&self, ty: Type<'db>) -> bool {
        !matches!(ty, Type::Dynamic(_))
            && !matches!(
                evaluate_sequence(self.db(), self.program_environment(), ty),
                Sequence::Error
            )
    }

    /// Infer `m.name`, `m.type`, `m.quals`, `m.init` or `m.definer` in a type expression, where
    /// `m` describes a `Member`. Returns `None` if the attribute access is not a `Member` access;
    /// the caller then infers it as an ordinary dotted name. In that case, if `Some(value_ty)` is
    /// returned in `inferred_value`, the value of the attribute has already been inferred (as a
    /// value expression) and stored.
    ///
    /// A dotted name is treated as a `Member` access if its left-hand side denotes a `Member`, a
    /// type variable, a union, `Any` or `Never` in a type expression, or if the attribute is a
    /// `Member` component that the left-hand side does not provide as a valid type form (so that
    /// `int.type` is reported as an invalid dot access while ordinary dotted names such as
    /// `typing.List` or `Outer.Inner` keep their meaning).
    pub(super) fn infer_member_access_type_expression(
        &mut self,
        attribute: &ast::ExprAttribute,
        inferred_value: &mut Option<Type<'db>>,
    ) -> Option<Type<'db>> {
        if !attribute.ctx.is_load() {
            return None;
        }
        let db = self.db();
        let env = self.program_environment();
        let value = &*attribute.value;
        let field = MemberField::from_name(attribute.attr.as_str());

        let root_is_binder =
            dotted_name_root(value).is_some_and(|root| self.type_binder(&root.id).is_some());

        let left = if root_is_binder || !is_dotted_name(value) {
            if field.is_none() && !root_is_binder {
                return None;
            }
            // A loop variable of a type comprehension (possibly with further member accesses),
            // whose attributes are never looked up as places, or a type expression such as
            // `GetMember[T, Literal["x"]]`.
            self.infer_type_expression(value)
        } else {
            let value_ty = self.infer_maybe_standalone_expression(value, TypeContext::default());
            *inferred_value = Some(value_ty);
            let as_type = value_ty
                .default_specialize(db, env)
                .in_type_expression(
                    db,
                    self.scope(),
                    self.typevar_binding_context,
                    self.inference_flags(),
                )
                .ok()?
                .resolve_type_alias(db);
            let intercept = match field {
                None => member_args(db, env, as_type).is_some(),
                Some(_) => match as_type {
                    Type::Dynamic(_) | Type::Never | Type::TypeOperator(_) | Type::Union(_) => true,
                    Type::TypeVar(typevar) => {
                        !typevar.is_paramspec(db) && !typevar.is_typevartuple(db)
                    }
                    _ => {
                        member_args(db, env, as_type).is_some()
                            || !self.is_type_form_attribute(value_ty, &attribute.attr.id)
                    }
                },
            };
            if !intercept {
                return None;
            }
            as_type
        };

        let Some(field) = field else {
            if let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, attribute) {
                diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                    format_args!(
                        "`{}` is not a component of `Member`; expected one of `name`, `type`, \
                        `quals`, `init` or `definer`",
                        attribute.attr.id,
                    ),
                ));
            }
            return Some(Type::unknown());
        };

        Some(match member_access(db, env, left, field) {
            Ok(ty) => ty,
            Err(()) => {
                if !left.is_unknown()
                    && let Some(builder) = self.context.report_lint(&INVALID_TYPE_FORM, attribute)
                {
                    diagnostic::add_type_expression_reference_link(builder.into_diagnostic(
                        format_args!(
                            "`.{}` in a type expression requires a `Member` type, not `{}`",
                            attribute.attr.id,
                            left.display(db, env),
                        ),
                    ));
                }
                Type::unknown()
            }
        })
    }

    /// Returns `true` if the attribute `name` of a value of type `value_ty` is valid in a type
    /// expression (e.g. a nested class).
    fn is_type_form_attribute(&self, value_ty: Type<'db>, name: &str) -> bool {
        let db = self.db();
        value_ty
            .member(db, self.program_environment(), name)
            .place
            .ignore_possibly_undefined()
            .is_some_and(|ty| {
                ty.default_specialize(db, self.program_environment())
                    .in_type_expression(
                        db,
                        self.scope(),
                        self.typevar_binding_context,
                        self.inference_flags(),
                    )
                    .is_ok()
            })
    }
}
