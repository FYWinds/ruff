//! Experimental support for [PEP 827] type operators.
//!
//! Phase 1: type booleans (`IsAssignable`, `IsEquivalent`, `Bool`, and `not`/`and`/`or` over
//! them) and conditional types (`X if C else Y`).
//!
//! Phase 2: member introspection (`Attrs`, `Members`, `GetMember`, `GetMemberType`, `m.name`
//! etc.), `FromUnion`, unpacked comprehensions (`*[body for p in Iter[X] if c]`), variadic
//! splicing into `tuple[...]`/`Union[...]`, and structural synthesis (`NewProtocol`,
//! `NewTypedDict`).
//!
//! Operators are evaluated eagerly whenever their operands are fully known. Only operators whose
//! operands still contain unsolved type variables (or other unresolved operators) are stored as
//! [`Type::TypeOperator`]; they are re-evaluated whenever a type mapping (e.g. a specialization)
//! is applied to them.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use std::cell::{Cell, RefCell};

use ruff_python_ast::name::Name;

use super::callable::walk_callable_type;
use super::function::{FunctionType, walk_function_type};
use super::generics::walk_specialization_types;

use super::member_introspection::{
    all_qualifiers_type, find_member, member_args, member_instance, members_of, parse_qualifiers,
};
use super::signatures::CallableSignature;
use super::tuple::{Tuple, TupleSpec, TupleSpecBuilder, TupleType, VariableSegment};
use super::typed_dict::{TypedDictFieldBuilder, TypedDictSchema};
use super::variance::{VarianceInferable, VarianceTerm};
use super::visitor::{TypeCollector, TypeVisitor};
use super::{
    ApplyTypeMappingVisitor, BoundTypeVarIdentity, BoundTypeVarInstance, CallableType,
    IntersectionType, KnownClass, LiteralValueType, PromotionMode, Type, TypeContext, TypeMapping,
    TypeVarVariance, TypedDictType, UnionBuilder, UnionType, visitor,
};
use crate::Db;
use crate::ProgramEnvironment;
use crate::TypeQualifiers;

/// The maximum number of argument combinations that an operator lifted over unions may be
/// evaluated for.
const MAX_LIFTED_COMBINATIONS: usize = 256;

#[salsa::interned(debug, heap_size=ruff_memory_usage::heap_size)]
pub struct TypeOperatorType<'db> {
    #[returns(ref)]
    pub(crate) kind: TypeOperatorKind<'db>,
}

// The Salsa heap is tracked separately.
impl get_size2::GetSize for TypeOperatorType<'_> {}

/// A field of a `Member` that can be accessed with dot syntax in a type expression (`m.name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub enum MemberField {
    Name,
    Type,
    Quals,
    Init,
    Definer,
}

impl MemberField {
    /// The index of the corresponding type parameter of `Member`.
    const fn index(self) -> usize {
        match self {
            Self::Name => 0,
            Self::Type => 1,
            Self::Quals => 2,
            Self::Init => 3,
            Self::Definer => 4,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Type => "type",
            Self::Quals => "quals",
            Self::Init => "init",
            Self::Definer => "definer",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "name" => Self::Name,
            "type" => Self::Type,
            "quals" => Self::Quals,
            "init" => Self::Init,
            "definer" => Self::Definer,
            _ => return None,
        })
    }
}

/// An argument in a variadic position (`tuple[...]`, `Union[...]`, `NewProtocol[...]`,
/// `NewTypedDict[...]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub enum VariadicArg<'db> {
    /// A single element.
    Single(Type<'db>),
    /// An unpacked sequence (`*X`): a tuple, an unpacked comprehension, or another operator that
    /// produces a sequence.
    Unpacked(Type<'db>),
}

impl<'db> VariadicArg<'db> {
    pub(crate) const fn ty(self) -> Type<'db> {
        match self {
            Self::Single(ty) | Self::Unpacked(ty) => ty,
        }
    }

    fn with_ty(self, ty: Type<'db>) -> Self {
        match self {
            Self::Single(_) => Self::Single(ty),
            Self::Unpacked(_) => Self::Unpacked(ty),
        }
    }
}

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
    /// `Attrs[T]`
    Attrs(Type<'db>),
    /// `Members[T]`
    Members(Type<'db>),
    /// `GetMember[T, S]`
    GetMember(Type<'db>, Type<'db>),
    /// `GetMemberType[T, S]`
    GetMemberType(Type<'db>, Type<'db>),
    /// `FromUnion[T]`
    FromUnion(Type<'db>),
    /// `m.name`, `m.type`, ...
    MemberAccess(Type<'db>, MemberField),
    /// `*[body for binder in Iter[iterable] if c1 if c2 ...]`
    UnpackMap {
        iterable: Type<'db>,
        binder: BoundTypeVarInstance<'db>,
        body: Type<'db>,
        conditions: Box<[Type<'db>]>,
    },
    /// `tuple[...]` with an unresolved unpacked argument.
    TupleOf(Box<[VariadicArg<'db>]>),
    /// `Union[...]` with an unresolved unpacked argument.
    UnionOf(Box<[VariadicArg<'db>]>),
    /// `NewProtocol[...]`
    NewProtocol(Box<[VariadicArg<'db>]>),
    /// `NewTypedDict[...]`
    NewTypedDict(Box<[VariadicArg<'db>]>),
}

/// How an operand contributes to the result of an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandRole {
    /// The operand is an input of a computation (e.g. a type-boolean operand or the argument of
    /// `Attrs`). Such operands are compared as written: they are not promoted or materialized,
    /// and type variables in them are invariant.
    Input,
    /// The operand becomes (part of) the result (e.g. the branches of a conditional type, or the
    /// elements of `tuple[...]`). Such operands are covariant.
    Output,
}

impl<'db> TypeOperatorKind<'db> {
    /// Calls `f` on every operand of this operator, together with its role.
    fn for_each_operand_with_role(&self, mut f: impl FnMut(Type<'db>, OperandRole)) {
        use OperandRole::{Input, Output};
        match self {
            Self::IsAssignable(a, b)
            | Self::IsEquivalent(a, b)
            | Self::GetMember(a, b)
            | Self::GetMemberType(a, b) => {
                f(*a, Input);
                f(*b, Input);
            }
            Self::Bool(a)
            | Self::Not(a)
            | Self::Attrs(a)
            | Self::Members(a)
            | Self::FromUnion(a)
            | Self::MemberAccess(a, _) => f(*a, Input),
            Self::And(operands) | Self::Or(operands) => {
                operands.iter().for_each(|ty| f(*ty, Input));
            }
            Self::Conditional {
                condition,
                if_true,
                if_false,
            } => {
                f(*condition, Input);
                f(*if_true, Output);
                f(*if_false, Output);
            }
            Self::UnpackMap {
                iterable,
                body,
                conditions,
                ..
            } => {
                f(*iterable, Input);
                conditions.iter().for_each(|ty| f(*ty, Input));
                f(*body, Output);
            }
            Self::TupleOf(args) | Self::UnionOf(args) => {
                args.iter().for_each(|arg| f(arg.ty(), Output));
            }
            Self::NewProtocol(args) | Self::NewTypedDict(args) => {
                args.iter().for_each(|arg| f(arg.ty(), Input));
            }
        }
    }

    /// Returns a new kind with `f` applied to every operand (see
    /// [`Self::for_each_operand_with_role`]). Returns `None` if `f` returns `None` for any
    /// operand. The binder of an unpacked comprehension is kept unchanged.
    fn try_map(
        &self,
        mut f: impl FnMut(Type<'db>, OperandRole) -> Option<Type<'db>>,
    ) -> Option<Self> {
        use OperandRole::{Input, Output};
        let mut map_all = |types: &[Type<'db>], role| {
            types
                .iter()
                .map(|ty| f(*ty, role))
                .collect::<Option<Box<[_]>>>()
        };
        Some(match self {
            Self::IsAssignable(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::IsAssignable(a?, b?)
            }
            Self::IsEquivalent(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::IsEquivalent(a?, b?)
            }
            Self::GetMember(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::GetMember(a?, b?)
            }
            Self::GetMemberType(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::GetMemberType(a?, b?)
            }
            Self::Bool(a) => Self::Bool(f(*a, Input)?),
            Self::Not(a) => Self::Not(f(*a, Input)?),
            Self::Attrs(a) => Self::Attrs(f(*a, Input)?),
            Self::Members(a) => Self::Members(f(*a, Input)?),
            Self::FromUnion(a) => Self::FromUnion(f(*a, Input)?),
            Self::MemberAccess(a, field) => Self::MemberAccess(f(*a, Input)?, *field),
            Self::And(operands) => Self::And(map_all(operands, Input)?),
            Self::Or(operands) => Self::Or(map_all(operands, Input)?),
            Self::Conditional {
                condition,
                if_true,
                if_false,
            } => Self::Conditional {
                condition: f(*condition, Input)?,
                if_true: f(*if_true, Output)?,
                if_false: f(*if_false, Output)?,
            },
            Self::UnpackMap {
                iterable,
                binder,
                body,
                conditions,
            } => Self::UnpackMap {
                iterable: f(*iterable, Input)?,
                conditions: conditions
                    .iter()
                    .map(|ty| f(*ty, Input))
                    .collect::<Option<_>>()?,
                body: f(*body, Output)?,
                binder: *binder,
            },
            Self::TupleOf(args) => Self::TupleOf(Self::map_args(args, |ty| f(ty, Output))?),
            Self::UnionOf(args) => Self::UnionOf(Self::map_args(args, |ty| f(ty, Output))?),
            Self::NewProtocol(args) => Self::NewProtocol(Self::map_args(args, |ty| f(ty, Input))?),
            Self::NewTypedDict(args) => {
                Self::NewTypedDict(Self::map_args(args, |ty| f(ty, Input))?)
            }
        })
    }

    fn map_args(
        args: &[VariadicArg<'db>],
        mut f: impl FnMut(Type<'db>) -> Option<Type<'db>>,
    ) -> Option<Box<[VariadicArg<'db>]>> {
        args.iter()
            .map(|arg| Some(arg.with_ty(f(arg.ty())?)))
            .collect()
    }
}

/// An error from eagerly evaluating a type operator. Reported by the inference layer as
/// `invalid-type-form`; errors from deferred evaluation are silent (the result is `Unknown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeOperatorError {
    message: String,
}

impl TypeOperatorError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for TypeOperatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

type OperatorResult<'db> = Result<Type<'db>, TypeOperatorError>;

/// The elements produced by a type that is unpacked in a variadic position or iterated over in a
/// type-level comprehension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Sequence<'db> {
    /// A sequence of known length.
    Fixed(Vec<Type<'db>>),
    /// A sequence of unknown length and contents (e.g. `Any`).
    Dynamic,
    /// The sequence depends on unsolved type variables.
    Deferred,
    /// The type does not describe a supported sequence.
    Error,
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

/// Returns `true` if evaluating an operator over `ty` must be deferred: `ty` contains a free
/// (unsolved) type variable, a cycle marker, or another unresolved type operator.
///
/// Type variables bound by the generic context of a callable signature within `ty` (e.g. the
/// type parameter `U` of a method `def f[U](self, x: U) -> U` that appears as a member type) are
/// not free.
fn needs_deferral<'db>(db: &'db dyn Db, env: &ProgramEnvironment<'db>, ty: Type<'db>) -> bool {
    let visitor = FreeTypeVarVisitor {
        env,
        bound: RefCell::default(),
        recursion_guard: TypeCollector::default(),
        found: Cell::new(false),
    };
    visitor.visit_type(db, ty);
    visitor.found.get()
}

struct FreeTypeVarVisitor<'db, 'a> {
    env: &'a ProgramEnvironment<'db>,
    /// Type variables bound by the callable signatures that are currently being visited.
    bound: RefCell<Vec<BoundTypeVarIdentity<'db>>>,
    recursion_guard: TypeCollector<'db>,
    found: Cell<bool>,
}

impl<'db> FreeTypeVarVisitor<'db, '_> {
    /// Visits a callable's signatures with their own type parameters marked as bound.
    fn with_signature_binders(
        &self,
        db: &'db dyn Db,
        signatures: &CallableSignature<'db>,
        walk: impl FnOnce(),
    ) {
        let binders: Vec<_> = signatures
            .iter()
            .filter_map(|signature| signature.generic_context)
            .flat_map(|context| context.variables(db))
            .map(|typevar| typevar.identity(db))
            .collect();
        let depth = self.bound.borrow().len();
        self.bound.borrow_mut().extend(binders);
        walk();
        self.bound.borrow_mut().truncate(depth);
    }
}

impl<'db> visitor::TypeVisitor<'db> for FreeTypeVarVisitor<'db, '_> {
    fn program_environment(&self) -> &ProgramEnvironment<'db> {
        self.env
    }

    fn should_visit_lazy_type_attributes(&self) -> bool {
        false
    }

    fn visit_type(&self, db: &'db dyn Db, ty: Type<'db>) {
        if self.found.get() {
            return;
        }
        match ty {
            Type::TypeVar(typevar) => {
                let typevar = if typevar.paramspec_attr(db).is_some() {
                    typevar.without_paramspec_attr(db)
                } else {
                    typevar
                };
                let identity = typevar.identity(db);
                if !self.bound.borrow().contains(&identity) {
                    self.found.set(true);
                }
            }
            Type::Divergent(_) | Type::Recursive(_) | Type::TypeOperator(_) => {
                self.found.set(true);
            }
            // Visit the arguments of a type alias without evaluating its value.
            Type::TypeAlias(alias) => {
                if let Some(specialization) = alias.specialization(db) {
                    walk_specialization_types(db, specialization, self);
                }
            }
            _ => visitor::walk_type_with_recursion_guard(db, ty, self, &self.recursion_guard),
        }
    }

    fn visit_callable_type(&self, db: &'db dyn Db, callable: CallableType<'db>) {
        self.with_signature_binders(db, callable.signatures(db), || {
            walk_callable_type(db, callable, self);
        });
    }

    fn visit_function_type(&self, db: &'db dyn Db, function: FunctionType<'db>) {
        self.with_signature_binders(db, function.signature(db), || {
            walk_function_type(db, function, self);
        });
    }
}

fn type_bool<'db>(value: bool) -> Type<'db> {
    Type::LiteralValue(LiteralValueType::unpromotable(value))
}

/// Creates the synthetic type variable bound by a type-level comprehension
/// (`*[... for name in Iter[...]]`).
///
/// Binders with the same name share an identity; nested comprehensions are kept apart by
/// shadowing (a type mapping never substitutes a comprehension's own binder in its body or
/// conditions).
pub(crate) fn comprehension_binder<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    name: Name,
) -> BoundTypeVarInstance<'db> {
    BoundTypeVarInstance::synthetic(db, env, name, TypeVarVariance::Invariant)
}

/// Accesses a field of a `Member` in a type expression (`m.name`).
///
/// Returns the corresponding specialization argument of a `Member`, an unresolved
/// [`TypeOperatorKind::MemberAccess`] if `ty` contains type variables (such as a comprehension
/// binder), `ty` itself for `Any`/`Unknown`, and `Never` for `Never`. Returns `Err` if `ty` is not
/// a `Member` (including unions of `Member`s).
pub(crate) fn member_access<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    field: MemberField,
) -> Result<Type<'db>, ()> {
    let resolved = ty.resolve_type_alias(db);
    match resolved {
        Type::Dynamic(_) | Type::Never => return Ok(resolved),
        _ => {}
    }
    if let Some(args) = member_args(db, env, resolved) {
        return Ok(args[field.index()]);
    }
    if needs_deferral(db, env, resolved) {
        return Ok(Type::TypeOperator(TypeOperatorType::new(
            db,
            TypeOperatorKind::MemberAccess(ty, field),
        )));
    }
    Err(())
}

/// Determines the elements of `ty` when it is unpacked in a variadic position or iterated over in
/// a type-level comprehension.
///
/// Fixed-length tuples produce their elements, `Never` produces no elements, and dynamic types
/// (and `tuple[Any, ...]`) produce a dynamic sequence. Unions, unbounded tuples and non-tuples are
/// errors.
pub(crate) fn evaluate_sequence<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Sequence<'db> {
    try_evaluate_sequence(db, env, ty).unwrap_or(Sequence::Error)
}

fn try_evaluate_sequence<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Result<Sequence<'db>, TypeOperatorError> {
    let resolved = ty.resolve_type_alias(db);
    match resolved {
        Type::Never => return Ok(Sequence::Fixed(Vec::new())),
        Type::Dynamic(_) => return Ok(Sequence::Dynamic),
        _ => {}
    }
    if let Some(spec) = resolved.exact_tuple_instance_spec(db) {
        return match spec.as_ref() {
            Tuple::Fixed(fixed) => Ok(Sequence::Fixed(fixed.all_elements().to_vec())),
            Tuple::Variable(variable) => {
                if variable.prefix_elements().is_empty()
                    && variable.suffix_elements().is_empty()
                    && matches!(
                        variable.variable(),
                        VariableSegment::Homogeneous(element) if element.is_dynamic()
                    )
                {
                    Ok(Sequence::Dynamic)
                } else {
                    // Iterating over unbounded tuples, including `tuple[*Ts]`, is not supported
                    // yet, even if specialization could later make them bounded.
                    Err(TypeOperatorError::new(format!(
                        "Iterating over the unbounded tuple `{}` is not yet supported",
                        resolved.display(db, env)
                    )))
                }
            }
        };
    }
    if needs_deferral(db, env, resolved) {
        return Ok(Sequence::Deferred);
    }
    if let Type::Union(_) = resolved {
        return Err(TypeOperatorError::new(format!(
            "Cannot iterate over the union `{}`; expected a tuple type",
            resolved.display(db, env)
        )));
    }
    Err(TypeOperatorError::new(format!(
        "Cannot iterate over `{}`; expected a tuple type",
        resolved.display(db, env)
    )))
}

/// Evaluates `f` for every combination of the union elements of `args` and unions the results
/// (PEP 827 "lifting over unions"). `Never` has no elements, so any `Never` argument makes the
/// result `Never`.
fn lift_over_unions<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    args: &[Type<'db>],
    mut f: impl FnMut(&[Type<'db>]) -> OperatorResult<'db>,
) -> OperatorResult<'db> {
    let elements: Vec<Vec<Type<'db>>> = args
        .iter()
        .map(|arg| match arg.resolve_type_alias(db) {
            Type::Union(union) => union.elements(db).to_vec(),
            Type::Never => Vec::new(),
            resolved => vec![resolved],
        })
        .collect();

    let combinations = elements
        .iter()
        .try_fold(1usize, |acc, elements| acc.checked_mul(elements.len()));
    match combinations {
        Some(0) => return Ok(Type::Never),
        Some(count) if count <= MAX_LIFTED_COMBINATIONS => {}
        _ => {
            return Err(TypeOperatorError::new(format!(
                "Too many union combinations to evaluate (the limit is \
                 {MAX_LIFTED_COMBINATIONS})"
            )));
        }
    }

    let mut builder = UnionBuilder::new(db, env);
    let mut indices = vec![0usize; elements.len()];
    let mut combination: Vec<Type<'db>> = elements.iter().map(|elements| elements[0]).collect();
    loop {
        builder = builder.add(f(&combination)?);

        // Advance to the next combination (odometer order: the last argument varies fastest).
        let mut position = elements.len();
        loop {
            if position == 0 {
                return Ok(builder.build());
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < elements[position].len() {
                combination[position] = elements[position][indices[position]];
                break;
            }
            indices[position] = 0;
            combination[position] = elements[position][0];
        }
    }
}

/// The members described by the arguments of `NewProtocol`/`NewTypedDict`.
enum SynthesisMembers<'db> {
    Members(Vec<SynthesisMember<'db>>),
    Dynamic,
    Deferred,
}

struct SynthesisMember<'db> {
    name: Name,
    ty: Type<'db>,
    qualifiers: TypeQualifiers,
}

impl<'db> TypeOperatorType<'db> {
    /// Evaluate the operator described by `kind`, or return an unresolved
    /// [`Type::TypeOperator`] if its operands are not yet known. Evaluation errors produce
    /// `Unknown` silently.
    pub(crate) fn evaluate_or_defer(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: TypeOperatorKind<'db>,
    ) -> Type<'db> {
        Self::try_evaluate_or_defer(db, env, kind).unwrap_or_else(|_| Type::unknown())
    }

    /// Like [`Self::evaluate_or_defer`], but returns an error describing why eager evaluation
    /// failed, so that it can be reported at the type expression.
    pub(crate) fn try_evaluate_or_defer(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: TypeOperatorKind<'db>,
    ) -> OperatorResult<'db> {
        let defer = |kind| Ok(Type::TypeOperator(TypeOperatorType::new(db, kind)));

        match &kind {
            TypeOperatorKind::IsAssignable(source, target) => {
                // An `Unknown` operand usually stems from an invalid type expression; propagate
                // it rather than produce an arbitrary type boolean.
                if source.is_unknown() || target.is_unknown() {
                    return Ok(Type::unknown());
                }
                if needs_deferral(db, env, *source) || needs_deferral(db, env, *target) {
                    return defer(kind);
                }
                Ok(type_bool(source.is_assignable_to(db, env, *target)))
            }
            TypeOperatorKind::IsEquivalent(left, right) => {
                if left.is_unknown() || right.is_unknown() {
                    return Ok(Type::unknown());
                }
                if needs_deferral(db, env, *left) || needs_deferral(db, env, *right) {
                    return defer(kind);
                }
                Ok(type_bool(
                    left.is_assignable_to(db, env, *right)
                        && right.is_assignable_to(db, env, *left),
                ))
            }
            TypeOperatorKind::Bool(operand) => {
                if operand.is_unknown() {
                    return Ok(Type::unknown());
                }
                if needs_deferral(db, env, *operand) {
                    return defer(kind);
                }
                Ok(type_bool(
                    operand.is_assignable_to(db, env, type_bool(true))
                        && !operand.is_assignable_to(db, env, Type::Never),
                ))
            }
            TypeOperatorKind::Not(operand) => Ok(match TypeBoolValue::of(db, env, *operand) {
                TypeBoolValue::Known(value) => type_bool(!value),
                TypeBoolValue::Indeterminate => KnownClass::Bool.to_instance(db, env),
                TypeBoolValue::Dynamic(dynamic) => dynamic,
                TypeBoolValue::Deferred => return defer(kind),
                TypeBoolValue::NotABool => Type::unknown(),
            }),
            TypeOperatorKind::And(operands) => {
                Ok(Self::evaluate_connective(db, env, &kind, operands, false))
            }
            TypeOperatorKind::Or(operands) => {
                Ok(Self::evaluate_connective(db, env, &kind, operands, true))
            }
            TypeOperatorKind::Conditional {
                condition,
                if_true,
                if_false,
            } => Ok(match TypeBoolValue::of(db, env, *condition) {
                TypeBoolValue::Known(true) => *if_true,
                TypeBoolValue::Known(false)
                | TypeBoolValue::Indeterminate
                | TypeBoolValue::NotABool => *if_false,
                TypeBoolValue::Dynamic(_) => {
                    UnionType::from_two_elements(db, env, *if_true, *if_false)
                }
                TypeBoolValue::Deferred => return defer(kind),
            }),
            TypeOperatorKind::Attrs(ty) | TypeOperatorKind::Members(ty) => {
                if needs_deferral(db, env, *ty) {
                    return defer(kind);
                }
                let attrs_only = matches!(kind, TypeOperatorKind::Attrs(_));
                lift_over_unions(db, env, &[*ty], |args| {
                    let ty = args[0];
                    if ty.is_dynamic() {
                        return Ok(ty);
                    }
                    members_of(db, env, ty, attrs_only).ok_or_else(|| {
                        TypeOperatorError::new(format!(
                            "`{}` requires a class type, but got `{}`",
                            if attrs_only { "Attrs" } else { "Members" },
                            ty.display(db, env)
                        ))
                    })
                })
            }
            TypeOperatorKind::GetMember(ty, name) | TypeOperatorKind::GetMemberType(ty, name) => {
                if needs_deferral(db, env, *ty) || needs_deferral(db, env, *name) {
                    return defer(kind);
                }
                let get_member = matches!(kind, TypeOperatorKind::GetMember(..));
                let operator_name = if get_member {
                    "GetMember"
                } else {
                    "GetMemberType"
                };
                lift_over_unions(db, env, &[*ty, *name], |args| {
                    let [ty, name] = [args[0], args[1]];
                    if ty.is_dynamic() {
                        return Ok(ty);
                    }
                    if name.is_dynamic() {
                        return Ok(name);
                    }
                    let Some(name_literal) = name.as_string_literal() else {
                        return Err(TypeOperatorError::new(format!(
                            "The second argument to `{operator_name}` must be a string literal \
                             type, but got `{}`",
                            name.display(db, env)
                        )));
                    };
                    let members = members_of(db, env, ty, false).ok_or_else(|| {
                        TypeOperatorError::new(format!(
                            "`{operator_name}` requires a class type, but got `{}`",
                            ty.display(db, env)
                        ))
                    })?;
                    let member_name = name_literal.value(db);
                    match find_member(db, env, members, member_name) {
                        Some(member) if get_member => Ok(member),
                        Some(member) => Ok(member_args(db, env, member)
                            .map_or_else(Type::unknown, |args| args[MemberField::Type.index()])),
                        None if get_member => Err(TypeOperatorError::new(format!(
                            "`{}` has no member named `{member_name}`",
                            ty.display(db, env)
                        ))),
                        None => Ok(Type::Never),
                    }
                })
            }
            TypeOperatorKind::FromUnion(ty) => {
                if needs_deferral(db, env, *ty) {
                    return defer(kind);
                }
                Ok(match ty.resolve_type_alias(db) {
                    Type::Union(union) => {
                        Type::heterogeneous_tuple(db, env, union.elements(db).iter().copied())
                    }
                    Type::Never => Type::empty_tuple(db, env),
                    resolved => Type::heterogeneous_tuple(db, env, [resolved]),
                })
            }
            TypeOperatorKind::MemberAccess(ty, field) => member_access(db, env, *ty, *field)
                .map_err(|()| {
                    TypeOperatorError::new(format!(
                        "Cannot access `.{}` on `{}`: expected a `Member` type",
                        field.as_str(),
                        ty.display(db, env)
                    ))
                }),
            TypeOperatorKind::UnpackMap {
                iterable,
                binder,
                body,
                conditions,
            } => Self::evaluate_unpack_map(db, env, &kind, *iterable, *binder, *body, conditions),
            TypeOperatorKind::TupleOf(args) => Self::evaluate_tuple_of(db, env, &kind, args),
            TypeOperatorKind::UnionOf(args) => Self::evaluate_union_of(db, env, &kind, args),
            TypeOperatorKind::NewProtocol(args) => {
                match Self::synthesis_members(db, env, "NewProtocol", args)? {
                    SynthesisMembers::Deferred => defer(kind),
                    SynthesisMembers::Dynamic => Ok(Type::any()),
                    SynthesisMembers::Members(members) => Ok(Type::protocol_from_members(
                        db,
                        env.program(db),
                        members
                            .into_iter()
                            .map(|member| (member.name, member.ty, member.qualifiers)),
                    )),
                }
            }
            TypeOperatorKind::NewTypedDict(args) => {
                match Self::synthesis_members(db, env, "NewTypedDict", args)? {
                    SynthesisMembers::Deferred => defer(kind),
                    SynthesisMembers::Dynamic => Ok(Type::any()),
                    SynthesisMembers::Members(members) => {
                        let schema: TypedDictSchema<'db> = members
                            .into_iter()
                            .map(|member| {
                                (
                                    member.name,
                                    TypedDictFieldBuilder::new(member.ty)
                                        .required(
                                            !member
                                                .qualifiers
                                                .contains(TypeQualifiers::NOT_REQUIRED),
                                        )
                                        .read_only(
                                            member.qualifiers.contains(TypeQualifiers::READ_ONLY),
                                        )
                                        .build(),
                                )
                            })
                            .collect();
                        Ok(Type::TypedDict(TypedDictType::from_schema_items(
                            db, schema,
                        )))
                    }
                }
            }
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

    /// Evaluate `*[body for binder in Iter[iterable] if c1 if c2 ...]` to a tuple of the body
    /// evaluated for every element that satisfies all conditions.
    fn evaluate_unpack_map(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        iterable: Type<'db>,
        binder: BoundTypeVarInstance<'db>,
        body: Type<'db>,
        conditions: &[Type<'db>],
    ) -> OperatorResult<'db> {
        let defer = || Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));

        let elements = match try_evaluate_sequence(db, env, iterable)? {
            Sequence::Fixed(elements) => elements,
            Sequence::Dynamic => return Ok(Type::homogeneous_tuple(db, env, Type::any())),
            Sequence::Deferred => return defer(),
            Sequence::Error => {
                return Err(TypeOperatorError::new(format!(
                    "Cannot iterate over `{}`",
                    iterable.display(db, env)
                )));
            }
        };

        let mut results = Vec::with_capacity(elements.len());
        'elements: for element in elements {
            for condition in conditions {
                let condition = condition.substitute_one_typevar(db, env, binder, element);
                match TypeBoolValue::of(db, env, condition) {
                    TypeBoolValue::Known(true) => {}
                    TypeBoolValue::Deferred => return defer(),
                    TypeBoolValue::Known(false)
                    | TypeBoolValue::Indeterminate
                    | TypeBoolValue::Dynamic(_)
                    | TypeBoolValue::NotABool => continue 'elements,
                }
            }
            let result = body.substitute_one_typevar(db, env, binder, element);
            if needs_deferral(db, env, result) {
                return defer();
            }
            results.push(result);
        }
        Ok(Type::heterogeneous_tuple(db, env, results))
    }

    /// Evaluate `tuple[...]` with unpacked arguments.
    fn evaluate_tuple_of(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        args: &[VariadicArg<'db>],
    ) -> OperatorResult<'db> {
        let mut builder = TupleSpecBuilder::with_capacity(args.len());
        for arg in args {
            match *arg {
                VariadicArg::Single(ty) => builder.push(ty),
                VariadicArg::Unpacked(ty) => {
                    let resolved = ty.resolve_type_alias(db);
                    if let Some(spec) = resolved.exact_tuple_instance_spec(db) {
                        builder = builder.concat(db, env, &spec);
                    } else if let Type::TypeVar(typevar) = resolved
                        && typevar.is_typevartuple(db)
                    {
                        builder = builder.concat_variadic_typevar(db, env, typevar);
                    } else {
                        match try_evaluate_sequence(db, env, resolved)? {
                            Sequence::Fixed(elements) => {
                                builder =
                                    builder.concat(db, env, &TupleSpec::heterogeneous(elements));
                            }
                            Sequence::Dynamic => {
                                builder =
                                    builder.concat(db, env, &TupleSpec::homogeneous(Type::any()));
                            }
                            Sequence::Deferred => {
                                return Ok(Type::TypeOperator(TypeOperatorType::new(
                                    db,
                                    kind.clone(),
                                )));
                            }
                            Sequence::Error => {
                                return Err(TypeOperatorError::new(format!(
                                    "Cannot unpack `{}` in `tuple`",
                                    ty.display(db, env)
                                )));
                            }
                        }
                    }
                }
            }
        }
        Ok(Type::tuple(TupleType::new(db, env, &builder.build())))
    }

    /// Evaluate `Union[...]` with unpacked arguments.
    fn evaluate_union_of(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        args: &[VariadicArg<'db>],
    ) -> OperatorResult<'db> {
        let mut builder = UnionBuilder::new(db, env);
        let mut dynamic = false;
        for arg in args {
            match *arg {
                VariadicArg::Single(ty) => builder = builder.add(ty),
                VariadicArg::Unpacked(ty) => match try_evaluate_sequence(db, env, ty)? {
                    Sequence::Fixed(elements) => {
                        for element in elements {
                            builder = builder.add(element);
                        }
                    }
                    Sequence::Dynamic => dynamic = true,
                    Sequence::Deferred => {
                        return Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));
                    }
                    Sequence::Error => {
                        return Err(TypeOperatorError::new(format!(
                            "Cannot unpack `{}` in `Union`",
                            ty.display(db, env)
                        )));
                    }
                },
            }
        }
        if dynamic {
            Ok(Type::any())
        } else {
            Ok(builder.build())
        }
    }

    /// Collects the `Member`s passed to `NewProtocol`/`NewTypedDict`.
    fn synthesis_members(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        operator_name: &str,
        args: &[VariadicArg<'db>],
    ) -> Result<SynthesisMembers<'db>, TypeOperatorError> {
        let mut member_types = Vec::with_capacity(args.len());
        for arg in args {
            match *arg {
                VariadicArg::Single(ty) => member_types.push(ty),
                VariadicArg::Unpacked(ty) => match try_evaluate_sequence(db, env, ty)? {
                    Sequence::Fixed(elements) => member_types.extend(elements),
                    Sequence::Dynamic => return Ok(SynthesisMembers::Dynamic),
                    Sequence::Deferred => return Ok(SynthesisMembers::Deferred),
                    Sequence::Error => {
                        return Err(TypeOperatorError::new(format!(
                            "Cannot unpack `{}` in `{operator_name}`",
                            ty.display(db, env)
                        )));
                    }
                },
            }
        }

        let mut members = Vec::with_capacity(member_types.len());
        let mut dynamic = false;
        for member_type in member_types {
            let resolved = member_type.resolve_type_alias(db);
            if resolved.is_dynamic() {
                dynamic = true;
                continue;
            }
            let Some([name, ty, quals, _, _]) = member_args(db, env, resolved) else {
                if needs_deferral(db, env, resolved) {
                    return Ok(SynthesisMembers::Deferred);
                }
                return Err(TypeOperatorError::new(format!(
                    "The arguments to `{operator_name}` must be `Member` types, but got `{}`",
                    member_type.display(db, env)
                )));
            };
            if needs_deferral(db, env, name) || needs_deferral(db, env, quals) {
                return Ok(SynthesisMembers::Deferred);
            }
            let Some(name) = name.as_string_literal() else {
                return Err(TypeOperatorError::new(format!(
                    "The name of a member passed to `{operator_name}` must be a string literal \
                     type, but got `{}`",
                    name.display(db, env)
                )));
            };
            let Some(qualifiers) = parse_qualifiers(db, quals) else {
                return Err(TypeOperatorError::new(format!(
                    "Invalid member qualifiers `{}` passed to `{operator_name}`",
                    quals.display(db, env)
                )));
            };
            members.push(SynthesisMember {
                name: Name::new(name.value(db)),
                ty,
                qualifiers,
            });
        }

        if dynamic {
            Ok(SynthesisMembers::Dynamic)
        } else {
            Ok(SynthesisMembers::Members(members))
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
            TypeOperatorKind::IsAssignable(..)
            | TypeOperatorKind::IsEquivalent(..)
            | TypeOperatorKind::Bool(_)
            | TypeOperatorKind::Not(_)
            | TypeOperatorKind::And(_)
            | TypeOperatorKind::Or(_) => KnownClass::Bool.to_instance(db, env),
            TypeOperatorKind::Attrs(_)
            | TypeOperatorKind::Members(_)
            | TypeOperatorKind::FromUnion(_)
            | TypeOperatorKind::UnpackMap { .. }
            | TypeOperatorKind::TupleOf(_) => Type::homogeneous_tuple(db, env, Type::any()),
            TypeOperatorKind::GetMember(..) => member_instance(db, env, [Type::any(); 5]),
            TypeOperatorKind::MemberAccess(_, MemberField::Name) => {
                KnownClass::Str.to_instance(db, env)
            }
            TypeOperatorKind::MemberAccess(_, MemberField::Quals) => all_qualifiers_type(db, env),
            TypeOperatorKind::MemberAccess(..)
            | TypeOperatorKind::GetMemberType(..)
            | TypeOperatorKind::UnionOf(_)
            | TypeOperatorKind::NewProtocol(_)
            | TypeOperatorKind::NewTypedDict(_) => Type::object(),
        }
    }

    /// The widest type that is assignable to every possible evaluation of this operator.
    ///
    /// For a conditional type `X if C else Y`, this is `X & Y`; for everything else, `Never`.
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
    /// Promotion and materialization only apply to operands that become part of the result
    /// (e.g. the branches of a conditional type): the inputs of computations are compared as
    /// written, so that (for example) promoting `Bool[Literal[True]]` does not change its value.
    ///
    /// A mapping never substitutes the binder of an unpacked comprehension in that
    /// comprehension's body or conditions (the binder shadows any outer binder with the same
    /// identity); the iterable is still mapped.
    pub(super) fn apply_type_mapping_impl<'a>(
        self,
        db: &'db dyn Db,
        type_mapping: &TypeMapping<'a, 'db>,
        tcx: TypeContext<'db>,
        visitor: &ApplyTypeMappingVisitor<'_, 'db>,
    ) -> Type<'db> {
        visitor.visit(db, Type::TypeOperator(self), type_mapping, || {
            let maps_inputs = !matches!(
                type_mapping,
                TypeMapping::Materialize(_) | TypeMapping::Promote(PromotionMode::On, _)
            );
            let map = |ty: Type<'db>, role: OperandRole| match role {
                OperandRole::Input if maps_inputs => {
                    ty.apply_type_mapping_impl(db, type_mapping, TypeContext::default(), visitor)
                }
                OperandRole::Input => ty,
                OperandRole::Output => ty.apply_type_mapping_impl(db, type_mapping, tcx, visitor),
            };

            let kind = match self.kind(db) {
                TypeOperatorKind::UnpackMap {
                    iterable,
                    binder,
                    body,
                    conditions,
                } if Type::TypeVar(*binder).apply_type_mapping_impl(
                    db,
                    type_mapping,
                    TypeContext::default(),
                    visitor,
                ) != Type::TypeVar(*binder) =>
                {
                    TypeOperatorKind::UnpackMap {
                        iterable: map(*iterable, OperandRole::Input),
                        binder: *binder,
                        body: *body,
                        conditions: conditions.clone(),
                    }
                }
                kind => kind
                    .try_map(|ty, role| Some(map(ty, role)))
                    .expect("infallible mapping"),
            };
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
        let kind = self.kind(db).try_map(|ty, _| {
            let normalized = ty.recursive_type_normalized_impl(db, env, div, true);
            if nested {
                normalized
            } else {
                Some(normalized.unwrap_or(div))
            }
        })?;
        Some(TypeOperatorType::new(db, kind))
    }

    /// Calls `f` on every operand of this operator.
    pub(super) fn for_each_operand(self, db: &'db dyn Db, mut f: impl FnMut(Type<'db>)) {
        self.kind(db).for_each_operand_with_role(|ty, _| f(ty));
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
    // A type variable that occurs in the input of a computation (e.g. a type-boolean operand or
    // the argument of `Attrs`) can change the result arbitrarily, so such occurrences are
    // invariant. Operands that become part of the result (the branches of a conditional type,
    // the elements of `tuple[...]`, the body of a comprehension) are covariant. The binder of a
    // comprehension is local to it.
    fn variance_of(
        self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        typevar: BoundTypeVarIdentity<'db>,
    ) -> VarianceTerm<'db> {
        let kind = self.kind(db);
        if let TypeOperatorKind::UnpackMap { binder, .. } = kind
            && binder.identity(db) == typevar
        {
            return VarianceTerm::BIVARIANT;
        }
        let mut terms = Vec::new();
        kind.for_each_operand_with_role(|ty, role| {
            terms.push(match role {
                OperandRole::Input => ty
                    .with_polarity(TypeVarVariance::Invariant)
                    .variance_of(db, env, typevar),
                OperandRole::Output => ty.variance_of(db, env, typevar),
            });
        });
        VarianceTerm::join(db, terms)
    }
}
