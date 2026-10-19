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
use super::type_alias::alias_has_type_operator;
use super::type_arguments;
pub(crate) use super::type_arguments::normalize_base;
use super::typed_dict::{TypedDictFieldBuilder, TypedDictSchema};
use super::variance::{VarianceInferable, VarianceTerm};
use super::visitor::{TypeCollector, TypeVisitor};
use super::{
    ApplyTypeMappingVisitor, BoundTypeVarIdentity, BoundTypeVarInstance, CallableType, DynamicType,
    IntersectionType, KnownClass, LiteralValueType, PromotionMode, Type, TypeAliasType,
    TypeContext, TypeMapping, TypeVarVariance, TypedDictType, UnionBuilder, UnionType, visitor,
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
    ///
    /// The body is [`VariadicArg::Single`] for a plain element, and [`VariadicArg::Unpacked`]
    /// for an element written as `Unpack[X]` or for a nested comprehension (a further `for`
    /// clause), in which case every element contributes several types.
    UnpackMap {
        iterable: Type<'db>,
        binder: BoundTypeVarInstance<'db>,
        body: VariadicArg<'db>,
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
    /// `GetArg[T, Base, Idx]`; `Base` is normalized by [`normalize_base`].
    GetArg(Type<'db>, Type<'db>, Type<'db>),
    /// `GetArgs[T, Base]`; `Base` is normalized by [`normalize_base`].
    GetArgs(Type<'db>, Type<'db>),
    /// `Length[T]`
    Length(Type<'db>),
    /// `Slice[S, Start, End]`
    Slice(Type<'db>, Type<'db>, Type<'db>),
    /// `GetSpecialAttr[T, Attr]`
    GetSpecialAttr(Type<'db>, Type<'db>),
    /// `RaiseError[S, *Ts]`. Only exists in this form while its operands contain unsolved type
    /// variables; once evaluated, it becomes [`DynamicType::RaisedError`].
    RaiseError {
        message: Type<'db>,
        args: Box<[VariadicArg<'db>]>,
    },
    /// `any(...)` (`all == false`) or `all(...)` (`all == true`). `elements` is the generator
    /// evaluated as an unpacked comprehension: a tuple of type booleans, or an unresolved
    /// `UnpackMap`.
    Quantifier { all: bool, elements: Type<'db> },
}

/// How an operand contributes to the result of an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandRole {
    /// The operand is an input of a computation (e.g. a type-boolean operand or the argument of
    /// `Attrs`). Such operands are compared as written: they are not promoted or materialized,
    /// and type variables in them are invariant.
    Input,
    /// Like [`OperandRole::Input`], the operand is not promoted or materialized, but the result
    /// varies covariantly with it (e.g. the sequence argument of `Slice`, whose result is a
    /// subsequence of it).
    CovariantInput,
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
            | Self::MemberAccess(a, _)
            | Self::Length(a)
            | Self::Quantifier { elements: a, .. } => f(*a, Input),
            Self::GetArgs(a, b) | Self::GetSpecialAttr(a, b) => {
                f(*a, Input);
                f(*b, Input);
            }
            Self::GetArg(a, b, c) => {
                f(*a, Input);
                f(*b, Input);
                f(*c, Input);
            }
            Self::Slice(sequence, start, end) => {
                f(*sequence, OperandRole::CovariantInput);
                f(*start, Input);
                f(*end, Input);
            }
            Self::RaiseError { message, args } => {
                f(*message, Input);
                args.iter().for_each(|arg| f(arg.ty(), Input));
            }
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
                f(body.ty(), Output);
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
                body: body.with_ty(f(body.ty(), Output)?),
                binder: *binder,
            },
            Self::TupleOf(args) => Self::TupleOf(Self::map_args(args, |ty| f(ty, Output))?),
            Self::UnionOf(args) => Self::UnionOf(Self::map_args(args, |ty| f(ty, Output))?),
            Self::NewProtocol(args) => Self::NewProtocol(Self::map_args(args, |ty| f(ty, Input))?),
            Self::NewTypedDict(args) => {
                Self::NewTypedDict(Self::map_args(args, |ty| f(ty, Input))?)
            }
            Self::GetArg(a, b, c) => {
                let [a, b, c] = [*a, *b, *c].map(|ty| f(ty, Input));
                Self::GetArg(a?, b?, c?)
            }
            Self::GetArgs(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::GetArgs(a?, b?)
            }
            Self::Length(a) => Self::Length(f(*a, Input)?),
            Self::Slice(sequence, start, end) => {
                let sequence = f(*sequence, OperandRole::CovariantInput);
                let [start, end] = [*start, *end].map(|ty| f(ty, Input));
                Self::Slice(sequence?, start?, end?)
            }
            Self::GetSpecialAttr(a, b) => {
                let [a, b] = [*a, *b].map(|ty| f(ty, Input));
                Self::GetSpecialAttr(a?, b?)
            }
            Self::RaiseError { message, args } => {
                let message = f(*message, Input)?;
                Self::RaiseError {
                    message,
                    args: Self::map_args(args, |ty| f(ty, Input))?,
                }
            }
            Self::Quantifier { all, elements } => Self::Quantifier {
                all: *all,
                elements: f(*elements, Input)?,
            },
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
    /// A sequence with an unbounded part: the `prefix`, any number of `element`s, then the
    /// `suffix` (e.g. `tuple[int, *tuple[str, ...], bytes]`).
    Unbounded {
        prefix: Vec<Type<'db>>,
        element: Type<'db>,
        suffix: Vec<Type<'db>>,
    },
    /// A sequence of unknown length and contents, carrying the dynamic type it stems from
    /// (`Any`, `Unknown`, a raised error, ...).
    Dynamic(Type<'db>),
    /// The sequence depends on unsolved type variables (including `tuple[*Ts]`).
    Deferred,
    /// The type does not describe a supported sequence.
    Error,
}

/// The payload of [`DynamicType::RaisedError`]: the message of an evaluated
/// `RaiseError[S, *Ts]`.
#[salsa::interned(debug, heap_size=ruff_memory_usage::heap_size)]
pub struct RaisedErrorType<'db> {
    #[returns(deref)]
    pub(crate) message: Box<str>,
}

// The Salsa heap is tracked separately.
impl get_size2::GetSize for RaisedErrorType<'_> {}

/// Returns `true` if `ty` is a type alias with type operators whose arguments contain unsolved
/// type variables. Such an alias is not resolved while evaluating an operator: its value is not
/// known yet, and resolving a recursive alias (`tuple[*Zip[DropLast[T], DropLast[S]], ...]`)
/// would expand it without end.
pub(crate) fn is_unresolved_alias<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> bool {
    match ty {
        Type::TypeAlias(alias) => needs_deferral(db, env, ty) && alias_has_type_operator(db, alias),
        _ => false,
    }
}

/// Returns the raised error marker if `ty` (after resolving type aliases) is one.
fn as_raised_error<'db>(db: &'db dyn Db, ty: Type<'db>) -> Option<Type<'db>> {
    match ty.resolve_type_alias(db) {
        raised @ Type::Dynamic(DynamicType::RaisedError(_)) => Some(raised),
        _ => None,
    }
}

/// The value of a type boolean operand, as far as it can be determined.
#[derive(Debug, Clone, Copy)]
enum TypeBoolValue<'db> {
    /// `Literal[True]` or `Literal[False]`.
    Known(bool),
    /// `bool`: a type boolean that is not statically known.
    Indeterminate,
    /// A raised error (`DynamicType::RaisedError`); it propagates to the result.
    Raised(Type<'db>),
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
            Type::Dynamic(DynamicType::RaisedError(_)) => Self::Raised(resolved),
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

    // The bounds and defaults of a signature's own type parameters (such as the synthetic
    // `Self` of a method, bounded by the unspecialized class) do not make the signature depend
    // on outer type variables.
    fn visit_bound_type_var_type(&self, db: &'db dyn Db, bound_typevar: BoundTypeVarInstance<'db>) {
        if !self.bound.borrow().contains(&bound_typevar.identity(db)) {
            super::typevar::walk_bound_type_var_type(db, bound_typevar, self);
        }
    }
}

fn type_bool<'db>(value: bool) -> Type<'db> {
    Type::LiteralValue(LiteralValueType::unpromotable(value))
}

/// Evaluate the `and` (`short_circuit == false`) or `or` (`short_circuit == true`) of type
/// booleans; `any(...)` and `all(...)` use the same rules over their evaluated elements. Returns
/// `None` if the result must be deferred.
///
/// Any operand equal to `short_circuit` decides the result, even if other operands are still
/// unresolved. Without operands, the result is `not short_circuit`.
fn evaluate_connective<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    operands: &[Type<'db>],
    short_circuit: bool,
) -> Option<Type<'db>> {
    let mut deferred = false;
    let mut raised = None;
    let mut dynamic = None;
    let mut indeterminate = false;
    let mut invalid = false;

    for operand in operands {
        match TypeBoolValue::of(db, env, *operand) {
            TypeBoolValue::Known(value) if value == short_circuit => {
                return Some(type_bool(short_circuit));
            }
            TypeBoolValue::Known(_) => {}
            TypeBoolValue::Indeterminate => indeterminate = true,
            TypeBoolValue::Raised(ty) => raised = raised.or(Some(ty)),
            TypeBoolValue::Dynamic(ty) => dynamic = dynamic.or(Some(ty)),
            TypeBoolValue::Deferred => deferred = true,
            TypeBoolValue::NotABool => invalid = true,
        }
    }

    if invalid {
        Some(Type::unknown())
    } else if deferred {
        None
    } else if let Some(dynamic) = raised.or(dynamic) {
        Some(dynamic)
    } else if indeterminate {
        Some(KnownClass::Bool.to_instance(db, env))
    } else {
        Some(type_bool(!short_circuit))
    }
}

/// The result of splicing an unpacked sequence into a tuple.
enum Spliced<'db> {
    Done(TupleSpecBuilder<'db>),
    Deferred,
    /// The unpacked sequence is a raised error; the result is that error.
    Raised(Type<'db>),
}

/// Splices the unpacked sequence `ty` (`*ty`) onto the end of `builder`. `context` names the
/// enclosing construct in error messages.
fn concat_unpacked<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    builder: TupleSpecBuilder<'db>,
    ty: Type<'db>,
    context: &str,
) -> Result<Spliced<'db>, TypeOperatorError> {
    if is_unresolved_alias(db, env, ty) {
        return Ok(Spliced::Deferred);
    }
    let resolved = ty.resolve_type_alias(db);
    if let Some(spec) = resolved.exact_tuple_instance_spec(db) {
        return Ok(Spliced::Done(builder.concat(db, env, &spec)));
    }
    if let Type::TypeVar(typevar) = resolved
        && typevar.is_typevartuple(db)
    {
        return Ok(Spliced::Done(
            builder.concat_variadic_typevar(db, env, typevar),
        ));
    }
    Ok(match try_evaluate_sequence(db, env, resolved)? {
        Sequence::Fixed(elements) => {
            Spliced::Done(builder.concat(db, env, &TupleSpec::heterogeneous(elements)))
        }
        Sequence::Unbounded {
            prefix,
            element,
            suffix,
        } => {
            let mut builder = builder.concat(db, env, &TupleSpec::heterogeneous(prefix));
            builder = builder.concat(db, env, &TupleSpec::homogeneous(element));
            Spliced::Done(builder.concat(db, env, &TupleSpec::heterogeneous(suffix)))
        }
        Sequence::Dynamic(dynamic) if as_raised_error(db, dynamic).is_some() => {
            Spliced::Raised(dynamic)
        }
        Sequence::Dynamic(dynamic) => {
            Spliced::Done(builder.concat(db, env, &TupleSpec::homogeneous(dynamic)))
        }
        Sequence::Deferred => Spliced::Deferred,
        Sequence::Error => {
            return Err(TypeOperatorError::new(format!(
                "Cannot unpack `{}` in `{context}`",
                ty.display(db, env)
            )));
        }
    })
}

/// Returns the raised errors (`DynamicType::RaisedError`) in `ty`, in order and without
/// duplicates, for reporting where a type is used.
///
/// The values of type aliases are searched if they contain type operators (see
/// [`alias_has_type_operator`]). Unresolved operators are not searched: an error in them has not
/// been raised yet.
pub(crate) fn find_raised_errors<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Vec<RaisedErrorType<'db>> {
    struct RaisedErrorVisitor<'db, 'a> {
        env: &'a ProgramEnvironment<'db>,
        recursion_guard: TypeCollector<'db>,
        seen_aliases: RefCell<Vec<TypeAliasType<'db>>>,
        found: RefCell<Vec<RaisedErrorType<'db>>>,
    }

    impl<'db> visitor::TypeVisitor<'db> for RaisedErrorVisitor<'db, '_> {
        fn program_environment(&self) -> &ProgramEnvironment<'db> {
            self.env
        }

        fn should_visit_lazy_type_attributes(&self) -> bool {
            false
        }

        fn visit_type(&self, db: &'db dyn Db, ty: Type<'db>) {
            match ty {
                Type::Dynamic(DynamicType::RaisedError(raised)) => {
                    let mut found = self.found.borrow_mut();
                    if !found.contains(&raised) {
                        found.push(raised);
                    }
                }
                Type::TypeOperator(_) => {}
                Type::TypeAlias(alias) => {
                    if self.seen_aliases.borrow().contains(&alias) {
                        return;
                    }
                    self.seen_aliases.borrow_mut().push(alias);
                    if alias_has_type_operator(db, alias) {
                        self.visit_type(db, alias.value_type(db));
                    } else if let Some(specialization) = alias.specialization(db) {
                        walk_specialization_types(db, specialization, self);
                    }
                }
                _ => visitor::walk_type_with_recursion_guard(db, ty, self, &self.recursion_guard),
            }
        }
    }

    let visitor = RaisedErrorVisitor {
        env,
        recursion_guard: TypeCollector::default(),
        seen_aliases: RefCell::default(),
        found: RefCell::default(),
    };
    visitor.visit_type(db, ty);
    visitor.found.into_inner()
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
/// Fixed-length tuples produce their elements, `Never` produces no elements, unbounded tuples
/// produce their prefix, the element type of their unbounded part, and their suffix, dynamic types
/// (and `tuple[Any, ...]`) produce a dynamic sequence, and `tuple[*Ts]` is deferred. Unions and
/// non-tuples are errors.
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
    if is_unresolved_alias(db, env, ty) {
        return Ok(Sequence::Deferred);
    }
    let resolved = ty.resolve_type_alias(db);
    match resolved {
        Type::Never => return Ok(Sequence::Fixed(Vec::new())),
        Type::Dynamic(_) => return Ok(Sequence::Dynamic(resolved)),
        _ => {}
    }
    if let Some(spec) = resolved.exact_tuple_instance_spec(db) {
        return Ok(match spec.as_ref() {
            Tuple::Fixed(fixed) => Sequence::Fixed(fixed.all_elements().to_vec()),
            Tuple::Variable(variable) => match variable.variable() {
                // `*Ts` must not be iterated with `iter_element_types`, which turns it into
                // `object`: wait until `Ts` is solved.
                VariableSegment::TypeVarTuple(_) => Sequence::Deferred,
                VariableSegment::Homogeneous(element)
                    if element.is_dynamic()
                        && variable.prefix_elements().is_empty()
                        && variable.suffix_elements().is_empty() =>
                {
                    Sequence::Dynamic(element)
                }
                VariableSegment::Homogeneous(element) => Sequence::Unbounded {
                    prefix: variable.prefix_elements().to_vec(),
                    element,
                    suffix: variable.suffix_elements().to_vec(),
                },
            },
        });
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

/// Returns the members of `ty` viewed as a union, after resolving type aliases. Union members that
/// are themselves aliases of unions are flattened recursively (in a type expression, `A | B`
/// keeps the alias `A` as a member). `Never` has no members.
fn flattened_union_elements<'db>(db: &'db dyn Db, ty: Type<'db>) -> Vec<Type<'db>> {
    fn collect<'db>(
        db: &'db dyn Db,
        ty: Type<'db>,
        seen_aliases: &mut Vec<Type<'db>>,
        out: &mut Vec<Type<'db>>,
    ) {
        if matches!(ty, Type::TypeAlias(_)) {
            if seen_aliases.contains(&ty) {
                return;
            }
            seen_aliases.push(ty);
        }
        match ty.resolve_type_alias(db) {
            Type::Union(union) => {
                for element in union.elements(db) {
                    collect(db, *element, seen_aliases, out);
                }
            }
            Type::Never => {}
            resolved => {
                if !out.contains(&resolved) {
                    out.push(resolved);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(db, ty, &mut Vec::new(), &mut out);
    out
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
        .map(|arg| flattened_union_elements(db, *arg))
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
    /// An argument is dynamic; the result is that dynamic type.
    Dynamic(Type<'db>),
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

        // An operator that reads a raised error evaluates to that error (lazy propagation: an
        // error in an operand that only becomes part of the result is kept where it is).
        let mut raised = None;
        kind.for_each_operand_with_role(|ty, role| {
            if raised.is_none() && role != OperandRole::Output && !is_unresolved_alias(db, env, ty)
            {
                raised = as_raised_error(db, ty);
            }
        });
        if let Some(raised) = raised {
            return Ok(raised);
        }

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
                TypeBoolValue::Raised(dynamic) | TypeBoolValue::Dynamic(dynamic) => dynamic,
                TypeBoolValue::Deferred => return defer(kind),
                TypeBoolValue::NotABool => Type::unknown(),
            }),
            TypeOperatorKind::And(operands) => Ok(evaluate_connective(db, env, operands, false)
                .unwrap_or_else(|| Type::TypeOperator(TypeOperatorType::new(db, kind.clone())))),
            TypeOperatorKind::Or(operands) => Ok(evaluate_connective(db, env, operands, true)
                .unwrap_or_else(|| Type::TypeOperator(TypeOperatorType::new(db, kind.clone())))),
            TypeOperatorKind::Conditional {
                condition,
                if_true,
                if_false,
            } => Ok(match TypeBoolValue::of(db, env, *condition) {
                TypeBoolValue::Known(true) => *if_true,
                TypeBoolValue::Known(false)
                | TypeBoolValue::Indeterminate
                | TypeBoolValue::NotABool => *if_false,
                TypeBoolValue::Raised(raised) => raised,
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
                Ok(Type::heterogeneous_tuple(
                    db,
                    env,
                    flattened_union_elements(db, *ty),
                ))
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
                    SynthesisMembers::Dynamic(dynamic) => Ok(dynamic),
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
                    SynthesisMembers::Dynamic(dynamic) => Ok(dynamic),
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
            TypeOperatorKind::GetArg(ty, base, index) => {
                Self::evaluate_lifted(db, env, &kind, &[*ty, *base, *index], |args| {
                    type_arguments::get_arg(db, env, args[0], args[1], Some(args[2]))
                })
            }
            TypeOperatorKind::GetArgs(ty, base) => {
                Self::evaluate_lifted(db, env, &kind, &[*ty, *base], |args| {
                    type_arguments::get_arg(db, env, args[0], args[1], None)
                })
            }
            TypeOperatorKind::Length(ty) => Self::evaluate_lifted(db, env, &kind, &[*ty], |args| {
                type_arguments::length(db, env, args[0])
            }),
            TypeOperatorKind::Slice(sequence, start, end) => {
                Self::evaluate_lifted(db, env, &kind, &[*sequence, *start, *end], |args| {
                    type_arguments::slice(db, env, args[0], args[1], args[2])
                })
            }
            TypeOperatorKind::GetSpecialAttr(ty, attribute) => {
                Self::evaluate_lifted(db, env, &kind, &[*ty, *attribute], |args| {
                    type_arguments::get_special_attr(db, env, args[0], args[1])
                })
            }
            TypeOperatorKind::RaiseError { message, args } => {
                Self::evaluate_raise_error(db, env, &kind, *message, args)
            }
            TypeOperatorKind::Quantifier { all, elements } => {
                // `any` is the `or` of the elements, `all` their `and`. An unbounded part counts
                // as one element.
                let short_circuit = !*all;
                let elements = match try_evaluate_sequence(db, env, *elements)? {
                    Sequence::Fixed(elements) => elements,
                    Sequence::Unbounded {
                        mut prefix,
                        element,
                        suffix,
                    } => {
                        prefix.push(element);
                        prefix.extend(suffix);
                        prefix
                    }
                    Sequence::Dynamic(dynamic) => return Ok(dynamic),
                    Sequence::Deferred => return defer(kind),
                    Sequence::Error => {
                        return Err(TypeOperatorError::new(format!(
                            "Cannot iterate over `{}`",
                            elements.display(db, env)
                        )));
                    }
                };
                Ok(evaluate_connective(db, env, &elements, short_circuit)
                    .unwrap_or_else(|| Type::TypeOperator(TypeOperatorType::new(db, kind.clone()))))
            }
        }
    }

    /// Evaluates an operator whose arguments are all lifted over unions (PEP 827 "Basic
    /// operators"). The operator is deferred while any argument contains unsolved type variables;
    /// otherwise, for every combination, the first dynamic argument is the result, and `f`
    /// evaluates the rest.
    fn evaluate_lifted(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        args: &[Type<'db>],
        f: impl Fn(&[Type<'db>]) -> Result<Type<'db>, type_arguments::EvaluationError>,
    ) -> OperatorResult<'db> {
        if args.iter().any(|arg| needs_deferral(db, env, *arg)) {
            return Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));
        }
        lift_over_unions(db, env, args, |args| {
            if let Some(dynamic) = args.iter().find(|arg| arg.is_dynamic()) {
                return Ok(*dynamic);
            }
            f(args).map_err(TypeOperatorError::new)
        })
    }

    /// Evaluate `RaiseError[message, *args]` to a raised error marker.
    fn evaluate_raise_error(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        message: Type<'db>,
        args: &[VariadicArg<'db>],
    ) -> OperatorResult<'db> {
        let defer = || Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));
        if needs_deferral(db, env, message)
            || args.iter().any(|arg| needs_deferral(db, env, arg.ty()))
        {
            return defer();
        }
        let message = message.resolve_type_alias(db);
        let Some(text) = message.as_string_literal() else {
            return Err(TypeOperatorError::new(format!(
                "The message of `RaiseError` must be a string literal type, but got `{}`",
                message.display(db, env)
            )));
        };

        // The extra arguments are displayed after expanding aliases at their outermost level.
        let display = |ty: Type<'db>| ty.resolve_type_alias(db).display(db, env).to_string();
        let mut parts = Vec::with_capacity(args.len());
        for arg in args {
            match *arg {
                VariadicArg::Single(ty) => parts.push(display(ty)),
                VariadicArg::Unpacked(ty) => match try_evaluate_sequence(db, env, ty) {
                    Ok(Sequence::Fixed(elements)) => {
                        parts.extend(elements.into_iter().map(display))
                    }
                    Ok(Sequence::Deferred) => return defer(),
                    Ok(Sequence::Unbounded { .. } | Sequence::Dynamic(_) | Sequence::Error)
                    | Err(_) => parts.push(format!("*{}", display(ty))),
                },
            }
        }

        let mut message = text.value(db).to_string();
        if !parts.is_empty() {
            message.push_str(": ");
            message.push_str(&parts.join(", "));
        }
        Ok(Type::Dynamic(DynamicType::RaisedError(
            RaisedErrorType::new(db, message.into_boxed_str()),
        )))
    }

    /// Evaluate `*[body for binder in Iter[iterable] if c1 if c2 ...]` to a tuple of the body
    /// evaluated for every element that satisfies all conditions.
    ///
    /// The fixed elements of the iterable are mapped one by one. The unbounded part of an
    /// unbounded iterable is mapped once, with its element type: it is kept as an unbounded part
    /// of the result if the element passes the conditions, and dropped otherwise.
    fn evaluate_unpack_map(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        iterable: Type<'db>,
        binder: BoundTypeVarInstance<'db>,
        body: VariadicArg<'db>,
        conditions: &[Type<'db>],
    ) -> OperatorResult<'db> {
        /// The result of mapping one element of the iterable.
        enum Mapped<'db> {
            Skipped,
            Kept(Type<'db>),
            Deferred,
            Raised(Type<'db>),
        }

        let defer = || Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));

        let (prefix, unbounded, suffix) = match try_evaluate_sequence(db, env, iterable)? {
            Sequence::Fixed(elements) => (elements, None, Vec::new()),
            Sequence::Unbounded {
                prefix,
                element,
                suffix,
            } => (prefix, Some(element), suffix),
            Sequence::Dynamic(dynamic) => {
                return Ok(if as_raised_error(db, dynamic).is_some() {
                    dynamic
                } else {
                    Type::homogeneous_tuple(db, env, dynamic)
                });
            }
            Sequence::Deferred => return defer(),
            Sequence::Error => {
                return Err(TypeOperatorError::new(format!(
                    "Cannot iterate over `{}`",
                    iterable.display(db, env)
                )));
            }
        };

        let map_element = |element: Type<'db>| {
            for condition in conditions {
                let condition = condition.substitute_one_typevar(db, env, binder, element);
                match TypeBoolValue::of(db, env, condition) {
                    TypeBoolValue::Known(true) => {}
                    TypeBoolValue::Deferred => return Mapped::Deferred,
                    // Skipping the element would lose the error.
                    TypeBoolValue::Raised(raised) => return Mapped::Raised(raised),
                    TypeBoolValue::Known(false)
                    | TypeBoolValue::Indeterminate
                    | TypeBoolValue::Dynamic(_)
                    | TypeBoolValue::NotABool => return Mapped::Skipped,
                }
            }
            let result = body.ty().substitute_one_typevar(db, env, binder, element);
            if needs_deferral(db, env, result) {
                Mapped::Deferred
            } else {
                Mapped::Kept(result)
            }
        };

        // Maps one fixed element and splices the result onto `builder`.
        let add_fixed = |mut builder: TupleSpecBuilder<'db>, element: Type<'db>| {
            let result = match map_element(element) {
                Mapped::Skipped => return Ok(Spliced::Done(builder)),
                Mapped::Kept(result) => result,
                Mapped::Deferred => return Ok(Spliced::Deferred),
                Mapped::Raised(raised) => return Ok(Spliced::Raised(raised)),
            };
            match body {
                VariadicArg::Single(_) => {
                    builder.push(result);
                    Ok(Spliced::Done(builder))
                }
                VariadicArg::Unpacked(_) => concat_unpacked(db, env, builder, result, "tuple"),
            }
        };

        let mut builder = TupleSpecBuilder::with_capacity(prefix.len() + suffix.len());
        for element in prefix {
            match add_fixed(builder, element)? {
                Spliced::Done(next) => builder = next,
                Spliced::Deferred => return defer(),
                Spliced::Raised(raised) => return Ok(raised),
            }
        }
        if let Some(element) = unbounded {
            match map_element(element) {
                Mapped::Skipped => {}
                Mapped::Kept(result) => {
                    if matches!(body, VariadicArg::Unpacked(_)) {
                        return Err(TypeOperatorError::new(
                            "Each element of an unbounded tuple must map to exactly one type",
                        ));
                    }
                    builder = builder.concat(db, env, &TupleSpec::homogeneous(result));
                }
                Mapped::Deferred => return defer(),
                Mapped::Raised(raised) => return Ok(raised),
            }
        }
        for element in suffix {
            match add_fixed(builder, element)? {
                Spliced::Done(next) => builder = next,
                Spliced::Deferred => return defer(),
                Spliced::Raised(raised) => return Ok(raised),
            }
        }
        Ok(Type::tuple(TupleType::new(db, env, &builder.build())))
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
                    match concat_unpacked(db, env, builder, ty, "tuple")? {
                        Spliced::Done(next) => builder = next,
                        Spliced::Deferred => {
                            return Ok(Type::TypeOperator(TypeOperatorType::new(db, kind.clone())));
                        }
                        Spliced::Raised(raised) => return Ok(raised),
                    }
                }
            }
        }
        Ok(Type::tuple(TupleType::new(db, env, &builder.build())))
    }

    /// Evaluate `Union[...]` with unpacked arguments. All element types of an unpacked
    /// unbounded tuple, including the one of its unbounded part, end up in the union.
    fn evaluate_union_of(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        kind: &TypeOperatorKind<'db>,
        args: &[VariadicArg<'db>],
    ) -> OperatorResult<'db> {
        let mut builder = UnionBuilder::new(db, env);
        let mut dynamic = None;
        for arg in args {
            match *arg {
                VariadicArg::Single(ty) => builder = builder.add(ty),
                VariadicArg::Unpacked(ty) => match try_evaluate_sequence(db, env, ty)? {
                    Sequence::Fixed(elements) => {
                        for element in elements {
                            builder = builder.add(element);
                        }
                    }
                    Sequence::Unbounded {
                        prefix,
                        element,
                        suffix,
                    } => {
                        for element in prefix.into_iter().chain([element]).chain(suffix) {
                            builder = builder.add(element);
                        }
                    }
                    Sequence::Dynamic(ty) if as_raised_error(db, ty).is_some() => return Ok(ty),
                    Sequence::Dynamic(ty) => dynamic = dynamic.or(Some(ty)),
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
        Ok(dynamic.unwrap_or_else(|| builder.build()))
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
                    Sequence::Unbounded { .. } => {
                        let what = if operator_name == "NewProtocol" {
                            "a protocol"
                        } else {
                            "a TypedDict"
                        };
                        return Err(TypeOperatorError::new(format!(
                            "Cannot create {what} from an unbounded number of members"
                        )));
                    }
                    Sequence::Dynamic(dynamic) => return Ok(SynthesisMembers::Dynamic(dynamic)),
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
        let mut dynamic = None;
        for member_type in member_types {
            let resolved = member_type.resolve_type_alias(db);
            if resolved.is_dynamic() {
                if as_raised_error(db, resolved).is_some() {
                    return Ok(SynthesisMembers::Dynamic(resolved));
                }
                dynamic = dynamic.or(Some(resolved));
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

        Ok(match dynamic {
            Some(dynamic) => SynthesisMembers::Dynamic(dynamic),
            None => SynthesisMembers::Members(members),
        })
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
            | TypeOperatorKind::Or(_)
            | TypeOperatorKind::Quantifier { .. } => KnownClass::Bool.to_instance(db, env),
            TypeOperatorKind::Attrs(_)
            | TypeOperatorKind::Members(_)
            | TypeOperatorKind::FromUnion(_)
            | TypeOperatorKind::UnpackMap { .. }
            | TypeOperatorKind::TupleOf(_)
            | TypeOperatorKind::GetArgs(..)
            | TypeOperatorKind::Slice(..) => Type::homogeneous_tuple(db, env, Type::any()),
            TypeOperatorKind::GetMember(..) => member_instance(db, env, [Type::any(); 5]),
            TypeOperatorKind::MemberAccess(_, MemberField::Name)
            | TypeOperatorKind::GetSpecialAttr(..) => KnownClass::Str.to_instance(db, env),
            TypeOperatorKind::MemberAccess(_, MemberField::Quals) => all_qualifiers_type(db, env),
            TypeOperatorKind::Length(_) => UnionType::from_two_elements(
                db,
                env,
                KnownClass::Int.to_instance(db, env),
                Type::none(db, env),
            ),
            // Once evaluated, `RaiseError` is a dynamic type.
            TypeOperatorKind::RaiseError { .. } => Type::unknown(),
            TypeOperatorKind::MemberAccess(..)
            | TypeOperatorKind::GetMemberType(..)
            | TypeOperatorKind::UnionOf(_)
            | TypeOperatorKind::NewProtocol(_)
            | TypeOperatorKind::NewTypedDict(_)
            | TypeOperatorKind::GetArg(..) => Type::object(),
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
            // Once evaluated, `RaiseError` is a dynamic type.
            TypeOperatorKind::RaiseError { .. } => Type::unknown(),
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
            let env = visitor.env;
            let maps_inputs = !matches!(
                type_mapping,
                TypeMapping::Materialize(_) | TypeMapping::Promote(PromotionMode::On, _)
            );
            let map = |ty: Type<'db>, role: OperandRole| match role {
                OperandRole::Input | OperandRole::CovariantInput if maps_inputs => {
                    ty.apply_type_mapping_impl(db, type_mapping, TypeContext::default(), visitor)
                }
                OperandRole::Input | OperandRole::CovariantInput => ty,
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
                // Only the branch selected by the mapped condition is mapped, so that a
                // recursive alias stops expanding at its base case.
                TypeOperatorKind::Conditional {
                    condition,
                    if_true,
                    if_false,
                } => {
                    let condition = map(*condition, OperandRole::Input);
                    match TypeBoolValue::of(db, env, condition) {
                        TypeBoolValue::Known(true) => return map(*if_true, OperandRole::Output),
                        TypeBoolValue::Known(false)
                        | TypeBoolValue::Indeterminate
                        | TypeBoolValue::NotABool => return map(*if_false, OperandRole::Output),
                        TypeBoolValue::Raised(raised) => return raised,
                        TypeBoolValue::Dynamic(_) => {
                            return UnionType::from_two_elements(
                                db,
                                env,
                                map(*if_true, OperandRole::Output),
                                map(*if_false, OperandRole::Output),
                            );
                        }
                        TypeBoolValue::Deferred => TypeOperatorKind::Conditional {
                            condition,
                            if_true: map(*if_true, OperandRole::Output),
                            if_false: map(*if_false, OperandRole::Output),
                        },
                    }
                }
                // Operands are mapped from left to right until one decides the result.
                TypeOperatorKind::And(operands) | TypeOperatorKind::Or(operands) => {
                    let short_circuit = matches!(self.kind(db), TypeOperatorKind::Or(_));
                    let mut mapped = Vec::with_capacity(operands.len());
                    for operand in operands {
                        let operand = map(*operand, OperandRole::Input);
                        if matches!(
                            TypeBoolValue::of(db, env, operand),
                            TypeBoolValue::Known(value) if value == short_circuit
                        ) {
                            return type_bool(short_circuit);
                        }
                        mapped.push(operand);
                    }
                    if short_circuit {
                        TypeOperatorKind::Or(mapped.into_boxed_slice())
                    } else {
                        TypeOperatorKind::And(mapped.into_boxed_slice())
                    }
                }
                kind => kind
                    .try_map(|ty, role| Some(map(ty, role)))
                    .expect("infallible mapping"),
            };
            // An unpacked operator that failed during this evaluation (and so became `Unknown`)
            // makes the enclosing variadic result `Unknown`, as a failed unpacked comprehension
            // does when it is evaluated in place.
            if let (
                TypeOperatorKind::TupleOf(before)
                | TypeOperatorKind::UnionOf(before)
                | TypeOperatorKind::NewProtocol(before)
                | TypeOperatorKind::NewTypedDict(before),
                TypeOperatorKind::TupleOf(after)
                | TypeOperatorKind::UnionOf(after)
                | TypeOperatorKind::NewProtocol(after)
                | TypeOperatorKind::NewTypedDict(after),
            ) = (self.kind(db), &kind)
                && before.iter().zip(after.iter()).any(|(before, after)| {
                    matches!(before, VariadicArg::Unpacked(Type::TypeOperator(_)))
                        && after.ty().is_unknown()
                })
            {
                return Type::unknown();
            }
            Self::evaluate_or_defer(db, env, kind)
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
                OperandRole::CovariantInput | OperandRole::Output => {
                    ty.variance_of(db, env, typevar)
                }
            });
        });
        VarianceTerm::join(db, terms)
    }
}
