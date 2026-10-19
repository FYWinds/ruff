# ruff: file-ignore[docstring-in-stub]
"""Experimental ty APIs intended to be exposed to end users."""

import collections.abc
import sys
from enum import IntEnum
from typing import Any, ClassVar, Generic, Literal, Protocol, TypedDict, _SpecialForm, final

from typing_extensions import (  # ruff: ignore[deprecated-import]
    LiteralString,
    Never,
    Self,
    TypeAlias,
    TypeVar,
    TypeVarTuple,
    Unpack,
)

from ._internal import TypeOf as _TypeOf

# ------------------
# Special operations
# ------------------

def static_assert(condition: object, msg: LiteralString | None = None) -> None: ...

# -------------
# Special forms
# -------------

Not: _SpecialForm
"""
`Not[T]`, also spelled as `~T`, represents the set of all objects that do not inhabit the type `T`.

In type expressions, ty supports the experimental spelling `~T`, and also uses it when displaying
negation types. Note that either spelling will fail in runtime contexts or in cases where an
annotation expression could be evaluated.
"""

Intersection: _SpecialForm
"""
`Intersection[T1, T2, ..., Tn]`, also spelled as `T1 & T2 & ... & Tn`, represents an intersection
type: the set of all objects that inhabit all of the types `T1`, `T2`, ..., `Tn`.

For any two fully static types `T1` and `T2`, `Intersection[T1, T2]` is a subtype of both `T1` and
`T2`. For any type `T3` that is a subtype of both `T1` and `T2`, `Intersection[T1, T2]` is a
supertype of `T3`.

In the following example, although neither `P` nor `Q` is a subtype of the other, an instance of `S`
inhabits `Intersection[P, Q]` because `S` inherits from both `P` and `Q`:

```python
class P: ...
class Q: ...
class S(P, Q): ...

s: Intersection[P, Q] = S()
```

In type expressions, ty supports the experimental spelling `T1 & T2`, and also uses this syntax
when displaying intersection types. Note that either spelling will fail in runtime contexts or in
cases where an annotation expression could be evaluated.
"""

IsAssignable: _SpecialForm
"""
`IsAssignable[T, S]` is an experimental [PEP 827] type boolean: it evaluates to `Literal[True]` if
`T` is assignable to `S`, and to `Literal[False]` otherwise.

The check is performed on `T` as a whole; unions are not distributed over. For example,
`IsAssignable[int | str, int]` is `Literal[False]`. If either operand contains an unsolved type
variable, evaluation is deferred until the type variable is specialized.

Type booleans can be combined with `not`, `and` and `or`, and used as the condition of a
conditional type:

```python
type Pick[T] = int if IsAssignable[T, str] else bytes
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

IsEquivalent: _SpecialForm
"""
`IsEquivalent[T, S]` is an experimental [PEP 827] type boolean: it evaluates to `Literal[True]` if
`T` is assignable to `S` and `S` is assignable to `T`, and to `Literal[False]` otherwise.

Because this is defined in terms of assignability, gradual types are consistent with any type:
`IsEquivalent[Any, int]` is `Literal[True]`.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Bool: _SpecialForm
"""
`Bool[T]` is an experimental [PEP 827] type boolean: it evaluates to `Literal[True]` if `T` is
assignable to `Literal[True]` and is not assignable to `Never`, and to `Literal[False]` otherwise.

This allows a type alias whose value is a type boolean to be used where a type boolean is expected.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Iter: _SpecialForm
"""
`Iter[T]` is an experimental [PEP 827] form that iterates over the elements of the tuple type `T`.
It may only appear as the iterable of a `for` clause of an unpacked list comprehension in a
variadic position of a type expression, or of a generator passed to `any()` or `all()` in a type
boolean:

```python
type Names[T] = tuple[*[m.name for m in Iter[Attrs[T]]]]
type Pairs[A, B] = tuple[*[tuple[a, b] for a in Iter[A] for b in Iter[B]]]
type AllInts[T] = all(IsAssignable[t, int] for t in Iter[T])
```

Iterating over `Never` produces no elements; iterating over `Any` produces an unknown number of
`Any` elements. The unbounded part of a tuple such as `tuple[int, *tuple[str, ...]]` is mapped
once, and stays unbounded in the result.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Attrs: _SpecialForm
"""
`Attrs[T]` is an experimental [PEP 827] operator: a tuple of `Member` types describing the
annotated attributes of the class `T` (excluding methods), from the most basic base class to `T`
itself. Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Members: _SpecialForm
"""
`Members[T]` is an experimental [PEP 827] operator: like `Attrs[T]`, but it also includes the
annotated methods of `T` (after the attributes). Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetMember: _SpecialForm
"""
`GetMember[T, S]` is an experimental [PEP 827] operator: the `Member` type describing the member
named by the string literal type `S` in `Members[T]`. It is an error if there is no such member.
Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetMemberType: _SpecialForm
"""
`GetMemberType[T, S]` is an experimental [PEP 827] operator: the type of the member named by the
string literal type `S` in `Members[T]`, or `Never` if there is no such member. Unions are
distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

FromUnion: _SpecialForm
"""
`FromUnion[T]` is an experimental [PEP 827] operator: a tuple of the elements of the union `T`
(a one-element tuple if `T` is not a union, and an empty tuple if `T` is `Never`).

[PEP 827]: https://peps.python.org/pep-0827/
"""

NewProtocol: _SpecialForm
"""
`NewProtocol[*Ms]` is an experimental [PEP 827] operator: a new structural protocol type with the
members described by the `Member` types `Ms`. Members qualified with `ClassVar` whose type is
callable become methods; other members become attributes.

```python
type Partial[T] = NewProtocol[*[Member[m.name, m.type | None] for m in Iter[Attrs[T]]]]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

NewTypedDict: _SpecialForm
"""
`NewTypedDict[*Ms]` is an experimental [PEP 827] operator: a new `TypedDict` type with the items
described by the `Member` types `Ms`. The `NotRequired` and `ReadOnly` qualifiers are respected.

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetArg: _SpecialForm
"""
`GetArg[T, Base, Idx]` is an experimental [PEP 827] operator: the type argument at index `Idx` (an
`int` literal type, negative indexes count from the end) of `T` viewed as an instance of the class
`Base`. The base classes of `T` are searched, so for `class A(B[C])`, `GetArg[A, B, Literal[0]]` is
`C`. `Base` can also be `tuple` (the elements of a tuple), `type` (the class of `type[C]`) or
`Callable` (index `-1` is the return type). Unions are distributed over.

```python
type ElemOf[T] = GetArg[T, Iterable, Literal[0]]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetArgs: _SpecialForm
"""
`GetArgs[T, Base]` is an experimental [PEP 827] operator: a tuple of all type arguments of `T`
viewed as an instance of the class `Base` (see `GetArg`). Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Length: _SpecialForm
"""
`Length[T]` is an experimental [PEP 827] operator: the length of the tuple type `T` as an `int`
literal type, or `None` if `T` has an unbounded part. Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Slice: _SpecialForm
"""
`Slice[S, Start, End]` is an experimental [PEP 827] operator: the slice `S[Start:End]` of the tuple
or string type `S`, where `Start` and `End` are `int` literal types or `None`. Strings are sliced
by code point, like Python's `str`. Unions are distributed over.

```python
type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

Uppercase: _SpecialForm
"""
`Uppercase[S]` is an experimental [PEP 827] operator: the string literal type `S` converted to
uppercase, like Python's `str.upper` (with full Unicode case mappings, e.g. `ß` becomes `SS`).
Unions are distributed over; `str` and `LiteralString` arguments give `str` and `LiteralString`.

String arguments must be written as `Literal["..."]`: a bare string is a forward reference.

```python
type Constant[S] = Uppercase[S]  # Constant[Literal["max_size"]] is Literal["MAX_SIZE"]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

Lowercase: _SpecialForm
"""
`Lowercase[S]` is an experimental [PEP 827] operator: the string literal type `S` converted to
lowercase, like Python's `str.lower`. Unions are distributed over; `str` and `LiteralString`
arguments give `str` and `LiteralString`.

String arguments must be written as `Literal["..."]`: a bare string is a forward reference.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Capitalize: _SpecialForm
"""
`Capitalize[S]` is an experimental [PEP 827] operator: the string literal type `S` capitalized
like Python's `str.capitalize`: the first character is converted to titlecase and **the rest is
converted to lowercase** (`Capitalize[Literal["fooBar"]]` is `Literal["Foobar"]`, unlike
TypeScript's `Capitalize`). Unions are distributed over.

String arguments must be written as `Literal["..."]`: a bare string is a forward reference.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Uncapitalize: _SpecialForm
"""
`Uncapitalize[S]` is an experimental [PEP 827] operator: the string literal type `S` with only its
first character converted to lowercase (`Uncapitalize[Literal["FOO"]]` is `Literal["fOO"]`).
Unions are distributed over.

String arguments must be written as `Literal["..."]`: a bare string is a forward reference.

[PEP 827]: https://peps.python.org/pep-0827/
"""

Concat: _SpecialForm
"""
`Concat[S1, S2]` is an experimental [PEP 827] operator: the concatenation of the string literal
types `S1` and `S2`. Unions are distributed over (every combination is concatenated); `str` and
`LiteralString` arguments give `str` and `LiteralString`.

String arguments must be written as `Literal["..."]`: a bare string is a forward reference.

```python
type Getter[K] = Concat[Literal["get_"], K]  # Getter[Literal["name"]] is Literal["get_name"]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetSpecialAttr: _SpecialForm
"""
`GetSpecialAttr[T, Attr]` is an experimental [PEP 827] operator: the value of the special attribute
`Attr` (`Literal["__name__"]`, `Literal["__module__"]` or `Literal["__qualname__"]`) of the class
of `T`, as a string literal type. Unions are distributed over.

[PEP 827]: https://peps.python.org/pep-0827/
"""

RaiseError: _SpecialForm
"""
`RaiseError[S, *Ts]` is an experimental [PEP 827] operator that reports the type error `S` (a string
literal type), followed by the types `Ts`, when it is evaluated where a type is used (for example
in an annotation, or in the return type of a call). It evaluates to `Any`. It is not reported in a
branch of a conditional type that is not taken.

```python
type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]
```

[PEP 827]: https://peps.python.org/pep-0827/
"""

MemberQuals: TypeAlias = Literal["ClassVar", "Final", "NotRequired", "ReadOnly"]
"""The qualifiers that can appear in the `Q` argument of an experimental [PEP 827] `Member`."""

_MemberN = TypeVar("_MemberN", bound=str, covariant=True)
_MemberT = TypeVar("_MemberT", covariant=True)
_MemberQ = TypeVar("_MemberQ", bound=MemberQuals, default=Never, covariant=True)
_MemberI = TypeVar("_MemberI", default=Never, covariant=True)
_MemberD = TypeVar("_MemberD", default=Never, covariant=True)

@final
class Member(Generic[_MemberN, _MemberT, _MemberQ, _MemberI, _MemberD]):
    """
    `Member[N, T, Q, I, D]` is an experimental [PEP 827] description of a class member: its name
    `N` (a string literal type), its type `T`, its qualifiers `Q` (a union of `MemberQuals`
    literals), the type of its initializer `I`, and the class `D` that defines it.

    In type expressions, the components can be accessed with `m.name`, `m.type`, `m.quals`,
    `m.init` and `m.definer`.

    [PEP 827]: https://peps.python.org/pep-0827/
    """

    @property
    def name(self) -> _MemberN: ...
    @property
    def type(self) -> _MemberT: ...
    @property
    def quals(self) -> _MemberQ: ...
    @property
    def init(self) -> _MemberI: ...
    @property
    def definer(self) -> _MemberD: ...

@final
class ParamKind(IntEnum):
    """
    The kind of a parameter in an experimental [PEP 827] extended callable (`Param[N, T, K, D]`).

    [PEP 827]: https://peps.python.org/pep-0827/
    """

    POSITIONAL_ONLY = 0
    POSITIONAL_OR_KEYWORD = 1
    VAR_POSITIONAL = 2
    KEYWORD_ONLY = 3
    VAR_KEYWORD = 4

_ParamN = TypeVar("_ParamN", bound=str | None, covariant=True)
_ParamT = TypeVar("_ParamT", covariant=True)
_ParamK = TypeVar(
    "_ParamK", bound=ParamKind, default=Literal[ParamKind.POSITIONAL_OR_KEYWORD], covariant=True
)
_ParamD = TypeVar("_ParamD", default=Never, covariant=True)

@final
class Param(Generic[_ParamN, _ParamT, _ParamK, _ParamD]):
    """
    `Param[N, T, K, D]` is an experimental [PEP 827] description of a callable parameter: its name
    `N` (a string literal type, or `None` for a parameter without a name), its type `T`, its kind
    `K` (a `ParamKind` literal type), and the type of its default value `D` (`Never` if it has no
    default).

    In type expressions, the components can be accessed with `p.name`, `p.type`, `p.kind` and
    `p.default`.

    [PEP 827]: https://peps.python.org/pep-0827/
    """

    @property
    def name(self) -> _ParamN: ...
    @property
    def type(self) -> _ParamT: ...
    @property
    def kind(self) -> _ParamK: ...
    @property
    def default(self) -> _ParamD: ...

_ParamsPs = TypeVarTuple("_ParamsPs")

@final
class Params(Generic[Unpack[_ParamsPs]]):
    """
    `Params[*Ps]` is an experimental [PEP 827] parameter list: a sequence of `Param` types. As the
    first argument of `Callable`, it describes the parameters of the callable
    (`Callable[Params[Param[Literal["x"], int]], None]` is `(x: int) -> None`).

    [PEP 827]: https://peps.python.org/pep-0827/
    """

_AliasN = TypeVar("_AliasN", bound=str)
_AliasT = TypeVar("_AliasT")

PosParam: TypeAlias = Param[None, _AliasT, Literal[ParamKind.POSITIONAL_ONLY]]
"""A positional-only parameter without a default ([PEP 827](https://peps.python.org/pep-0827/))."""
PosDefaultParam: TypeAlias = Param[None, _AliasT, Literal[ParamKind.POSITIONAL_ONLY], _AliasT]
"""A positional-only parameter with a default ([PEP 827](https://peps.python.org/pep-0827/))."""
DefaultParam: TypeAlias = Param[_AliasN, _AliasT, Literal[ParamKind.POSITIONAL_OR_KEYWORD], _AliasT]
"""A parameter with a default ([PEP 827](https://peps.python.org/pep-0827/))."""
NamedParam: TypeAlias = Param[_AliasN, _AliasT, Literal[ParamKind.KEYWORD_ONLY]]
"""A keyword-only parameter without a default ([PEP 827](https://peps.python.org/pep-0827/))."""
NamedDefaultParam: TypeAlias = Param[_AliasN, _AliasT, Literal[ParamKind.KEYWORD_ONLY], _AliasT]
"""A keyword-only parameter with a default ([PEP 827](https://peps.python.org/pep-0827/))."""
ArgsParam: TypeAlias = Param[None, _AliasT, Literal[ParamKind.VAR_POSITIONAL]]
"""A `*args` parameter ([PEP 827](https://peps.python.org/pep-0827/))."""
KwargsParam: TypeAlias = Param[None, _AliasT, Literal[ParamKind.VAR_KEYWORD]]
"""A `**kwargs` parameter ([PEP 827](https://peps.python.org/pep-0827/))."""

Overloaded: _SpecialForm
"""
`Overloaded[C1, C2, ...]` is the experimental [PEP 827] type of an overloaded function whose
signatures are the callable types `C1`, `C2`, ... in order. Without arguments, it can be used as
the `Base` argument of `GetArg` and `GetArgs` to inspect the signatures of a callable.

[PEP 827]: https://peps.python.org/pep-0827/
"""

GenericCallable: _SpecialForm
"""
`GenericCallable[tuple[T1, ...], lambda t1, ...: C]` is the experimental [PEP 827] type of a
generic callable with its own type parameters. It is only allowed as the type argument of a
`Member`.

[PEP 827]: https://peps.python.org/pep-0827/
"""

GetAnnotations: _SpecialForm
"""
`GetAnnotations[T]` is an experimental [PEP 827] operator: the union of the types of the metadata
of `Annotated[X, m1, m2, ...]`, or `Never` if `T` is not `Annotated`.

[PEP 827]: https://peps.python.org/pep-0827/
"""

DropAnnotations: _SpecialForm
"""
`DropAnnotations[T]` is an experimental [PEP 827] operator: `T` without its `Annotated` wrapper.

[PEP 827]: https://peps.python.org/pep-0827/
"""

UpdateClass: _SpecialForm
"""
`UpdateClass[*Ms]` is an experimental [PEP 827] return type of class decorators and of
`__init_subclass__`: it adds, replaces or removes the members `Ms` (`Member` types) of the class.

[PEP 827]: https://peps.python.org/pep-0827/
"""

class BaseTypedDict(TypedDict):
    """
    The experimental [PEP 827] base of all `TypedDict` types, for use as the bound of a type
    variable in `**kwargs: Unpack[K]`.

    [PEP 827]: https://peps.python.org/pep-0827/
    """

_KwargDict = TypeVar("_KwargDict", bound=BaseTypedDict)

class InitField(Generic[_KwargDict]):
    """
    An experimental [PEP 827] field descriptor: the keyword arguments of its construction are
    inferred as a `TypedDict` with literal types (`KwargDict`), and become the `init` component of
    the `Member` that describes the field.

    [PEP 827]: https://peps.python.org/pep-0827/
    """

    def __init__(self, **kwargs: Unpack[_KwargDict]) -> None: ...
    def get_kwargs(self) -> _KwargDict: ...

Top: _SpecialForm
"""
`Top[T]` represents the "top materialization" of `T`.

For any type `T`, the top [materialization] of `T` is a type that is
a supertype of all materializations of `T`.

For a [fully static] type `T`, `Top[T]` is always exactly the same type
as `T` itself. For example, the top materialization of `Sequence[int]`
is simply `Sequence[int]`.

For a [gradual type] `T` that contains [`Any`][Any] or `Unknown` inside
it, however, `Top[T]` will not be equivalent to `T`. `Top[Sequence[Any]]`
evaluates to `Sequence[object]`: since `Sequence` is covariant, no
possible materialization of `Any` exists such that a fully static
materialization of `Sequence[Any]` would not be a subtype of
`Sequence[object]`.

`Top[T]` cannot be simplified further for invariant gradual types.
`Top[list[Any]]` cannot be simplified to any other type: because `list`
is invariant, `list[object]` is not a supertype of `list[int]`. The
top materialization of `list[Any]` is simply `Top[list[Any]]`: the
infinite union of `list[T]` for every possible fully static type `T`.

[materialization]: https://typing.python.org/en/latest/spec/concepts.html#materialization
[fully static]: https://typing.python.org/en/latest/spec/concepts.html#fully-static-types
[gradual type]: https://typing.python.org/en/latest/spec/concepts.html#gradual-types
[Any]: https://typing.python.org/en/latest/spec/special-types.html#any
"""

Bottom: _SpecialForm
"""
`Bottom[T]` represents the "bottom materialization" of `T`.

For any type `T`, the bottom [materialization] of `T` is a type that is
a subtype of all materializations of `T`.

For a [fully static] type `T`, `Bottom[T]` is always exactly the same type
as `T` itself. For example, the bottom materialization of `Sequence[int]`
is simply `Sequence[int]`.

For a [gradual type] `T` that contains [`Any`][Any] or `Unknown` inside it,
however, `Bottom[T]` will not be equivalent to `T`. `Bottom[Sequence[Any]]`
evaluates to `Sequence[Never]`: since `Sequence` is covariant, no
possible materialization of `Any` exists such that a fully static
materialization of `Sequence[Any]` would not be a supertype of
`Sequence[Never]`. (`Sequence[Never]` is not the same type as the
uninhabited type `Never`: for example, it is inhabited by the empty tuple,
`()`.)

For many invariant gradual types `T`, `Bottom[T]` is equivalent to
[`Never`][Never], although ty will not necessarily apply this simplification
eagerly.

[materialization]: https://typing.python.org/en/latest/spec/concepts.html#materialization
[fully static]: https://typing.python.org/en/latest/spec/concepts.html#fully-static-types
[gradual type]: https://typing.python.org/en/latest/spec/concepts.html#gradual-types
[Never]: https://typing.python.org/en/latest/spec/special-types.html#never
[Any]: https://typing.python.org/en/latest/spec/special-types.html#any
"""

# -----
# Types
# -----

AlwaysTruthy: _SpecialForm
"""
`AlwaysTruthy` represents the set of all objects that always evaluate to `True` in a boolean
context.

`AlwaysTruthy` is inhabited by singleton objects such as `True` and `...`, as well as truthy
literal strings, integers and bytestrings. It can also be inhabited by instances of classes
with `__bool__` methods returning `Literal[True]`.

In practice, most Python objects inhabit neither `AlwaysTruthy` nor `AlwaysFalsy`, since
their boolean evaluation may be uncertain or depend on runtime state. For example, although
an instance of *exactly* `object` is always truthy, a variable annotated as having type
`object` could also be an instance of an arbitrary subclass of `object` that is always falsy.
This means that the boolean evaluation of a variable inferred as `object` is uncertain, so
`object` is not a subtype of `AlwaysTruthy`.
"""

AlwaysFalsy: _SpecialForm
"""
`AlwaysFalsy` represents the set of all objects that always evaluate to `False` in a boolean
context.

`AlwaysFalsy` is inhabited by singleton objects such as `False` and `None`, as well as the
literals `""`, `b""` and `0`. It can also be inhabited by instances of classes with `__bool__`
methods returning `Literal[False]`.

In practice, however, most Python objects inhabit neither `AlwaysTruthy` nor `AlwaysFalsy`,
since their boolean evaluation may be uncertain or depend on runtime state.
"""

# ty treats annotations of `float` to mean `float | int`, and annotations of `complex`
# to mean `complex | float | int`. This is to support a typing-system special case [1].
# We therefore provide `JustFloat` and `JustComplex` to represent the "bare" `float` and
# `complex` types, respectively.
#
# [1]: https://typing.readthedocs.io/en/latest/spec/special-types.html#special-cases-for-float-and-complex
type JustFloat = _TypeOf[1.0]
type JustComplex = _TypeOf[1.0j]

class NamedTupleLike(Protocol):
    """
    A protocol describing an interface that should be satisfied by all named tuples
    created using `typing.NamedTuple` or `collections.namedtuple`.
    """

    # _fields is defined as `tuple[Any, ...]` rather than `tuple[str, ...]` so
    # that instances of actual `NamedTuple` classes with more precise `_fields`
    # types are considered assignable to this protocol (protocol attribute members
    # are invariant, and `tuple[str, str]` is not invariantly assignable to
    # `tuple[str, ...]`).
    _fields: ClassVar[tuple[Any, ...]]
    _field_defaults: ClassVar[dict[str, Any]]
    @classmethod
    def _make(cls: type[Self], iterable: collections.abc.Iterable[Any]) -> Self: ...
    def _asdict(self, /) -> dict[str, Any]: ...

    # Positional arguments aren't actually accepted by these methods at runtime,
    # but adding the `*args` parameters means that all `NamedTuple` classes
    # are understood as assignable to this protocol due to the special case
    # outlined in https://typing.python.org/en/latest/spec/callables.html#meaning-of-in-callable:
    #
    # > If the input signature in a function definition includes both a
    # > `*args` and `**kwargs` parameter and both are typed as `Any`
    # > (explicitly or implicitly because it has no annotation), a type
    # > checker should treat this as the equivalent of `...`.
    def _replace(self, *args, **kwargs) -> Self: ...
    if sys.version_info >= (3, 13):
        def __replace__(self, *args, **kwargs) -> Self: ...
