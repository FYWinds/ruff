//! Experimental support for [PEP 827] class updates (`UpdateClass`).
//!
//! A class `C` is updated by:
//!
//! - every ancestor whose own `__init_subclass__` returns `UpdateClass[...]`, starting from the
//!   most basic one (reverse MRO order, as the PEP specifies; the reference implementation uses
//!   MRO order instead). The class that defines the hook is not updated by it;
//! - then every class decorator whose return type is `UpdateClass[...]`, from the innermost one.
//!   Decorators outside of the first decorator that replaces the class binding are ordinary
//!   calls (see `infer_class_definition`).
//!
//! Each update is one *step* of `C` ([`class_update_step`]). It is evaluated without binding a
//! call: if the first parameter of the hook is annotated as `type[T]`, `T` is replaced by the
//! inert [`TypeOperatorKind::ClassView`] of `C` at that step, so that `Attrs` / `Members` of it
//! only see the earlier steps of `C`, and the step does not depend on itself. The type
//! parameters of an ancestor are replaced according to `C`'s MRO. Any `ClassView` left in the
//! result is replaced by `C` itself.
//!
//! Updates of different classes that inspect each other form a Salsa cycle. Every step in such a
//! cycle falls back to an empty update (independently of the evaluation order) and is reported
//! where the class is checked, as is a `RaiseError` that a step selects.
//!
//! The updated members override everything in the class body, including synthesized members, in
//! the member lookup hooks of [`StaticClassLiteral`], and are merged into the class's own members
//! by member introspection (`Attrs` / `Members`). A member whose type is `Never` removes the
//! member: it is declared as `Never` on the class and no longer introspected.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use itertools::Itertools;
use ruff_python_ast::{self as ast, name::Name};
use ruff_text_size::{Ranged, TextRange};
use ty_python_core::place_table;

use crate::place::Place;
use crate::types::context::InferContext;
use crate::types::diagnostic::{INVALID_TYPE_FORM, report_raised_type_errors};
use crate::types::function::FunctionType;
use crate::types::member::{Member, class_member};
use crate::types::member_introspection::{is_method_like_type, member_args, parse_qualifiers};
use crate::types::signatures::Signature;
use crate::types::type_operator::{
    Sequence, TypeOperatorError, TypeOperatorKind, VariadicArg, as_raised_error, evaluate_sequence,
    find_raised_errors, needs_deferral, strip_annotated,
};
use crate::types::visitor::any_over_type_including_alias_arguments;
use crate::types::{
    BoundTypeVarInstance, ClassBase, ClassType, GenericContext, KnownClass, StaticClassLiteral,
    Type, TypeContext, TypeMapping, TypeOperatorType, generics::Specialization,
};
use crate::{Db, FxIndexMap, ProgramEnvironment, TypeQualifiers};

/// A class decorator whose return type is `UpdateClass[...]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub(crate) struct UpdateClassDecorator<'db> {
    /// The position of the decorator in the class's decorator list.
    pub(crate) index: u32,
    pub(crate) function: FunctionType<'db>,
}

/// Returns the function `decorator` if it is a function whose return type is `UpdateClass[...]`.
pub(crate) fn update_class_decorator<'db>(
    db: &'db dyn Db,
    decorator: Type<'db>,
) -> Option<FunctionType<'db>> {
    match decorator {
        Type::FunctionLiteral(function) if returns_update_class(db, function) => Some(function),
        _ => None,
    }
}

/// Returns `true` if `function` has a single signature whose return type is `UpdateClass[...]`.
fn returns_update_class<'db>(db: &'db dyn Db, function: FunctionType<'db>) -> bool {
    function
        .signature(db)
        .iter()
        .exactly_one()
        .is_ok_and(|signature| is_update_class(db, signature.return_ty))
}

fn is_update_class<'db>(db: &'db dyn Db, ty: Type<'db>) -> bool {
    matches!(
        ty.resolve_type_alias(db),
        Type::TypeOperator(operator) if matches!(operator.kind(db), TypeOperatorKind::UpdateClass(_))
    )
}

/// Evaluates `UpdateClass[*args]`: once every argument is known to be a `Member` with a string
/// literal name, the result is the inert `UpdateClass` operator with one
/// [`VariadicArg::Single`] argument per member (whose other arguments may still contain type
/// variables). Otherwise, it is deferred.
pub(crate) fn evaluate_update_class<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    kind: &TypeOperatorKind<'db>,
    args: &[VariadicArg<'db>],
) -> Result<Type<'db>, TypeOperatorError> {
    let mut members = Vec::with_capacity(args.len());
    let mut deferred = false;
    for arg in args {
        let elements = match *arg {
            VariadicArg::Single(ty) => vec![ty],
            VariadicArg::Unpacked(ty) => match evaluate_sequence(db, env, ty) {
                Sequence::Fixed(elements) => elements,
                Sequence::Unbounded { .. } => {
                    return Err(TypeOperatorError::new(
                        "Cannot update a class with an unbounded number of members",
                    ));
                }
                Sequence::Dynamic(dynamic) => return Ok(dynamic),
                Sequence::Deferred => {
                    deferred = true;
                    continue;
                }
                Sequence::Error => {
                    return Err(TypeOperatorError::new(format!(
                        "Cannot unpack `{}` in `UpdateClass`",
                        ty.display(db, env)
                    )));
                }
            },
        };
        for element in elements {
            let resolved = element.resolve_type_alias(db);
            if let Some(raised) = as_raised_error(db, resolved) {
                return Ok(raised);
            }
            if resolved.is_dynamic() {
                return Ok(resolved);
            }
            match member_args(db, env, resolved) {
                Some([name, ..]) if name.as_string_literal().is_some() => {
                    members.push(VariadicArg::Single(resolved));
                }
                Some([name, ..]) if needs_deferral(db, env, name) => deferred = true,
                Some([name, ..]) => {
                    return Err(TypeOperatorError::new(format!(
                        "The name of a member passed to `UpdateClass` must be a string literal \
                         type, but got `{}`",
                        name.display(db, env)
                    )));
                }
                None if needs_deferral(db, env, resolved) => deferred = true,
                None => {
                    return Err(TypeOperatorError::new(format!(
                        "The arguments to `UpdateClass` must be `Member` types, but got `{}`",
                        element.display(db, env)
                    )));
                }
            }
        }
    }
    let kind = if deferred {
        kind.clone()
    } else {
        TypeOperatorKind::UpdateClass(members.into_boxed_slice())
    };
    Ok(Type::TypeOperator(TypeOperatorType::new(db, kind)))
}

/// Where the updates of a class come from, in the order in which they are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
enum UpdateSource<'db> {
    /// The `__init_subclass__` defined in the body of `ancestor` (as specialized in the MRO of
    /// the updated class).
    InitSubclass {
        ancestor: ClassType<'db>,
        hook: FunctionType<'db>,
    },
    /// A class decorator.
    Decorator(UpdateClassDecorator<'db>),
}

impl<'db> UpdateSource<'db> {
    const fn function(self) -> FunctionType<'db> {
        match self {
            Self::InitSubclass { hook, .. } => hook,
            Self::Decorator(decorator) => decorator.function,
        }
    }
}

/// The sources of the updates of `class`, in the order in which they are applied.
#[salsa::tracked(
    returns(deref),
    cycle_initial=|_, _, _| Box::default(),
    heap_size=ruff_memory_usage::heap_size,
)]
fn update_sources<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
) -> Box<[UpdateSource<'db>]> {
    let ancestors: Vec<ClassType<'db>> = class
        .iter_mro(db, None)
        .skip(1)
        .filter_map(ClassBase::into_class)
        .collect();
    ancestors
        .into_iter()
        .rev()
        .filter_map(|ancestor| {
            let (literal, _) = ancestor.static_class_literal(db)?;
            let hook = own_update_hook(db, literal)?;
            Some(UpdateSource::InitSubclass { ancestor, hook })
        })
        .chain(
            class
                .update_class_decorators(db)
                .iter()
                .copied()
                .map(UpdateSource::Decorator),
        )
        .collect()
}

/// Returns the `__init_subclass__` defined in the body of `class` if it returns `UpdateClass`.
#[salsa::tracked(
    returns(copy),
    cycle_initial=|_, _, _| None,
    heap_size=ruff_memory_usage::heap_size,
)]
fn own_update_hook<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
) -> Option<FunctionType<'db>> {
    if class.is_known(db, KnownClass::Object) {
        return None;
    }
    let scope = class.body_scope(db);
    place_table(db, scope).symbol_id("__init_subclass__")?;
    match class_member(db, scope, "__init_subclass__").ignore_possibly_undefined()? {
        Type::FunctionLiteral(function) if returns_update_class(db, function) => Some(function),
        _ => None,
    }
}

/// Returns `true` if any `UpdateClass` hook or decorator applies to `class`.
pub(crate) fn has_class_updates(db: &dyn Db, class: StaticClassLiteral<'_>) -> bool {
    !update_sources(db, class).is_empty()
}

/// The number of updates (steps) of `class`.
pub(crate) fn class_update_count(db: &dyn Db, class: StaticClassLiteral<'_>) -> u32 {
    u32::try_from(update_sources(db, class).len()).unwrap_or(u32::MAX)
}

/// A member added, replaced or removed (`ty` is `Never`) by an update.
#[derive(Debug, Clone, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub(crate) struct UpdatedMember<'db> {
    pub(crate) name: Name,
    pub(crate) ty: Type<'db>,
    pub(crate) qualifiers: TypeQualifiers,
    pub(crate) init: Type<'db>,
}

impl<'db> UpdatedMember<'db> {
    /// Returns `true` if this member removes the member with its name.
    pub(crate) const fn is_removal(&self) -> bool {
        matches!(self.ty, Type::Never)
    }

    /// Returns `true` if this member is a method: a `ClassVar` with a callable type.
    pub(crate) fn is_method(&self) -> bool {
        self.qualifiers.contains(TypeQualifiers::CLASS_VAR) && is_method_like_type(self.ty)
    }

    /// The type of the member as seen in the class namespace: a callable method binds its
    /// receiver like a function.
    fn class_namespace_type(&self, db: &'db dyn Db) -> Type<'db> {
        match self.ty {
            Type::Callable(callable) if self.is_method() && callable.is_regular(db) => {
                Type::Callable(callable.into_function_like(db))
            }
            ty => ty,
        }
    }
}

/// The result of one update of a class.
#[derive(Debug, Default, Clone, PartialEq, Eq, get_size2::GetSize, salsa::SalsaValue)]
pub(crate) struct ClassUpdateStep<'db> {
    /// The updated members, in the order of the update (without duplicate names).
    pub(crate) members: Box<[UpdatedMember<'db>]>,
    /// Types that contain the `RaiseError`s selected by the update, to report.
    raised: Box<[Type<'db>]>,
    /// The update depends on itself through the update of another class; it is empty.
    cycle: bool,
}

impl<'db> ClassUpdateStep<'db> {
    fn cycle() -> Self {
        Self {
            cycle: true,
            ..Self::default()
        }
    }

    /// Reads the members of the evaluated (specialized) return type of an update hook.
    fn from_update(db: &'db dyn Db, env: &ProgramEnvironment<'db>, update: Type<'db>) -> Self {
        let update = update.resolve_type_alias(db);
        let args = match update {
            Type::TypeOperator(operator) => match operator.kind(db) {
                TypeOperatorKind::UpdateClass(args) => args,
                _ => return Self::default(),
            },
            // A `RaiseError` that replaces the whole update.
            _ if !find_raised_errors(db, env, update).is_empty() => {
                return Self {
                    raised: Box::new([update]),
                    ..Self::default()
                };
            }
            _ => return Self::default(),
        };
        let mut members: FxIndexMap<Name, UpdatedMember<'db>> = FxIndexMap::default();
        let mut raised = Vec::new();
        for arg in args {
            // An update whose members are not known yet (for example, because they depend on
            // the type parameters of a generic class) is not applied.
            let VariadicArg::Single(member) = *arg else {
                return Self::default();
            };
            let Some([name, ty, quals, init, _]) = member_args(db, env, member) else {
                return Self::default();
            };
            let Some(name) = name.as_string_literal() else {
                return Self::default();
            };
            let mut ty = resolve_views(db, env, strip_annotated(db, ty));
            if !find_raised_errors(db, env, ty).is_empty() {
                // P5-33: the member that failed is `Unknown`.
                raised.push(ty);
                ty = Type::unknown();
            }
            let name = Name::new(name.value(db));
            members.insert(
                name.clone(),
                UpdatedMember {
                    name,
                    ty,
                    qualifiers: parse_qualifiers(db, quals).unwrap_or_default(),
                    init: resolve_views(db, env, init),
                },
            );
        }
        Self {
            members: members.into_values().collect(),
            raised: raised.into_boxed_slice(),
            cycle: false,
        }
    }
}

/// Replaces every [`TypeOperatorKind::ClassView`] in `ty` with the instance type of its class,
/// and every [`TypeOperatorKind::ViewTypeVar`] with its type variable.
fn resolve_views<'db>(db: &'db dyn Db, env: &ProgramEnvironment<'db>, ty: Type<'db>) -> Type<'db> {
    let has_view = any_over_type_including_alias_arguments(db, env, ty, |ty| {
        matches!(
            ty,
            Type::TypeOperator(operator) if matches!(
                operator.kind(db),
                TypeOperatorKind::ClassView { .. } | TypeOperatorKind::ViewTypeVar(_)
            )
        )
    });
    if has_view {
        ty.apply_type_mapping(
            db,
            env,
            &TypeMapping::ResolveClassViews,
            TypeContext::default(),
        )
    } else {
        ty
    }
}

/// The specialization of a generic class that is being updated, as seen by the update: each of
/// its (non-variadic) type parameters is a fixed [`TypeOperatorKind::ViewTypeVar`], so that the
/// update can be evaluated (and is not deferred) for the class with its own type parameters.
fn view_specialization<'db>(db: &'db dyn Db, context: GenericContext<'db>) -> Specialization<'db> {
    let types: Vec<Type<'db>> = context
        .variables(db)
        .map(|typevar| {
            if typevar.is_typevartuple(db) || typevar.is_paramspec(db) {
                Type::TypeVar(typevar)
            } else {
                Type::TypeOperator(TypeOperatorType::new(
                    db,
                    TypeOperatorKind::ViewTypeVar(typevar),
                ))
            }
        })
        .collect();
    context.specialize(db, types)
}

/// The class of a [`TypeOperatorKind::ClassView`] as seen by member introspection and the other
/// operators: a generic class is specialized with [`view_specialization`].
pub(crate) fn view_class<'db>(db: &'db dyn Db, class: ClassType<'db>) -> ClassType<'db> {
    let Some((literal, _)) = class.static_class_literal(db) else {
        return class;
    };
    if literal.generic_context(db).is_none() {
        return class;
    }
    literal.apply_specialization(db, |context| view_specialization(db, context))
}

/// Returns the type variable `T` of the first parameter `cls: type[T]` of `signature`, if `T`
/// is a type parameter of the signature itself (P5-25).
fn class_type_var<'db>(
    db: &'db dyn Db,
    signature: &Signature<'db>,
) -> Option<BoundTypeVarInstance<'db>> {
    let first = signature.parameters().iter().next()?;
    let Type::SubclassOf(subclass_of) = first.annotated_type() else {
        return None;
    };
    let typevar = subclass_of.subclass_of().into_type_var()?;
    let identity = typevar.identity(db);
    signature
        .generic_context?
        .variables(db)
        .any(|variable| variable.identity(db) == identity)
        .then_some(typevar)
}

/// Evaluates the update number `step` of `class`.
///
/// Updates of different classes that depend on each other form a cycle: every step in it falls
/// back to an empty update, whatever the order of evaluation (P5-29).
#[salsa::tracked(
    returns(ref),
    cycle_result=|_, _, _, _| ClassUpdateStep::cycle(),
    heap_size=ruff_memory_usage::heap_size,
)]
pub(crate) fn class_update_step<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    step: u32,
) -> ClassUpdateStep<'db> {
    let env = ProgramEnvironment::from_scope(class.body_scope(db));
    let Some(source) = update_sources(db, class).get(step as usize).copied() else {
        return ClassUpdateStep::default();
    };
    let Ok(signature) = source.function().signature(db).iter().exactly_one() else {
        return ClassUpdateStep::default();
    };
    // The type parameters of the ancestor that defines the hook, as specialized by `class`
    // (P5-25), with the type parameters of `class` itself seen as fixed types.
    let owner_specialization = match source {
        UpdateSource::InitSubclass { ancestor, .. } => {
            ancestor.static_class_literal(db).and_then(|(_, spec)| spec)
        }
        UpdateSource::Decorator(_) => None,
    };
    let mut update = signature
        .return_ty
        .apply_optional_specialization(db, owner_specialization);
    if let Some(context) = class.generic_context(db) {
        update = update.apply_specialization(db, view_specialization(db, context));
    }
    if let Some(typevar) = class_type_var(db, signature) {
        let view = Type::TypeOperator(TypeOperatorType::new(
            db,
            TypeOperatorKind::ClassView {
                class: class.identity_specialization(db),
                step,
            },
        ));
        let specialization =
            GenericContext::from_typevar_instances(db, &env, [typevar]).specialize(db, vec![view]);
        update = update.apply_specialization(db, specialization);
    }
    ClassUpdateStep::from_update(db, &env, update)
}

/// The latest update of the member `name` of `class`, if any.
fn updated_member<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    name: &str,
) -> Option<&'db UpdatedMember<'db>> {
    (0..class_update_count(db, class)).rev().find_map(|step| {
        class_update_step(db, class, step)
            .members
            .iter()
            .find(|member| member.name == name)
    })
}

/// The member `name` of the namespace of `class` if an update of `class` declares it.
pub(super) fn own_updated_class_member<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    name: &str,
) -> Option<Member<'db>> {
    let member = updated_member(db, class, name)?;
    Some(Member {
        inner: Place::declared(member.class_namespace_type(db)).with_qualifiers(member.qualifiers),
    })
}

/// The instance attribute `name` of `class` if an update of `class` declares it. A `ClassVar`
/// (including a method) is not an instance attribute.
pub(super) fn own_updated_instance_member<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    name: &str,
) -> Option<Member<'db>> {
    let member = updated_member(db, class, name)?;
    Some(if member.qualifiers.contains(TypeQualifiers::CLASS_VAR) {
        Member::unbound()
    } else {
        Member {
            inner: Place::declared(member.ty).with_qualifiers(member.qualifiers),
        }
    })
}

/// Reports the updates of `class` that depend on themselves through another class (P5-29), and
/// the `RaiseError`s selected by its updates (P5-33). An update from a decorator is reported on
/// the decorator; one from `__init_subclass__`, on the base class through which the hook is
/// inherited.
pub(crate) fn report_class_update_errors<'db>(
    context: &InferContext<'db, '_>,
    class: StaticClassLiteral<'db>,
    class_node: &ast::StmtClassDef,
) {
    let db = context.db();
    let sources = update_sources(db, class);
    for (step, source) in (0..).zip(sources) {
        let result = class_update_step(db, class, step);
        if !result.cycle && result.raised.is_empty() {
            continue;
        }
        let range = source_range(db, class, class_node, *source);
        if result.cycle
            && let Some(builder) = context.report_lint(&INVALID_TYPE_FORM, range)
        {
            builder.into_diagnostic(format_args!(
                "UpdateClass evaluation depends on itself: the update of `{}` inspects a class \
                 whose update inspects `{}`",
                class.name(db),
                class.name(db),
            ));
        }
        if !result.raised.is_empty() {
            let raised = Type::heterogeneous_tuple(
                db,
                context.program_environment(),
                result.raised.iter().copied(),
            );
            report_raised_type_errors(context, range, raised);
        }
    }
}

fn source_range<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    class_node: &ast::StmtClassDef,
    source: UpdateSource<'db>,
) -> TextRange {
    match source {
        UpdateSource::Decorator(decorator) => class_node
            .decorator_list
            .get(decorator.index as usize)
            .map_or_else(|| class_node.name.range(), |node| node.expression.range()),
        UpdateSource::InitSubclass { ancestor, .. } => {
            let ancestor = ancestor.class_literal(db);
            let bases = class_node.bases();
            let explicit_bases = class.explicit_bases(db);
            if bases.len() != explicit_bases.len() {
                return class_node.name.range();
            }
            bases
                .iter()
                .zip(explicit_bases)
                .find(|(_, base)| {
                    base.to_class_type(db).is_some_and(|base| {
                        base.iter_mro(db).any(|entry| {
                            entry
                                .into_class()
                                .is_some_and(|entry| entry.class_literal(db) == ancestor)
                        })
                    })
                })
                .map_or_else(|| class_node.name.range(), |(node, _)| node.range())
        }
    }
}
