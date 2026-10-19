//! Extended callables for [PEP 827]: the `ty_extensions` classes `Param` and `Params`, the
//! conversion of `Callable[Params[...], R]` into an ordinary callable type, `Overloaded[...]`, and
//! the `Params` view of a signature that `GetArg` / `GetArgs` produce when inspecting callables.
//!
//! ty has a single representation of callable types ([`Signature`]). `Params` is only a way to
//! construct one and a view that is computed when a callable is inspected; it is never stored in a
//! callable type.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_python_ast::name::Name;

use super::callable::CallableTypeKind;
use super::generics::ApplySpecialization;
use super::literal::{EnumLiteralType, LiteralValueTypeKind};
use super::member_introspection::string_literal_type;
use super::signatures::{
    CallableSignature, Parameter, ParameterKind, Parameters, ParametersKind, Signature,
};
use super::type_operator::{
    Sequence, TypeOperatorError, VariadicArg, as_raised_error, evaluate_sequence, needs_deferral,
    strip_annotated,
};
use super::{
    ApplyTypeMappingVisitor, CallableType, ClassType, KnownClass, LiteralValueType, Type,
    TypeContext, TypeMapping,
};
use crate::{Db, ProgramEnvironment};

/// The kinds of `ParamKind`, in the order in which parameters of these kinds must appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ParamKindName {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

impl ParamKindName {
    const ALL: [Self; 5] = [
        Self::PositionalOnly,
        Self::PositionalOrKeyword,
        Self::VarPositional,
        Self::KeywordOnly,
        Self::VarKeyword,
    ];

    /// The name of the corresponding member of `ParamKind`.
    const fn member_name(self) -> &'static str {
        match self {
            Self::PositionalOnly => "POSITIONAL_ONLY",
            Self::PositionalOrKeyword => "POSITIONAL_OR_KEYWORD",
            Self::VarPositional => "VAR_POSITIONAL",
            Self::KeywordOnly => "KEYWORD_ONLY",
            Self::VarKeyword => "VAR_KEYWORD",
        }
    }

    /// How a parameter of this kind is described in error messages.
    const fn description(self) -> &'static str {
        match self {
            Self::PositionalOnly => "a positional-only parameter",
            Self::PositionalOrKeyword => "a positional-or-keyword parameter",
            Self::VarPositional => "a `*args` parameter",
            Self::KeywordOnly => "a keyword-only parameter",
            Self::VarKeyword => "a `**kwargs` parameter",
        }
    }

    const fn is_variadic(self) -> bool {
        matches!(self, Self::VarPositional | Self::VarKeyword)
    }

    const fn is_positional(self) -> bool {
        matches!(self, Self::PositionalOnly | Self::PositionalOrKeyword)
    }
}

/// Returns `Literal[ParamKind.<kind>]`.
fn param_kind_type<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    kind: ParamKindName,
) -> Type<'db> {
    let Some(enum_class) = KnownClass::TyExtensionsParamKind
        .to_class_literal(db, env)
        .as_class_literal()
        .and_then(|class| class.into_enum_class(db))
    else {
        return Type::unknown();
    };
    Type::LiteralValue(LiteralValueType::unpromotable(EnumLiteralType::new(
        db,
        enum_class,
        Name::new_static(kind.member_name()),
    )))
}

/// Parses a `ParamKind` literal type (exactly one member).
fn parse_param_kind<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<ParamKindName> {
    let literal = ty.resolve_type_alias(db).as_enum_literal()?;
    if Type::ClassLiteral(literal.enum_class(db))
        != KnownClass::TyExtensionsParamKind.to_class_literal(db, env)
    {
        return None;
    }
    ParamKindName::ALL
        .into_iter()
        .find(|kind| kind.member_name() == literal.name(db).as_str())
}

/// Returns the specialization `[name, type, kind, default]` of a `Param` instance type, or `None`
/// if `ty` is not a `Param` instance.
pub(crate) fn param_args<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<[Type<'db>; 4]> {
    let Type::NominalInstance(instance) = ty.resolve_type_alias(db) else {
        return None;
    };
    let ClassType::Generic(alias) = instance.class(db, env) else {
        return None;
    };
    if alias.origin(db).known(db) != Some(KnownClass::TyExtensionsParam) {
        return None;
    }
    <[Type<'db>; 4]>::try_from(alias.specialization(db).types(db)).ok()
}

/// Returns `Param[name, ty, kind, default]`.
fn param_instance<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    args: [Type<'db>; 4],
) -> Type<'db> {
    KnownClass::TyExtensionsParam.to_specialized_instance(db, env, &args[..])
}

/// Returns `Params[*elements]`.
fn params_instance<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    elements: Vec<Type<'db>>,
) -> Type<'db> {
    let elements = Type::heterogeneous_tuple(db, env, elements);
    KnownClass::TyExtensionsParams.to_specialized_instance(db, env, &[elements][..])
}

/// Returns the unpacked tuple of `Param`s (the type argument of its `TypeVarTuple` parameter) of a
/// `Params` instance type, or `None` if `ty` is not a `Params` instance.
pub(crate) fn params_elements<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<Type<'db>> {
    let Type::NominalInstance(instance) = ty.resolve_type_alias(db) else {
        return None;
    };
    if !instance.has_known_class(db, KnownClass::TyExtensionsParams) {
        return None;
    }
    match instance.class(db, env) {
        ClassType::Generic(alias) => alias.specialization(db).types(db).first().copied(),
        ClassType::NonGeneric(_) => None,
    }
}

/// Returns `true` if `ty` is a `Param` instance type (used to report `Param`s that are not
/// wrapped in `Params[...]`).
pub(crate) fn is_param_instance<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> bool {
    param_args(db, env, ty).is_some()
}

/// The result of converting the arguments of `Params[...]` into a parameter list.
pub(crate) enum ParamsConversion<'db> {
    Parameters(Parameters<'db>),
    /// The arguments depend on unsolved type variables.
    Deferred,
    /// An argument is dynamic (e.g. `Unknown` from an invalid `Param` that has already been
    /// reported, or a raised error); the callable is that type.
    Dynamic(Type<'db>),
}

/// A `Param` whose components have been checked.
struct ParsedParam<'db> {
    name: Option<Name>,
    ty: Type<'db>,
    kind: ParamKindName,
    default: Option<Type<'db>>,
}

/// Converts the (evaluated) arguments of `Params[...]` into a parameter list (P5-1 to P5-4,
/// P5-32).
pub(crate) fn parameters_from_params<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    args: &[VariadicArg<'db>],
) -> Result<ParamsConversion<'db>, TypeOperatorError> {
    let mut elements = Vec::with_capacity(args.len());
    let mut gradual = false;
    for arg in args {
        match *arg {
            VariadicArg::Single(ty) => elements.push(ty),
            VariadicArg::Unpacked(ty) => match evaluate_sequence(db, env, ty) {
                Sequence::Fixed(fixed) => elements.extend(fixed),
                Sequence::Unbounded { .. } => {
                    return Err(TypeOperatorError::new(format!(
                        "The number of parameters in `Params` must be known, but `*{}` has an \
                         unbounded length",
                        ty.display(db, env)
                    )));
                }
                Sequence::Dynamic(dynamic) => {
                    if as_raised_error(db, dynamic).is_some() {
                        return Ok(ParamsConversion::Dynamic(dynamic));
                    }
                    gradual = true;
                }
                Sequence::Deferred => return Ok(ParamsConversion::Deferred),
                Sequence::Error => {
                    return Err(TypeOperatorError::new(format!(
                        "Cannot unpack `{}` in `Params`",
                        ty.display(db, env)
                    )));
                }
            },
        }
    }

    let mut deferred = false;
    let mut parsed = Vec::with_capacity(elements.len());
    for element in elements {
        let resolved = element.resolve_type_alias(db);
        if resolved.is_dynamic() {
            return Ok(ParamsConversion::Dynamic(resolved));
        }
        let Some([name, ty, kind, default]) = param_args(db, env, resolved) else {
            if needs_deferral(db, env, resolved) {
                deferred = true;
                continue;
            }
            return Err(TypeOperatorError::new(format!(
                "The arguments to `Params` must be `Param` types, but got `{}`",
                element.display(db, env)
            )));
        };
        // A component that violates the bounds of `Param` has already been reported.
        if [name, ty, kind, default].iter().any(Type::is_unknown) {
            return Ok(ParamsConversion::Dynamic(Type::unknown()));
        }
        if needs_deferral(db, env, name) || needs_deferral(db, env, kind) {
            deferred = true;
            continue;
        }
        let name = match name.resolve_type_alias(db) {
            Type::LiteralValue(literal) if literal.as_string().is_some() => literal
                .as_string()
                .map(|literal| Name::new(literal.value(db))),
            none if none.is_none(db) => None,
            _ => {
                return Err(TypeOperatorError::new(format!(
                    "The name of a `Param` must be a string literal type or `None`, but got `{}`",
                    name.display(db, env)
                )));
            }
        };
        let Some(kind_name) = parse_param_kind(db, env, kind) else {
            return Err(TypeOperatorError::new(format!(
                "The kind of a `Param` must be exactly one `ParamKind` literal type, but got `{}`",
                kind.display(db, env)
            )));
        };
        // P5-4: an unnamed positional-or-keyword parameter can only be passed positionally.
        let kind_name = if kind_name == ParamKindName::PositionalOrKeyword && name.is_none() {
            ParamKindName::PositionalOnly
        } else {
            kind_name
        };
        let default = match default.resolve_type_alias(db) {
            Type::Never => None,
            _ => Some(default),
        };
        parsed.push(ParsedParam {
            name,
            ty: strip_annotated(db, ty),
            kind: kind_name,
            default,
        });
    }
    if deferred {
        return Ok(ParamsConversion::Deferred);
    }
    if gradual {
        return Ok(ParamsConversion::Parameters(Parameters::gradual_form()));
    }

    let mut parameters = Vec::with_capacity(parsed.len());
    let mut previous_kind: Option<ParamKindName> = None;
    let mut names: Vec<Name> = Vec::with_capacity(parsed.len());
    let mut positional_default = false;
    for param in parsed {
        if let Some(previous) = previous_kind
            && (param.kind < previous || (param.kind == previous && param.kind.is_variadic()))
        {
            return Err(TypeOperatorError::new(format!(
                "Invalid parameter order in `Params`: {} cannot follow {}",
                param.kind.description(),
                previous.description()
            )));
        }
        previous_kind = Some(param.kind);
        if param.kind.is_variadic() && param.default.is_some() {
            return Err(TypeOperatorError::new(format!(
                "{} cannot have a default",
                capitalize(param.kind.description())
            )));
        }
        if param.kind == ParamKindName::KeywordOnly && param.name.is_none() {
            return Err(TypeOperatorError::new(
                "A keyword-only `Param` must have a name",
            ));
        }
        if let Some(name) = &param.name {
            if names.contains(name) {
                return Err(TypeOperatorError::new(format!(
                    "Duplicate parameter name `{name}` in `Params`"
                )));
            }
            names.push(name.clone());
        }
        if param.kind.is_positional() {
            if param.default.is_some() {
                positional_default = true;
            } else if positional_default {
                return Err(TypeOperatorError::new(
                    "A positional parameter without a default cannot follow one with a default \
                     in `Params`",
                ));
            }
        }

        let parameter = match (param.kind, param.name) {
            (ParamKindName::PositionalOnly, name)
            | (ParamKindName::PositionalOrKeyword, name @ None) => Parameter::positional_only(name),
            (ParamKindName::PositionalOrKeyword, Some(name)) => {
                Parameter::positional_or_keyword(name)
            }
            (ParamKindName::VarPositional, name) => {
                Parameter::variadic(name.unwrap_or_else(|| Name::new_static("args")))
            }
            (ParamKindName::KeywordOnly, name) => {
                Parameter::keyword_only(name.unwrap_or_else(|| Name::new_static("_")))
            }
            (ParamKindName::VarKeyword, name) => {
                Parameter::keyword_variadic(name.unwrap_or_else(|| Name::new_static("kwargs")))
            }
        };
        parameters.push(
            parameter
                .with_annotated_type(param.ty)
                .with_optional_default_type(param.default),
        );
    }
    Ok(ParamsConversion::Parameters(Parameters::from_annotation(
        db, parameters,
    )))
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Evaluates `Callable[Params[*params], return_ty]`. Returns `None` if the parameter list depends
/// on unsolved type variables.
pub(crate) fn evaluate_extended_callable<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    params: &[VariadicArg<'db>],
    return_ty: Type<'db>,
) -> Result<Option<Type<'db>>, TypeOperatorError> {
    Ok(match parameters_from_params(db, env, params)? {
        ParamsConversion::Parameters(parameters) => Some(Type::single_callable(
            db,
            Signature::new(parameters, return_ty),
        )),
        ParamsConversion::Deferred => None,
        ParamsConversion::Dynamic(dynamic) => Some(dynamic),
    })
}

/// Evaluates `Overloaded[*args]` (P5-12, P5-32). Returns `None` if an argument depends on
/// unsolved type variables.
pub(crate) fn evaluate_overloaded<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    args: &[VariadicArg<'db>],
) -> Result<Option<Type<'db>>, TypeOperatorError> {
    let mut elements = Vec::with_capacity(args.len());
    for arg in args {
        match *arg {
            VariadicArg::Single(ty) => elements.push(ty),
            VariadicArg::Unpacked(ty) => match evaluate_sequence(db, env, ty) {
                Sequence::Fixed(fixed) => elements.extend(fixed),
                Sequence::Unbounded { .. } => {
                    return Err(TypeOperatorError::new(format!(
                        "The number of arguments to `Overloaded` must be known, but `*{}` has an \
                         unbounded length",
                        ty.display(db, env)
                    )));
                }
                Sequence::Dynamic(dynamic) => return Ok(Some(dynamic)),
                Sequence::Deferred => return Ok(None),
                Sequence::Error => {
                    return Err(TypeOperatorError::new(format!(
                        "Cannot unpack `{}` in `Overloaded`",
                        ty.display(db, env)
                    )));
                }
            },
        }
    }
    if elements.is_empty() {
        return Err(TypeOperatorError::new(
            "`Overloaded` requires at least one callable type",
        ));
    }

    let mut callables = Vec::with_capacity(elements.len());
    for element in &elements {
        let resolved = element.resolve_type_alias(db);
        if resolved.is_dynamic() {
            return Ok(Some(resolved));
        }
        match resolved
            .try_upcast_to_callable(db, env)
            .and_then(|callables| callables.exactly_one())
        {
            Some(callable) => callables.push(callable),
            None if needs_deferral(db, env, resolved) => return Ok(None),
            None => {
                return Err(TypeOperatorError::new(format!(
                    "The arguments to `Overloaded` must be callable types, but got `{}`",
                    element.display(db, env)
                )));
            }
        }
    }
    if let [single] = &elements[..] {
        return Ok(Some(single.resolve_type_alias(db)));
    }

    let kind = if callables
        .iter()
        .all(|callable| callable.kind(db) == CallableTypeKind::FunctionLike)
    {
        CallableTypeKind::FunctionLike
    } else {
        CallableTypeKind::Regular
    };
    let signatures = CallableSignature::from_overloads(
        callables
            .iter()
            .flat_map(|callable| callable.signatures(db).iter().cloned()),
    );
    Ok(Some(Type::Callable(CallableType::new(
        db, signatures, kind,
    ))))
}

/// Returns `signature` with the synthetic `Self` type variables of its generic context replaced
/// by their upper bound, the (specialized) class that defines the method (P5-7). Returns `None` if
/// the signature has any other type parameters of its own (P5-11).
fn signature_without_self<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    signature: &Signature<'db>,
) -> Option<Signature<'db>> {
    let Some(generic_context) = signature.generic_context else {
        return Some(signature.clone());
    };
    let mut result = Signature::new(signature.parameters().clone(), signature.return_ty);
    for typevar in generic_context.variables(db) {
        if !typevar.typevar(db).is_self(db) {
            return None;
        }
        let bound = typevar
            .typevar(db)
            .upper_bound(db, env)
            .unwrap_or_else(Type::object);
        result = result.apply_type_mapping_impl(
            db,
            &TypeMapping::ApplySpecialization(ApplySpecialization::Single(typevar, bound)),
            TypeContext::default(),
            &ApplyTypeMappingVisitor::new(env),
        );
    }
    Some(result)
}

/// The default component `D` of the `Param` for a parameter (P5-6): the default's type if it is a
/// `None`, `int`, `str`, `bytes`, `bool` or enum literal, otherwise the parameter's type; `Never`
/// if the parameter has no default.
fn param_default_type<'db>(
    db: &'db dyn Db,
    parameter: &Parameter<'db>,
    annotated: Type<'db>,
) -> Type<'db> {
    let Some(default) = parameter.default_type(db) else {
        return Type::Never;
    };
    match default {
        Type::LiteralValue(literal)
            if !matches!(literal.kind(), LiteralValueTypeKind::LiteralString) =>
        {
            Type::LiteralValue(literal.to_unpromotable())
        }
        none if none.is_none(db) => none,
        _ => annotated,
    }
}

/// Returns the `Param` describing `parameter`.
fn param_from_parameter<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    parameter: &Parameter<'db>,
) -> Type<'db> {
    let (name, kind) = match parameter.kind() {
        ParameterKind::PositionalOnly { name, .. } => {
            (name.as_ref(), ParamKindName::PositionalOnly)
        }
        ParameterKind::PositionalOrKeyword { name, .. } => {
            (Some(name), ParamKindName::PositionalOrKeyword)
        }
        ParameterKind::Variadic { name } => (Some(name), ParamKindName::VarPositional),
        ParameterKind::KeywordOnly { name, .. } => (Some(name), ParamKindName::KeywordOnly),
        ParameterKind::KeywordVariadic { name } => (Some(name), ParamKindName::VarKeyword),
    };
    let name = name.map_or_else(|| Type::none(db, env), |name| string_literal_type(db, name));
    let ty = parameter.annotated_type();
    let default = param_default_type(db, parameter, ty);
    param_instance(db, env, [name, ty, param_kind_type(db, env, kind), default])
}

/// Returns the `Param`s describing a parameter list (P5-5).
fn params_of_parameters<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    parameters: &Parameters<'db>,
) -> Result<Vec<Type<'db>>, String> {
    match parameters.kind() {
        ParametersKind::Gradual => Ok([ParamKindName::VarPositional, ParamKindName::VarKeyword]
            .into_iter()
            .map(|kind| {
                param_instance(
                    db,
                    env,
                    [
                        Type::none(db, env),
                        Type::any(),
                        param_kind_type(db, env, kind),
                        Type::Never,
                    ],
                )
            })
            .collect()),
        ParametersKind::Top => {
            Err("The parameters of the top callable type cannot be inspected".to_string())
        }
        _ => Ok(parameters
            .iter()
            .map(|parameter| param_from_parameter(db, env, parameter))
            .collect()),
    }
}

/// Returns the single signature of the callable `ty`, with the synthetic `Self` replaced (P5-7).
/// `what` describes the inspected part (`"the parameters"` or `"the return type"`).
fn single_signature<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    callable: CallableType<'db>,
    operator: &str,
    what: &str,
) -> Result<Signature<'db>, String> {
    let [signature] = &callable.signatures(db).overloads[..] else {
        return Err(format!(
            "`{operator}` cannot inspect a callable with several signatures as a `Callable`; use \
             `Overloaded` as the base"
        ));
    };
    signature_without_self(db, env, signature).ok_or_else(|| {
        format!("Getting {what} of a generic callable with `{operator}` is not supported yet")
    })
}

/// Upcasts `ty` to a single callable type.
fn as_callable<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    operator: &str,
) -> Result<CallableType<'db>, String> {
    ty.try_upcast_to_callable(db, env)
        .and_then(|callables| callables.exactly_one())
        .ok_or_else(|| {
            format!(
                "`{operator}` requires a callable type, but got `{}`",
                ty.display(db, env)
            )
        })
}

/// Selects the element at `index` (Python indexing) of `elements`, or all of them as a tuple if
/// `index` is `None`.
fn select<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    base_name: &str,
    elements: Vec<Type<'db>>,
    index: Option<i64>,
) -> Result<Type<'db>, String> {
    let Some(index) = index else {
        return Ok(Type::heterogeneous_tuple(db, env, elements));
    };
    let len = i64::try_from(elements.len()).unwrap_or(i64::MAX);
    let position = if index < 0 { len + index } else { index };
    usize::try_from(position)
        .ok()
        .and_then(|position| elements.get(position).copied())
        .ok_or_else(|| {
            format!(
                "Index {index} is out of range for `{}` as `{base_name}`",
                ty.display(db, env)
            )
        })
}

/// `GetArg[ty, Callable, index]` / `GetArgs[ty, Callable]` (P5-9): the `Params` of the single
/// signature of `ty` (index `0` or `-2`) and its return type (index `1` or `-1`).
pub(super) fn callable_arg<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    operator: &str,
    index: Option<i64>,
) -> Result<Type<'db>, String> {
    let callable = as_callable(db, env, ty, operator)?;
    let wants_parameters = !matches!(index, Some(1 | -1));
    let what = if wants_parameters {
        "the parameters"
    } else {
        "the return type"
    };
    let signature = single_signature(db, env, callable, operator, what)?;
    let params = if wants_parameters {
        params_instance(
            db,
            env,
            params_of_parameters(db, env, signature.parameters())?,
        )
    } else {
        Type::unknown()
    };
    select(
        db,
        env,
        ty,
        "Callable",
        vec![params, signature.return_ty],
        index,
    )
}

/// `GetArg[ty, classmethod, index]` / `GetArg[ty, staticmethod, index]` and `GetArgs` (P5-10) for
/// a callable `ty` that describes a class method or static method (such as the type of a method
/// member): the receiver class (class methods only), the `Params` without the receiver, and the
/// return type.
pub(super) fn method_arg<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    base: KnownClass,
    operator: &str,
    index: Option<i64>,
) -> Result<Type<'db>, String> {
    let is_classmethod = base == KnownClass::Classmethod;
    let (expected_kind, base_name) = if is_classmethod {
        (CallableTypeKind::ClassMethodLike, "classmethod")
    } else {
        (CallableTypeKind::StaticMethodLike, "staticmethod")
    };
    let callable = ty
        .try_upcast_to_callable(db, env)
        .and_then(|callables| callables.exactly_one())
        .filter(|callable| callable.kind(db) == expected_kind)
        .ok_or_else(|| {
            format!(
                "`{operator}` requires a subclass of `{base_name}`, but got `{}`",
                ty.display(db, env)
            )
        })?;
    let signature = single_signature(db, env, callable, operator, "the parameters")?;
    let mut params = params_of_parameters(db, env, signature.parameters())?;
    let mut elements = Vec::with_capacity(3);
    if is_classmethod {
        if params.is_empty() {
            return Err(format!(
                "The class method `{}` has no `cls` parameter",
                ty.display(db, env)
            ));
        }
        params.remove(0);
        let receiver = signature.parameters()[0].annotated_type();
        elements.push(match receiver {
            Type::SubclassOf(subclass_of) => subclass_of.to_instance(db, env),
            other => other,
        });
    }
    elements.push(params_instance(db, env, params));
    elements.push(signature.return_ty);
    select(db, env, ty, base_name, elements, index)
}

/// Converts the `ParamSpec` argument (a callable describing a parameter list) of a typeshed
/// `classmethod[...]` / `staticmethod[...]` instance into `Params`. Other types are returned
/// unchanged.
pub(super) fn paramspec_value_as_params<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Type<'db> {
    let Type::Callable(callable) = ty else {
        return ty;
    };
    let [signature] = &callable.signatures(db).overloads[..] else {
        return ty;
    };
    params_of_parameters(db, env, signature.parameters())
        .map_or(ty, |params| params_instance(db, env, params))
}

/// `GetArg[ty, Overloaded, index]` / `GetArgs[ty, Overloaded]` (P5-13): the signatures of `ty`,
/// each as a callable with a single signature.
pub(super) fn overloaded_arg<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    operator: &str,
    index: Option<i64>,
) -> Result<Type<'db>, String> {
    let callable = as_callable(db, env, ty, operator)?;
    let elements = callable
        .signatures(db)
        .iter()
        .map(|signature| {
            let signature =
                signature_without_self(db, env, signature).unwrap_or_else(|| signature.clone());
            Type::single_callable(db, signature)
        })
        .collect();
    select(db, env, ty, "Overloaded", elements, index)
}
