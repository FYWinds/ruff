//! Evaluation of the PEP 827 operators that read type arguments, tuple shapes and special class
//! attributes: `GetArg`, `GetArgs`, `Length`, `Slice` and `GetSpecialAttr`.
//!
//! The functions here evaluate a single combination of arguments; lifting over unions, dynamic
//! arguments and deferral are handled by the caller in [`super::type_operator`].

use ty_module_resolver::file_to_module;

use super::class::ClassType;
use super::extended_callable;
use super::member_introspection::string_literal_type;
use super::special_form::SpecialFormType;
use super::string_ops::StringOperand;
use super::tuple::{TupleSpec, TupleSpecBuilder, TupleType};
use super::type_operator::{TypeOperatorKind, strip_annotated};
use super::{ClassLiteral, KnownClass, LiteralValueType, Type};
use crate::Db;
use crate::ProgramEnvironment;
use crate::subscript::{PyIndex, PySlice};

/// An error from evaluating one of the operators in this module, as a message.
pub(super) type EvaluationError = String;

/// Normalizes the `Base` argument of `GetArg`/`GetArgs`, as inferred as a type expression.
///
/// Only the class of `Base` matters, not its type arguments: the result is the class literal of
/// the origin class (`tuple` for any tuple type, `type` for `type[...]`), or the `Callable`
/// special form for a callable type. Dynamic types are returned unchanged. Returns `None` if
/// `Base` does not denote a class.
pub(crate) fn normalize_base<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<Type<'db>> {
    let class = match ty.resolve_type_alias(db) {
        dynamic @ Type::Dynamic(_) => return Some(dynamic),
        Type::Callable(_) => return Some(Type::SpecialForm(SpecialFormType::TypingCallable)),
        // A bare `Overloaded` is only valid as a `Base` (P5-34).
        overloaded @ Type::SpecialForm(SpecialFormType::Overloaded) => return Some(overloaded),
        Type::SubclassOf(_) => return Some(KnownClass::Type.to_class_literal(db, env)),
        Type::NominalInstance(instance) => instance.class(db, env),
        Type::ProtocolInstance(instance) => *instance.class_origin(db)?,
        Type::TypedDict(typed_dict) => typed_dict.defining_class()?,
        _ => return None,
    };
    Some(Type::ClassLiteral(class.class_literal(db)))
}

/// Returns the class name used to display a normalized `Base` argument (see [`normalize_base`]).
pub(crate) fn base_display_name<'db>(db: &'db dyn Db, base: Type<'db>) -> Option<&'db str> {
    match base {
        Type::ClassLiteral(class) => Some(class.name(db).as_str()),
        Type::SpecialForm(form) => Some(form.name()),
        _ => None,
    }
}

/// Parses an index argument: an `int` literal type or a `bool` literal type (as `0`/`1`).
fn int_literal(ty: Type<'_>) -> Option<i64> {
    let literal = ty.as_literal_value()?;
    literal
        .as_int()
        .or_else(|| literal.as_bool().map(i64::from))
}

/// Interprets `ty` as an instance of the class `base` and returns its type arguments as a tuple
/// spec (flattening the arguments of a `TypeVarTuple` parameter), or `None` if `ty` is not an
/// instance of a subclass of `base`.
fn type_arguments_as<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    base: ClassLiteral<'db>,
) -> Option<TupleSpec<'db>> {
    match base.known(db) {
        Some(KnownClass::Tuple) => {
            return ty
                .tuple_instance_spec(db, env)
                .map(std::borrow::Cow::into_owned);
        }
        Some(KnownClass::Type) => {
            if let Type::SubclassOf(subclass_of) = ty {
                return Some(TupleSpec::heterogeneous([subclass_of.to_instance(db, env)]));
            }
        }
        _ => {}
    }

    let class = instance_class(db, env, ty)?;
    let base_class = class.find_base_specialization(db, base)?;
    let Some((_, Some(specialization))) = base_class.static_class_literal(db) else {
        // A non-generic base has no type arguments.
        return Some(TupleSpec::heterogeneous([]));
    };
    let variables = specialization.generic_context(db).variables(db);
    let mut builder = TupleSpecBuilder::with_capacity(specialization.types(db).len());
    for (typevar, argument) in variables.zip(specialization.types(db)) {
        if typevar.is_typevartuple(db)
            && let Some(spec) = argument.exact_tuple_instance_spec(db)
        {
            builder = builder.concat(db, env, &spec);
        } else {
            // The inert `Annotated` wrapper of a `Member`'s type is only visible to
            // `GetAnnotations` / `DropAnnotations` (P5-22).
            builder.push(strip_annotated(db, *argument));
        }
    }
    Some(builder.build())
}

/// The class of an instance type, including `TypedDict`s and the view of a class that is being
/// updated (`ClassView`, see `update_class`).
fn instance_class<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<ClassType<'db>> {
    match ty {
        Type::TypedDict(typed_dict) => typed_dict.defining_class(),
        Type::TypeOperator(operator) => match operator.kind(db) {
            TypeOperatorKind::ClassView { class, .. } => {
                Some(super::class::update_class::view_class(db, *class))
            }
            _ => None,
        },
        _ => ty.nominal_class(db, env),
    }
}

/// Evaluates `GetArg[ty, base, index]` (`index` is `Some`) or `GetArgs[ty, base]` (`index` is
/// `None`) for non-union, non-dynamic arguments. `base` is normalized by [`normalize_base`].
pub(super) fn get_arg<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    base: Type<'db>,
    index: Option<Type<'db>>,
) -> Result<Type<'db>, EvaluationError> {
    let operator = if index.is_some() { "GetArg" } else { "GetArgs" };
    let index = match index {
        Some(index) => Some(int_literal(index).ok_or_else(|| {
            format!(
                "The index of `{operator}` must be an `int` literal type, but got `{}`",
                index.display(db, env)
            )
        })?),
        None => None,
    };

    let base_class = match base {
        Type::SpecialForm(SpecialFormType::TypingCallable) => {
            return extended_callable::callable_arg(db, env, ty, operator, index);
        }
        Type::SpecialForm(SpecialFormType::Overloaded) => {
            return extended_callable::overloaded_arg(db, env, ty, operator, index);
        }
        Type::ClassLiteral(class) => class,
        _ => {
            return Err(format!(
                "The `Base` argument of `{operator}` must be a class"
            ));
        }
    };
    let base_name = base_class.name(db);

    // Class methods and static methods are usually function-like callables rather than instances
    // of the typeshed classes (P5-10).
    let method_base = base_class
        .known(db)
        .filter(|known| matches!(known, KnownClass::Classmethod | KnownClass::Staticmethod));
    let Some(arguments) = type_arguments_as(db, env, ty, base_class) else {
        if let Some(method_base) = method_base {
            return extended_callable::method_arg(db, env, ty, method_base, operator, index);
        }
        return Err(format!(
            "`{operator}` requires a subclass of `{base_name}`, but got `{}`",
            ty.display(db, env)
        ));
    };
    // The `ParamSpec` argument of `classmethod[C, P, R]` / `staticmethod[P, R]` is a `Params`.
    let arguments = if method_base.is_some() {
        TupleSpec::heterogeneous(
            arguments
                .fixed_elements()
                .map(|argument| extended_callable::paramspec_value_as_params(db, env, *argument))
                .collect::<Vec<_>>(),
        )
    } else {
        arguments
    };

    let Some(index) = index else {
        return Ok(Type::tuple(TupleType::new(db, env, &arguments)));
    };
    i32::try_from(index)
        .ok()
        .and_then(|index| (&arguments).py_index(db, env, index).ok())
        .ok_or_else(|| {
            format!(
                "Index {index} is out of range for `{}` as `{base_name}`",
                ty.display(db, env)
            )
        })
}

/// Evaluates `Length[ty]` for a non-union, non-dynamic argument.
pub(super) fn length<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Result<Type<'db>, EvaluationError> {
    let spec = ty.tuple_instance_spec(db, env).ok_or_else(|| {
        format!(
            "`Length` requires a tuple type, but got `{}`",
            ty.display(db, env)
        )
    })?;
    Ok(match spec.len().into_fixed_length() {
        Some(length) => Type::LiteralValue(LiteralValueType::unpromotable(
            i64::try_from(length).unwrap_or(i64::MAX),
        )),
        None => Type::none(db, env),
    })
}

/// Parses a `Start`/`End` argument of `Slice`: an `int` literal type, a `bool` literal type or
/// `None`.
fn slice_index<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    which: &str,
) -> Result<Option<i32>, EvaluationError> {
    if ty.is_none(db) {
        return Ok(None);
    }
    let invalid = || {
        format!(
            "The {which} index of `Slice` must be an `int` literal type or `None`, but got `{}`",
            ty.display(db, env)
        )
    };
    let index = int_literal(ty).ok_or_else(invalid)?;
    // Indexes beyond the range of `i32` behave like the saturated values for any tuple ty can
    // represent.
    Ok(Some(i32::try_from(index).unwrap_or(if index < 0 {
        i32::MIN
    } else {
        i32::MAX
    })))
}

/// Evaluates `Slice[sequence, start, end]` for non-union, non-dynamic arguments. The sequence is
/// a tuple, or a string (sliced by code point like Python's `str`).
pub(super) fn slice<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    sequence: Type<'db>,
    start: Type<'db>,
    end: Type<'db>,
) -> Result<Type<'db>, EvaluationError> {
    let start = slice_index(db, env, start, "start")?;
    let end = slice_index(db, env, end, "end")?;
    // The step is always `1`, so the slice cannot fail.
    if let Some(spec) = sequence.tuple_instance_spec(db, env) {
        return Ok(spec
            .py_slice_type(db, env, start, end, None)
            .unwrap_or_else(|_| Type::unknown()));
    }
    match StringOperand::classify(db, env, sequence) {
        StringOperand::Literal(value) => {
            let chars: Vec<char> = value.chars().collect();
            let sliced: String = chars
                .py_slice(db, start, end, None)
                .map(Iterator::collect)
                .unwrap_or_default();
            Ok(string_literal_type(db, &sliced))
        }
        StringOperand::LiteralString => Ok(Type::literal_string()),
        StringOperand::Str => Ok(KnownClass::Str.to_instance(db, env)),
        StringOperand::Dynamic(dynamic) => Ok(dynamic),
        StringOperand::Invalid => Err(format!(
            "`Slice` requires a tuple or string type, but got `{}`",
            sequence.display(db, env)
        )),
    }
}

/// The special class attributes that `GetSpecialAttr` supports.
const SPECIAL_ATTRIBUTES: [&str; 3] = ["__name__", "__module__", "__qualname__"];

/// Evaluates `GetSpecialAttr[ty, attribute]` for non-union, non-dynamic arguments.
pub(super) fn get_special_attr<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    attribute: Type<'db>,
) -> Result<Type<'db>, EvaluationError> {
    let Some(attribute_name) = attribute.as_string_literal() else {
        return Err(format!(
            "The second argument to `GetSpecialAttr` must be a string literal type, but got `{}`",
            attribute.display(db, env)
        ));
    };
    let attribute_name = attribute_name.value(db);
    if !SPECIAL_ATTRIBUTES.contains(&attribute_name) {
        return Err(
            "`GetSpecialAttr` supports only `__name__`, `__module__` and `__qualname__`"
                .to_string(),
        );
    }

    // If `x: ty`, the attribute is read from `type(x)`: the class of an instance, or the
    // metaclass of a class object.
    let class = match ty {
        Type::SubclassOf(subclass_of) => subclass_of
            .to_metaclass_instance(db, env)
            .nominal_class(db, env),
        _ => instance_class(db, env, ty),
    }
    .ok_or_else(|| {
        format!(
            "`GetSpecialAttr` requires a class, but got `{}`",
            ty.display(db, env)
        )
    })?;
    let class = class.class_literal(db);

    let value = match attribute_name {
        "__name__" => class.name(db).to_string(),
        "__qualname__" => class.qualified_name(db).python_qualname(),
        _ => file_to_module(db, class.program_file(db).resolver_file(db))
            .map(|module| module.name(db).as_str().to_string())
            .ok_or_else(|| format!("The module of `{}` is unknown", ty.display(db, env)))?,
    };
    Ok(string_literal_type(db, &value))
}
