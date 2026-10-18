//! Experimental support for [PEP 827] type operators: type booleans (`IsAssignable`,
//! `IsEquivalent`, `Bool`, and `not`/`and`/`or` over them) and conditional types
//! (`X if C else Y`).
//!
//! Operators are evaluated eagerly whenever their operands are fully known. Only operators whose
//! operands still contain unsolved type variables (or other unresolved operators) are stored as
//! [`Type::TypeOperator`]; they are re-evaluated whenever a type mapping (e.g. a specialization)
//! is applied to them.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use super::variance::{VarianceInferable, VarianceTerm};
use super::visitor::any_over_type_including_alias_arguments;
use super::{
    ApplyTypeMappingVisitor, BoundTypeVarIdentity, IntersectionType, KnownClass, LiteralValueType,
    PromotionMode, Type, TypeContext, TypeMapping, TypeVarVariance, UnionType, visitor,
};
use crate::Db;
use crate::ProgramEnvironment;

#[salsa::interned(debug, heap_size=ruff_memory_usage::heap_size)]
pub struct TypeOperatorType<'db> {
    #[returns(ref)]
    pub(crate) kind: TypeOperatorKind<'db>,
}

// The Salsa heap is tracked separately.
impl get_size2::GetSize for TypeOperatorType<'_> {}

/// The payload of an unresolved PEP 827 type operator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub enum TypeOperatorKind<'db> {
    /// `IsAssignable[T, S]`
    IsAssignable(Type<'db>, Type<'db>),
    /// `IsEquivalent[T, S]`
    IsEquivalent(Type<'db>, Type<'db>),
    /// `Bool[T]`
    Bool(Type<'db>),
    /// `not B`
    Not(Type<'db>),
    /// `B1 and B2 and ...`
    And(Box<[Type<'db>]>),
    /// `B1 or B2 or ...`
    Or(Box<[Type<'db>]>),
    /// `X if C else Y`
    Conditional {
        condition: Type<'db>,
        if_true: Type<'db>,
        if_false: Type<'db>,
    },
}

impl<'db> TypeOperatorKind<'db> {
    /// Calls `f` on every operand that is evaluated as part of a type-boolean computation:
    /// the arguments of `IsAssignable`/`IsEquivalent`/`Bool`, the operands of `not`/`and`/`or`,
    /// and the condition of a conditional type.
    fn for_each_condition_operand(&self, mut f: impl FnMut(Type<'db>)) {
        match self {
            Self::IsAssignable(a, b) | Self::IsEquivalent(a, b) => {
                f(*a);
                f(*b);
            }
            Self::Bool(a) | Self::Not(a) => f(*a),
            Self::And(operands) | Self::Or(operands) => operands.iter().copied().for_each(f),
            Self::Conditional { condition, .. } => f(*condition),
        }
    }

    /// Calls `f` on every operand of this operator.
    fn for_each_operand(&self, mut f: impl FnMut(Type<'db>)) {
        self.for_each_condition_operand(&mut f);
        if let Self::Conditional {
            if_true, if_false, ..
        } = self
        {
            f(*if_true);
            f(*if_false);
        }
    }

    /// Returns a new kind with `condition` applied to every condition operand (see
    /// [`Self::for_each_condition_operand`]) and `branch` applied to the branches of a
    /// conditional type. Returns `None` if any of the mapping functions return `None`.
    fn try_map(
        &self,
        mut condition: impl FnMut(Type<'db>) -> Option<Type<'db>>,
        mut branch: impl FnMut(Type<'db>) -> Option<Type<'db>>,
    ) -> Option<Self> {
        Some(match self {
            Self::IsAssignable(a, b) => Self::IsAssignable(condition(*a)?, condition(*b)?),
            Self::IsEquivalent(a, b) => Self::IsEquivalent(condition(*a)?, condition(*b)?),
            Self::Bool(a) => Self::Bool(condition(*a)?),
            Self::Not(a) => Self::Not(condition(*a)?),
            Self::And(operands) => Self::And(
                operands
                    .iter()
                    .map(|ty| condition(*ty))
                    .collect::<Option<_>>()?,
            ),
            Self::Or(operands) => Self::Or(
                operands
                    .iter()
                    .map(|ty| condition(*ty))
                    .collect::<Option<_>>()?,
            ),
            Self::Conditional {
                condition: cond,
                if_true,
                if_false,
            } => Self::Conditional {
                condition: condition(*cond)?,
                if_true: branch(*if_true)?,
                if_false: branch(*if_false)?,
            },
        })
    }

    fn map(
        &self,
        mut condition: impl FnMut(Type<'db>) -> Type<'db>,
        mut branch: impl FnMut(Type<'db>) -> Type<'db>,
    ) -> Self {
        self.try_map(|ty| Some(condition(ty)), |ty| Some(branch(ty)))
            .expect("infallible mapping")
    }
}

/// The value of a type boolean operand, as far as it can be determined.
#[derive(Debug, Clone, Copy)]
enum TypeBoolValue<'db> {
    /// `Literal[True]` or `Literal[False]`.
    Known(bool),
    /// `bool`: a type boolean that is not statically known.
    Indeterminate,
    /// A dynamic type (`Any`, `Unknown`, ...).
    Dynamic(Type<'db>),
    /// An unresolved type boolean that depends on unsolved type variables.
    Deferred,
    /// Not a type boolean at all.
    NotABool,
}

impl<'db> TypeBoolValue<'db> {
    fn of(db: &'db dyn Db, env: &ProgramEnvironment<'db>, ty: Type<'db>) -> Self {
        let resolved = ty.resolve_type_alias(db);
        match resolved {
            Type::LiteralValue(literal) => match literal.as_bool() {
                Some(value) => Self::Known(value),
                None => Self::NotABool,
            },
            Type::NominalInstance(instance) if instance.has_known_class(db, KnownClass::Bool) => {
                Self::Indeterminate
            }
            Type::Dynamic(_) => Self::Dynamic(resolved),
            // An unresolved operator. (An unresolved conditional type may still evaluate to a
            // type boolean once its condition is known.)
            Type::TypeOperator(_) => Self::Deferred,
            _ if needs_deferral(db, env, resolved) => Self::Deferred,
            _ => Self::NotABool,
        }
    }
}

/// Returns `true` if `ty` is acceptable as the condition of a conditional type or as an operand
/// of `not`/`and`/`or` in a type expression: `Literal[True]`, `Literal[False]`, `bool`, a
/// dynamic type, or an unresolved type boolean.
pub(crate) fn is_type_bool_operand<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> bool {
    let resolved = ty.resolve_type_alias(db);
    match resolved {
        Type::TypeOperator(operator) => match operator.kind(db) {
            TypeOperatorKind::Conditional {
                if_true, if_false, ..
            } => {
                is_type_bool_operand(db, env, *if_true) && is_type_bool_operand(db, env, *if_false)
            }
            _ => true,
        },
        // Cycle markers can stand in for any type during fixed-point iteration.
        Type::Divergent(_) | Type::Recursive(_) => true,
        _ => !matches!(
            TypeBoolValue::of(db, env, resolved),
            TypeBoolValue::NotABool | TypeBoolValue::Deferred
        ),
    }
}

/// Returns `true` if evaluating an operator over `ty` must be deferred: `ty` contains an unsolved
/// type variable, a cycle marker, or another unresolved type operator.
fn needs_deferral<'db>(db: &'db dyn Db, env: &ProgramEnvironment<'db>, ty: Type<'db>) -> bool {
    any_over_type_including_alias_arguments(db, env, ty, |ty| {
        matches!(
            ty,
            Type::TypeVar(_) | Type::Divergent(_) | Type::Recursive(_) | Type::TypeOperator(_)
        )
    })
}

fn type_bool<'db>(value: bool) -> Type<'db> {
    Type::LiteralValue(LiteralValueType::unpromotable(value))
}

impl<'db> TypeOperatorType<'db> {
    /// Evaluate the operator described by `kind`, or return an unresolved
    /// [`Type::TypeOperator`] if its operands are not yet known.
    pub(crate) fn evaluate_or_defer(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: TypeOperatorKind<'db>,
    ) -> Type<'db> {
        let defer = |kind| Type::TypeOperator(TypeOperatorType::new(db, kind));

        match &kind {
            TypeOperatorKind::IsAssignable(source, target) => {
                // An `Unknown` operand usually stems from an invalid type expression; propagate
                // it rather than produce an arbitrary type boolean.
                if source.is_unknown() || target.is_unknown() {
                    return Type::unknown();
                }
                if needs_deferral(db, env, *source) || needs_deferral(db, env, *target) {
                    return defer(kind);
                }
                type_bool(source.is_assignable_to(db, env, *target))
            }
            TypeOperatorKind::IsEquivalent(left, right) => {
                if left.is_unknown() || right.is_unknown() {
                    return Type::unknown();
                }
                if needs_deferral(db, env, *left) || needs_deferral(db, env, *right) {
                    return defer(kind);
                }
                type_bool(
                    left.is_assignable_to(db, env, *right)
                        && right.is_assignable_to(db, env, *left),
                )
            }
            TypeOperatorKind::Bool(operand) => {
                if operand.is_unknown() {
                    return Type::unknown();
                }
                if needs_deferral(db, env, *operand) {
                    return defer(kind);
                }
                type_bool(
                    operand.is_assignable_to(db, env, type_bool(true))
                        && !operand.is_assignable_to(db, env, Type::Never),
                )
            }
            TypeOperatorKind::Not(operand) => match TypeBoolValue::of(db, env, *operand) {
                TypeBoolValue::Known(value) => type_bool(!value),
                TypeBoolValue::Indeterminate => KnownClass::Bool.to_instance(db, env),
                TypeBoolValue::Dynamic(dynamic) => dynamic,
                TypeBoolValue::Deferred => defer(kind),
                TypeBoolValue::NotABool => Type::unknown(),
            },
            TypeOperatorKind::And(operands) => {
                Self::evaluate_connective(db, env, &kind, operands, false)
            }
            TypeOperatorKind::Or(operands) => {
                Self::evaluate_connective(db, env, &kind, operands, true)
            }
            TypeOperatorKind::Conditional {
                condition,
                if_true,
                if_false,
            } => match TypeBoolValue::of(db, env, *condition) {
                TypeBoolValue::Known(true) => *if_true,
                TypeBoolValue::Known(false)
                | TypeBoolValue::Indeterminate
                | TypeBoolValue::NotABool => *if_false,
                TypeBoolValue::Dynamic(_) => {
                    UnionType::from_two_elements(db, env, *if_true, *if_false)
                }
                TypeBoolValue::Deferred => defer(kind),
            },
        }
    }

    /// Evaluate `and` (`short_circuit == false`) or `or` (`short_circuit == true`).
    ///
    /// Any operand equal to `short_circuit` decides the result, even if other operands are still
    /// unresolved.
    fn evaluate_connective(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        operands: &[Type<'db>],
        short_circuit: bool,
    ) -> Type<'db> {
        let mut deferred = false;
        let mut dynamic = None;
        let mut indeterminate = false;
        let mut invalid = false;

        for operand in operands {
            match TypeBoolValue::of(db, env, *operand) {
                TypeBoolValue::Known(value) if value == short_circuit => {
                    return type_bool(short_circuit);
                }
                TypeBoolValue::Known(_) => {}
                TypeBoolValue::Indeterminate => indeterminate = true,
                TypeBoolValue::Dynamic(ty) => dynamic = dynamic.or(Some(ty)),
                TypeBoolValue::Deferred => deferred = true,
                TypeBoolValue::NotABool => invalid = true,
            }
        }

        if invalid {
            Type::unknown()
        } else if deferred {
            Type::TypeOperator(TypeOperatorType::new(db, kind.clone()))
        } else if let Some(dynamic) = dynamic {
            dynamic
        } else if indeterminate {
            KnownClass::Bool.to_instance(db, env)
        } else {
            type_bool(!short_circuit)
        }
    }

    /// The narrowest type that every possible evaluation of this operator is assignable to.
    ///
    /// For a conditional type `X if C else Y`, this is `X | Y`; for a type boolean, `bool`.
    pub(crate) fn upper_bound(self, db: &'db dyn Db, env: &ProgramEnvironment<'db>) -> Type<'db> {
        match self.kind(db) {
            TypeOperatorKind::Conditional {
                if_true, if_false, ..
            } => UnionType::from_two_elements(db, env, *if_true, *if_false),
            _ => KnownClass::Bool.to_instance(db, env),
        }
    }

    /// The widest type that is assignable to every possible evaluation of this operator.
    ///
    /// For a conditional type `X if C else Y`, this is `X & Y`; for a type boolean, `Never`
    /// (since it is either `Literal[True]` or `Literal[False]`).
    pub(crate) fn lower_bound(self, db: &'db dyn Db, env: &ProgramEnvironment<'db>) -> Type<'db> {
        match self.kind(db) {
            TypeOperatorKind::Conditional {
                if_true, if_false, ..
            } => IntersectionType::from_two_elements(db, env, *if_true, *if_false),
            _ => Type::Never,
        }
    }

    /// Apply `type_mapping` to the operands of this operator, then re-evaluate it.
    ///
    /// Promotion and materialization only apply to the branches of a conditional type: the
    /// operands of type-boolean computations are compared as written, so that (for example)
    /// promoting `Bool[Literal[True]]` does not change its value.
    pub(super) fn apply_type_mapping_impl<'a>(
        self,
        db: &'db dyn Db,
        type_mapping: &TypeMapping<'a, 'db>,
        tcx: TypeContext<'db>,
        visitor: &ApplyTypeMappingVisitor<'_, 'db>,
    ) -> Type<'db> {
        visitor.visit(db, Type::TypeOperator(self), type_mapping, || {
            let maps_conditions = !matches!(
                type_mapping,
                TypeMapping::Materialize(_) | TypeMapping::Promote(PromotionMode::On, _)
            );
            let kind = self.kind(db).map(
                |ty| {
                    if maps_conditions {
                        ty.apply_type_mapping_impl(
                            db,
                            type_mapping,
                            TypeContext::default(),
                            visitor,
                        )
                    } else {
                        ty
                    }
                },
                |ty| ty.apply_type_mapping_impl(db, type_mapping, tcx, visitor),
            );
            Self::evaluate_or_defer(db, visitor.env, kind)
        })
    }

    /// Normalize the operands of this operator without re-evaluating it.
    pub(super) fn recursive_type_normalized_impl(
        self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        div: Type<'db>,
        nested: bool,
    ) -> Option<Self> {
        let normalize = |ty: Type<'db>| {
            let normalized = ty.recursive_type_normalized_impl(db, env, div, true);
            if nested {
                normalized
            } else {
                Some(normalized.unwrap_or(div))
            }
        };
        let kind = self.kind(db).try_map(normalize, normalize)?;
        Some(TypeOperatorType::new(db, kind))
    }

    /// Calls `f` on every operand of this operator.
    pub(super) fn for_each_operand(self, db: &'db dyn Db, f: impl FnMut(Type<'db>)) {
        self.kind(db).for_each_operand(f);
    }
}

pub(super) fn walk_type_operator_type<'db, V: visitor::TypeVisitor<'db> + ?Sized>(
    db: &'db dyn Db,
    operator: TypeOperatorType<'db>,
    visitor: &V,
) {
    operator.for_each_operand(db, |ty| visitor.visit_type(db, ty));
}

impl<'db> VarianceInferable<'db> for TypeOperatorType<'db> {
    // A type variable that occurs in a type-boolean computation can change the result
    // arbitrarily, so such occurrences are invariant. The branches of a conditional type are
    // covariant.
    fn variance_of(
        self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        typevar: BoundTypeVarIdentity<'db>,
    ) -> VarianceTerm<'db> {
        let kind = self.kind(db);
        let mut terms = Vec::new();
        kind.for_each_condition_operand(|ty| {
            terms.push(
                ty.with_polarity(TypeVarVariance::Invariant)
                    .variance_of(db, env, typevar),
            );
        });
        if let TypeOperatorKind::Conditional {
            if_true, if_false, ..
        } = kind
        {
            terms.push(if_true.variance_of(db, env, typevar));
            terms.push(if_false.variance_of(db, env, typevar));
        }
        VarianceTerm::join(db, terms)
    }
}
