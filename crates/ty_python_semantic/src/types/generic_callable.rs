//! Experimental support for [PEP 827] generic callables:
//! `GenericCallable[tuple[V1, ...], lambda T1, ...: Ty]`.
//!
//! Each lambda parameter is a new type variable ([`generic_callable_binder`]) that takes the bound
//! or constraints and the default of the corresponding type variable of the tuple. The body `Ty`
//! is inferred with these binders in scope (see the type inference builder) and must be a
//! callable type; the result is that callable with the binders as its own type parameters
//! ([`evaluate_generic_callable`]).
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_python_ast::name::Name;

use super::generics::GenericContext;
use super::type_operator::{TypeOperatorError, is_unresolved_alias};
use super::typevar::{TypeVarIdentity, TypeVarInstance};
use super::visitor::any_over_type_including_alias_arguments;
use super::{
    BindingContext, BoundTypeVarInstance, Type, TypeVarKind, TypeVarNonce, TypeVarVariance,
};
use crate::{Db, ProgramEnvironment};

/// Creates the type variable bound by the lambda parameter `name` of a `GenericCallable`. It has
/// the bound or constraints and the default of `source` (the corresponding type variable of the
/// `Vs` tuple), if any.
///
/// Like the binders of type comprehensions, binders with the same name and the same bound share
/// an identity; call binding freshens the type parameters of a callable anyway.
pub(crate) fn generic_callable_binder<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    name: Name,
    source: Option<TypeVarInstance<'db>>,
) -> BoundTypeVarInstance<'db> {
    let identity = TypeVarIdentity::new(db, name, None, TypeVarKind::Pep695TypeVar);
    let bound_or_constraints =
        source.and_then(|source| source.bound_or_constraints(db, env).map(Into::into));
    let default = source.and_then(|source| source.default_type(db, env).map(Into::into));
    let typevar = TypeVarInstance::new(
        db,
        identity,
        bound_or_constraints,
        Some(TypeVarVariance::Invariant),
        default,
    );
    BoundTypeVarInstance::new(
        db,
        typevar,
        BindingContext::Synthetic(env.program(db)),
        None,
        TypeVarNonce::NONE,
    )
}

/// Evaluates `GenericCallable[..., lambda <binders>: body]` with an evaluated `body`: the body
/// callable with `binders` as (additional) type parameters of each of its signatures.
///
/// Returns `Ok(None)` if `body` is an unresolved operator that depends on type variables other
/// than the binders (so that it may still evaluate to a callable once they are specialized). An
/// unresolved body that only depends on the binders is not supported: the binders are only
/// solved at a call, where the body could no longer be evaluated.
pub(crate) fn evaluate_generic_callable<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    binders: &[BoundTypeVarInstance<'db>],
    body: Type<'db>,
) -> Result<Option<Type<'db>>, TypeOperatorError> {
    let unresolved = is_unresolved_alias(db, env, body)
        || matches!(body, Type::TypeOperator(operator) if !operator.kind(db).is_inert());
    if unresolved {
        return if depends_on_outer_type_variables(db, env, body, binders) {
            Ok(None)
        } else {
            Err(TypeOperatorError::new(format!(
                "A `GenericCallable` body that depends on its own type parameters must be a \
                 callable type, but got `{}` (not supported yet)",
                body.display(db, env)
            )))
        };
    }
    match body.resolve_type_alias(db) {
        dynamic @ Type::Dynamic(_) => Ok(Some(dynamic)),
        Type::Callable(callable) => {
            let context = GenericContext::from_typevar_instances(db, env, binders.iter().copied());
            let signatures = callable
                .signatures(db)
                .with_inherited_generic_context(db, context);
            Ok(Some(Type::Callable(
                callable.with_signatures(db, signatures),
            )))
        }
        resolved => Err(TypeOperatorError::new(format!(
            "The body of `GenericCallable` must be a callable type, but got `{}`",
            resolved.display(db, env)
        ))),
    }
}

/// Returns `true` if `ty` contains a type variable other than `binders` (or a cycle marker).
fn depends_on_outer_type_variables<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    binders: &[BoundTypeVarInstance<'db>],
) -> bool {
    any_over_type_including_alias_arguments(db, env, ty, |ty| match ty {
        Type::TypeVar(typevar) => !binders
            .iter()
            .any(|binder| binder.is_same_typevar_as(db, typevar)),
        Type::Divergent(_) | Type::Recursive(_) => true,
        _ => false,
    })
}
