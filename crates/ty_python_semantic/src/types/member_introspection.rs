//! Member introspection for [PEP 827] type operators (`Attrs`, `Members`, `GetMember`,
//! `GetMemberType`) and the `ty_extensions.Member` descriptor class.
//!
//! The type argument `T` of a collected attribute whose annotation is written as
//! `Annotated[X, *metadata]` is the inert [`TypeOperatorKind::Annotated`] operator, so that
//! `GetAnnotations` can read the metadata; every other reader strips it (see
//! [`strip_annotated`](super::type_operator::strip_annotated)).
//!
//! A class's members are collected in an ordered, qualifier-preserving walk over its MRO,
//! starting from the most basic class. A subclass that redeclares a member keeps the member's
//! original position but supplies its type, initializer and definer.
//!
//! The own members of a class that is updated with PEP 827 `UpdateClass` include its updates
//! (see [`update_class`]); `Attrs` / `Members` of the internal `ClassView` of a class only
//! include the updates of the class that come before the one being evaluated.
//!
//! [PEP 827]: https://peps.python.org/pep-0827/

use ruff_db::parsed::parsed_module;
use ruff_python_ast::name::Name;
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_core::definition::{Definition, DefinitionKind, DefinitionState};
use ty_python_core::{place_table, use_def_map};

use super::class::{CodeGeneratorKind, update_class};
use super::member::class_member;
use crate::place::place_from_bindings;
use crate::reachability::DeclarationsIteratorExtension;
use crate::types::special_form::SpecialFormType;
use crate::types::type_operator::{TypeOperatorKind, strip_annotated};
use crate::types::{
    ClassBase, ClassType, KnownClass, LiteralValueType, StaticClassLiteral, StringLiteralType,
    Type, TypeOperatorType, TypedDictType, UnionType, definition_expression_type,
};
use crate::{Db, FxIndexMap, ProgramEnvironment, TypeQualifiers};

/// The qualifiers that can be expressed in the `Q` parameter of a `Member`, in the order in which
/// they are displayed (`MemberQuals`).
const MEMBER_QUALIFIERS: [(TypeQualifiers, &str); 4] = [
    (TypeQualifiers::CLASS_VAR, "ClassVar"),
    (TypeQualifiers::FINAL, "Final"),
    (TypeQualifiers::NOT_REQUIRED, "NotRequired"),
    (TypeQualifiers::READ_ONLY, "ReadOnly"),
];

/// Names that are never reported as members (`typing.EXCLUDED_ATTRIBUTES`, except `__init__`).
const EXCLUDED_MEMBER_NAMES: &[&str] = &[
    "__parameters__",
    "__orig_bases__",
    "__orig_class__",
    "_is_protocol",
    "_is_runtime_protocol",
    "__protocol_attrs__",
    "__non_callable_proto_members__",
    "__type_params__",
    "__abstractmethods__",
    "__annotations__",
    "__dict__",
    "__doc__",
    "__module__",
    "__new__",
    "__slots__",
    "__subclasshook__",
    "__weakref__",
    "__class_getitem__",
    "__match_args__",
    "__static_attributes__",
    "__firstlineno__",
    "__annotate__",
    "__annotate_func__",
    "__annotations_cache__",
    "_MutableMapping__marker",
];

/// Returns an (unpromotable) string literal type, as written in a type expression.
pub(crate) fn string_literal_type<'db>(db: &'db dyn Db, value: &str) -> Type<'db> {
    Type::LiteralValue(LiteralValueType::unpromotable(StringLiteralType::new(
        db, value,
    )))
}

/// Returns `Member[name, ty, quals, init, definer]`.
pub(crate) fn member_instance<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    args: [Type<'db>; 5],
) -> Type<'db> {
    KnownClass::TyExtensionsMember.to_specialized_instance(db, env, &args[..])
}

/// Returns the specialization `[name, ty, quals, init, definer]` of a `Member` instance type, or
/// `None` if `ty` is not a `Member` instance.
pub(crate) fn member_args<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
) -> Option<[Type<'db>; 5]> {
    let Type::NominalInstance(instance) = ty.resolve_type_alias(db) else {
        return None;
    };
    let ClassType::Generic(alias) = instance.class(db, env) else {
        return None;
    };
    if alias.origin(db).known(db) != Some(KnownClass::TyExtensionsMember) {
        return None;
    }
    <[Type<'db>; 5]>::try_from(alias.specialization(db).types(db)).ok()
}

/// Returns the `MemberQuals` literal type for the given qualifiers (`Never` if there are none).
pub(crate) fn qualifiers_type<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    qualifiers: TypeQualifiers,
) -> Type<'db> {
    UnionType::from_elements(
        db,
        env,
        MEMBER_QUALIFIERS
            .iter()
            .filter(|(qualifier, _)| qualifiers.contains(*qualifier))
            .map(|(_, name)| string_literal_type(db, name)),
    )
}

/// The full `MemberQuals` type: `Literal["ClassVar", "Final", "NotRequired", "ReadOnly"]`.
pub(crate) fn all_qualifiers_type<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
) -> Type<'db> {
    qualifiers_type(
        db,
        env,
        MEMBER_QUALIFIERS
            .iter()
            .fold(TypeQualifiers::empty(), |acc, (qualifier, _)| {
                acc | *qualifier
            }),
    )
}

/// Parses the `Q` argument of a `Member` (`Never` or a union of `MemberQuals` literals). Returns
/// `None` if the type is not of that form.
pub(crate) fn parse_qualifiers<'db>(db: &'db dyn Db, ty: Type<'db>) -> Option<TypeQualifiers> {
    let parse_one = |ty: Type<'db>| {
        let literal = ty.as_string_literal()?;
        MEMBER_QUALIFIERS
            .iter()
            .find(|(_, name)| *name == literal.value(db))
            .map(|(qualifier, _)| *qualifier)
    };
    match ty.resolve_type_alias(db) {
        Type::Never => Some(TypeQualifiers::empty()),
        Type::Union(union) => union
            .elements(db)
            .iter()
            .try_fold(TypeQualifiers::empty(), |acc, element| {
                Some(acc | parse_one(*element)?)
            }),
        ty => parse_one(ty),
    }
}

/// Returns `true` if `ty` is a callable type that describes a method: a `Callable`, a function
/// (including static methods and class methods), or a bound method.
pub(crate) fn is_method_like_type(ty: Type<'_>) -> bool {
    matches!(
        ty,
        Type::Callable(_) | Type::FunctionLiteral(_) | Type::BoundMethod(_)
    )
}

/// Returns `Attrs[ty]` (`attrs_only`) or `Members[ty]` for a class instance type `ty`, as a tuple
/// of `Member` instances. Returns `None` if `ty` does not describe instances of a class.
pub(crate) fn members_of<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    ty: Type<'db>,
    attrs_only: bool,
) -> Option<Type<'db>> {
    match ty.resolve_type_alias(db) {
        Type::NominalInstance(instance) => {
            Some(class_members(db, instance.class(db, env), attrs_only, None))
        }
        // The class as it was before one of its updates (see `update_class`).
        Type::TypeOperator(operator) => match operator.kind(db) {
            TypeOperatorKind::ClassView { class, step } => Some(class_members(
                db,
                update_class::view_class(db, *class),
                attrs_only,
                Some(*step),
            )),
            _ => None,
        },
        Type::ProtocolInstance(protocol) => protocol
            .class_origin(db)
            .map(|class| class_members(db, *class, attrs_only, None)),
        Type::TypedDict(TypedDictType::Class(class)) => {
            Some(class_members(db, class, attrs_only, None))
        }
        Type::TypedDict(typed_dict @ TypedDictType::Synthesized(_)) => {
            Some(Type::heterogeneous_tuple(
                db,
                env,
                typed_dict.items(db).iter().map(|(name, field)| {
                    typed_dict_member(
                        db,
                        env,
                        name,
                        field.declared_ty,
                        field.is_required(),
                        field.is_read_only(),
                    )
                }),
            ))
        }
        _ => None,
    }
}

fn typed_dict_member<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    name: &Name,
    ty: Type<'db>,
    is_required: bool,
    is_read_only: bool,
) -> Type<'db> {
    let mut qualifiers = TypeQualifiers::empty();
    qualifiers.set(TypeQualifiers::NOT_REQUIRED, !is_required);
    qualifiers.set(TypeQualifiers::READ_ONLY, is_read_only);
    member_instance(
        db,
        env,
        [
            string_literal_type(db, name),
            ty,
            qualifiers_type(db, env, qualifiers),
            Type::Never,
            Type::Never,
        ],
    )
}

/// Returns the `Member` with the given name in `Members[ty]`, or `None` if there is no such
/// member (or `ty` does not describe instances of a class).
pub(crate) fn find_member<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    members: Type<'db>,
    name: &str,
) -> Option<Type<'db>> {
    let spec = members.exact_tuple_instance_spec(db)?;
    spec.fixed_elements().copied().find(|member| {
        member_args(db, env, *member).is_some_and(|[member_name, ..]| {
            member_name
                .as_string_literal()
                .is_some_and(|literal| literal.value(db) == name)
        })
    })
}

/// Cached member collection for a class, keyed by the class, whether to only include
/// attributes, and how many updates of the class itself (`UpdateClass`) to apply (`None` for
/// all of them).
#[salsa::tracked(
    returns(copy),
    cycle_initial=|_, _, _, _, _| Type::Never,
    heap_size=ruff_memory_usage::heap_size,
)]
fn class_members<'db>(
    db: &'db dyn Db,
    class: ClassType<'db>,
    attrs_only: bool,
    steps: Option<u32>,
) -> Type<'db> {
    let env = ProgramEnvironment::from_file(class.class_literal(db).program_file(db));

    if class.is_typed_dict(db) {
        return typed_dict_class_members(db, &env, class);
    }

    let mut attributes: FxIndexMap<Name, CollectedMember<'db>> = FxIndexMap::default();
    let mut methods: FxIndexMap<Name, CollectedMember<'db>> = FxIndexMap::default();

    let mro: Vec<ClassBase<'db>> = class.iter_mro(db).collect();
    let level_count = mro.len();
    for (level, base) in mro.into_iter().rev().enumerate() {
        let Some(base_class) = base.into_class() else {
            continue;
        };
        let Some((class_literal, specialization)) = base_class.static_class_literal(db) else {
            continue;
        };
        if contributes_no_members(db, class_literal) {
            continue;
        }
        let definer = Type::instance(db, &env, base_class);
        // Only the class itself is viewed before some of its updates.
        let steps = if level + 1 == level_count {
            steps
        } else {
            None
        };
        let (own_members, removed) = updated_own_members(db, class_literal, steps);
        for name in &removed {
            attributes.shift_remove(name);
            methods.shift_remove(name);
        }
        for own in own_members {
            let member = CollectedMember {
                // A method's retained synthetic `Self` is bounded by the owner, so the owner's
                // specialization also applies to that bound (P5-7).
                ty: if own.is_method {
                    own.ty
                        .apply_optional_owner_specialization_to_member(db, specialization)
                } else {
                    own.ty.apply_optional_specialization(db, specialization)
                },
                qualifiers: own.qualifiers,
                init: own.init.apply_optional_specialization(db, specialization),
                definer,
                level,
            };
            if own.is_method {
                methods.insert(own.name, member);
            } else {
                attributes.insert(own.name, member);
            }
        }
    }

    // Attributes come first, then methods. When a method and an attribute share a name, the most
    // derived definition wins and keeps the position of the first one.
    let mut members = attributes;
    for (name, method) in methods {
        match members.get_mut(&name) {
            Some(existing) if existing.level > method.level => {}
            Some(existing) => *existing = method,
            None => {
                members.insert(name, method);
            }
        }
    }

    Type::heterogeneous_tuple(
        db,
        &env,
        members
            .into_iter()
            .filter(|(_, member)| {
                // `Attrs` excludes methods: `ClassVar`s with a callable type.
                let is_method = member.qualifiers.contains(TypeQualifiers::CLASS_VAR)
                    && is_method_like_type(strip_annotated(db, member.ty));
                !(attrs_only && is_method)
            })
            .map(|(name, member)| {
                member_instance(
                    db,
                    &env,
                    [
                        string_literal_type(db, &name),
                        member.ty,
                        qualifiers_type(db, &env, member.qualifiers),
                        member.init,
                        member.definer,
                    ],
                )
            }),
    )
}

fn typed_dict_class_members<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    class: ClassType<'db>,
) -> Type<'db> {
    if let Some((class_literal, specialization)) = class.static_class_literal(db) {
        let fields = class_literal.fields(db, specialization, CodeGeneratorKind::TypedDict);
        Type::heterogeneous_tuple(
            db,
            env,
            fields.iter().map(|(name, field)| {
                typed_dict_member(
                    db,
                    env,
                    name,
                    field.declared_ty,
                    field.is_required(),
                    field.is_read_only(),
                )
            }),
        )
    } else {
        Type::heterogeneous_tuple(
            db,
            env,
            TypedDictType::Class(class)
                .items(db)
                .iter()
                .map(|(name, field)| {
                    typed_dict_member(
                        db,
                        env,
                        name,
                        field.declared_ty,
                        field.is_required(),
                        field.is_read_only(),
                    )
                }),
        )
    }
}

struct CollectedMember<'db> {
    ty: Type<'db>,
    qualifiers: TypeQualifiers,
    init: Type<'db>,
    definer: Type<'db>,
    /// Position of the defining class in the reversed MRO; higher is more derived.
    level: usize,
}

struct OwnMember<'db> {
    name: Name,
    ty: Type<'db>,
    qualifiers: TypeQualifiers,
    init: Type<'db>,
    is_method: bool,
}

/// The members of `class` itself after its first `steps` updates (all of them if `None`), and
/// the names that its updates remove (also from the members inherited from its bases).
///
/// The members of each update come first, in the order of the update, followed by the members
/// that it does not replace (P5-27).
fn updated_own_members<'db>(
    db: &'db dyn Db,
    class: StaticClassLiteral<'db>,
    steps: Option<u32>,
) -> (Vec<OwnMember<'db>>, Vec<Name>) {
    let mut members = own_members(db, class);
    let mut removed: Vec<Name> = Vec::new();
    if !update_class::has_class_updates(db, class) {
        return (members, removed);
    }
    let count = update_class::class_update_count(db, class);
    for step in 0..steps.map_or(count, |steps| steps.min(count)) {
        let update = &update_class::class_update_step(db, class, step).members;
        if update.is_empty() {
            continue;
        }
        let mut updated = Vec::with_capacity(update.len() + members.len());
        for member in update {
            removed.retain(|name| *name != member.name);
            if member.is_removal() {
                removed.push(member.name.clone());
                continue;
            }
            updated.push(OwnMember {
                name: member.name.clone(),
                ty: member.ty,
                qualifiers: member.qualifiers,
                init: member.init,
                is_method: member.is_method(),
            });
        }
        updated.extend(
            members
                .into_iter()
                .filter(|own| !update.iter().any(|member| member.name == own.name)),
        );
        members = updated;
    }
    (members, removed)
}

/// Classes from `builtins` (including `object`) and type-checker internal fallback classes
/// contribute no members.
fn contributes_no_members<'db>(db: &'db dyn Db, class: StaticClassLiteral<'db>) -> bool {
    if matches!(
        class.known(db),
        Some(
            KnownClass::Object
                | KnownClass::TypedDictFallback
                | KnownClass::ExtensionTypedDictFallback
                | KnownClass::NamedTupleFallback
        )
    ) {
        return true;
    }
    file_to_module(db, class.program_file(db).resolver_file(db))
        .and_then(|module| module.known(db))
        .is_some_and(|module| {
            matches!(
                module,
                KnownModule::Builtins | KnownModule::TypeCheckerInternals
            )
        })
}

/// Returns `true` if any overload of `function` has a parameter or return annotation.
fn has_any_annotation(db: &dyn Db, function: super::function::FunctionType<'_>) -> bool {
    function.signature(db).iter().any(|signature| {
        !signature.return_ty.is_unknown()
            || signature
                .parameters()
                .iter()
                .any(|parameter| !parameter.inferred_annotation)
    })
}

/// The members declared directly in the body of `class`, in source order (unspecialized).
///
/// Annotated assignments are attributes; properties are attributes whose type is the getter's
/// return type; functions with at least one annotation are methods.
fn own_members<'db>(db: &'db dyn Db, class: StaticClassLiteral<'db>) -> Vec<OwnMember<'db>> {
    let scope = class.body_scope(db);
    let env = ProgramEnvironment::from_scope(scope);
    let table = place_table(db, scope);
    let use_def = use_def_map(db, scope);
    let is_stub = class.file(db).is_stub(db);

    let mut members = Vec::new();
    for (symbol_id, _) in use_def.all_end_of_scope_symbol_declarations() {
        let Some(order) = use_def
            .reachable_symbol_declarations(symbol_id)
            .first_reachable_declaration_order(db, |declaration| {
                declaration.is_defined_and(|definition| {
                    matches!(
                        definition.kind(db),
                        DefinitionKind::AnnotatedAssignment(_) | DefinitionKind::Function(_)
                    )
                })
            })
        else {
            continue;
        };
        let name = table.symbol(symbol_id).name();
        if EXCLUDED_MEMBER_NAMES.contains(&name.as_str()) {
            continue;
        }

        let member = class_member(db, scope, name).inner;
        let Some(ty) = member.place.ignore_possibly_undefined() else {
            continue;
        };

        let own = match ty {
            Type::PropertyInstance(property) => {
                let ty = match property.getter(db) {
                    Some(Type::FunctionLiteral(getter)) => {
                        getter.last_definition_signature(db).return_ty
                    }
                    _ => Type::unknown(),
                };
                OwnMember {
                    name: name.clone(),
                    ty,
                    qualifiers: member.qualifiers & TypeQualifiers::FINAL,
                    init: Type::Never,
                    is_method: false,
                }
            }
            Type::FunctionLiteral(function) => {
                if !has_any_annotation(db, function) {
                    continue;
                }
                OwnMember {
                    name: name.clone(),
                    ty: Type::Callable(function.into_callable_type(db)),
                    qualifiers: TypeQualifiers::CLASS_VAR,
                    init: Type::Never,
                    is_method: true,
                }
            }
            _ => {
                // Only annotated assignments describe attributes.
                // The first reachable one is the declaration whose annotation is read for
                // `Annotated` metadata.
                let mut annotated_assignment = None;
                if !use_def
                    .reachable_symbol_declarations(symbol_id)
                    .any_reachable(db, |declaration| match declaration {
                        DefinitionState::Defined(definition)
                            if matches!(
                                definition.kind(db),
                                DefinitionKind::AnnotatedAssignment(_)
                            ) =>
                        {
                            annotated_assignment = Some(definition);
                            true
                        }
                        _ => false,
                    })
                {
                    continue;
                }
                let has_initializer = use_def
                    .reachable_symbol_declarations(symbol_id)
                    .any_reachable(db, |declaration| {
                        declaration.is_defined_and(|definition| {
                            matches!(
                                definition.kind(db),
                                DefinitionKind::AnnotatedAssignment(assignment)
                                    if assignment.has_value()
                            )
                        })
                    });
                // Annotation-only declarations in stubs act as bindings but have no initializer.
                let init = if is_stub && !has_initializer {
                    None
                } else {
                    place_from_bindings(db, &env, use_def.end_of_scope_symbol_bindings(symbol_id))
                        .place
                        .ignore_possibly_undefined()
                };
                OwnMember {
                    name: name.clone(),
                    ty: annotated_assignment
                        .map_or(ty, |definition| with_annotated_metadata(db, definition, ty)),
                    qualifiers: member.qualifiers
                        & (TypeQualifiers::CLASS_VAR | TypeQualifiers::FINAL),
                    init: init.unwrap_or(Type::Never),
                    is_method: false,
                }
            }
        };
        members.push((order, own));
    }

    members.sort_by_key(|(order, _)| *order);
    members.into_iter().map(|(_, member)| member).collect()
}

/// Returns `ty` wrapped in the inert [`TypeOperatorKind::Annotated`] operator if the annotation
/// of `definition` is written as `Annotated[X, *metadata]` (P5-22). The metadata types are the
/// inferred types of the metadata expressions (literals keep their literal type). Annotations
/// that only refer to an `Annotated` type (aliases, string annotations) are not recognized.
fn with_annotated_metadata<'db>(
    db: &'db dyn Db,
    definition: Definition<'db>,
    ty: Type<'db>,
) -> Type<'db> {
    let DefinitionKind::AnnotatedAssignment(assignment) = definition.kind(db) else {
        return ty;
    };
    let module = parsed_module(db, definition.python_file(db)).load(db);
    let Some(subscript) = assignment.annotation(&module).as_subscript_expr() else {
        return ty;
    };
    if definition_expression_type(db, definition, &subscript.value)
        != Type::SpecialForm(SpecialFormType::Annotated)
    {
        return ty;
    }
    let Some(metadata) = subscript
        .slice
        .as_tuple_expr()
        .and_then(|tuple| tuple.elts.get(1..))
        .filter(|metadata| !metadata.is_empty())
    else {
        return ty;
    };
    let metadata = metadata
        .iter()
        .map(|expression| definition_expression_type(db, definition, expression))
        .collect();
    Type::TypeOperator(TypeOperatorType::new(
        db,
        TypeOperatorKind::Annotated {
            inner: ty,
            metadata,
        },
    ))
}
