# Phase 5 smoke tests (P5Callables)

Copies of the draft tests for extended callables, callable introspection, `Overloaded`, and the
`Params` / dot-access parts of `invalid.md`.

```toml
[environment]
python-version = "3.14"
```

## Extended callables

PEP 827 describes a callable's parameter list as `Params[*Ps]`, a sequence of `Param[N, T, K, D]`
types: the name (a string literal, or `None` for an unnamed parameter), the type, the kind (a
`ParamKind` literal, `POSITIONAL_OR_KEYWORD` by default) and the type of the default value (`Never`
if there is no default). `Callable[Params[...], R]` is converted to an ordinary callable type, so it
is displayed, called and compared like any other callable.

### All parameter kinds

This is the example from the PEP. The extended callable is equivalent to the callable type of the
corresponding function.

```py
from typing import Callable, Literal
from ty_extensions import Param, ParamKind, Params, static_assert
from ty_extensions._internal import RegularCallableTypeOf, is_equivalent_to

type Func = Callable[
    Params[
        Param[Literal["a"], int, Literal[ParamKind.POSITIONAL_ONLY]],
        Param[Literal["b"], int],
        Param[Literal["c"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal[0]],
        Param[None, int, Literal[ParamKind.VAR_POSITIONAL]],
        Param[Literal["d"], int, Literal[ParamKind.KEYWORD_ONLY]],
        Param[Literal["e"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]],
        Param[None, int, Literal[ParamKind.VAR_KEYWORD]],
    ],
    int,
]

def func(a: int, /, b: int, c: int = 0, *args: int, d: int, e: int = 0, **kwargs: int) -> int:
    return 0

def _(f: Func):
    reveal_type(f)  # revealed: (a: int, /, b: int, c: int = 0, *args: int, d: int, e: int = 0, **kwargs: int) -> int
    reveal_type(f(1, 2, d=3))  # revealed: int
    f(1, 2)  # error: [missing-argument]
    f(1, 2, d="x")  # error: [invalid-argument-type]

static_assert(is_equivalent_to(Func, RegularCallableTypeOf[func]))
```

### Abbreviations

The aliases from the PEP are available as well. A default given through an alias has the parameter's
type rather than a literal type, so it is displayed as `...`.

```py
from typing import Callable, Literal
from ty_extensions import (
    ArgsParam,
    DefaultParam,
    KwargsParam,
    NamedDefaultParam,
    NamedParam,
    Param,
    Params,
    PosParam,
)

type Func = Callable[
    Params[
        PosParam[int],
        Param[Literal["b"], int],
        DefaultParam[Literal["c"], int],
        ArgsParam[int],
        NamedParam[Literal["d"], int],
        NamedDefaultParam[Literal["e"], int],
        KwargsParam[int],
    ],
    int,
]

def _(f: Func):
    reveal_type(f)  # revealed: (int, /, b: int, c: int = ..., *args: int, d: int, e: int = ..., **kwargs: int) -> int
```

### Unnamed parameters

A parameter without a name can only be passed positionally. `Params` with only unnamed parameters is
the same as the classic list form.

```py
from typing import Callable
from ty_extensions import Param, Params, static_assert
from ty_extensions._internal import is_equivalent_to

type Classic = Callable[[int, str], bool]
type Extended = Callable[Params[Param[None, int], Param[None, str]], bool]

def _(f: Extended):
    reveal_type(f)  # revealed: (int, str, /) -> bool

static_assert(is_equivalent_to(Classic, Extended))
```

### Gradual parameter lists

`*args: Any` together with `**kwargs: Any` is the gradual parameter list `...`.

```py
from typing import Any, Callable, Literal
from ty_extensions import Param, ParamKind, Params, static_assert
from ty_extensions._internal import is_equivalent_to

type Gradual = Callable[
    Params[
        Param[None, Any, Literal[ParamKind.VAR_POSITIONAL]],
        Param[None, Any, Literal[ParamKind.VAR_KEYWORD]],
    ],
    int,
]

def _(f: Gradual):
    reveal_type(f)  # revealed: (...) -> int

static_assert(is_equivalent_to(Gradual, Callable[..., int]))
```

### Parameter lists of unknown length

When the arguments of `Params` come from a sequence of unknown length and contents (for example a
comprehension over `Any`), the parameter list is gradual.

```py
from typing import Any, Callable
from ty_extensions import Iter, Params

type Mapped[T] = Callable[Params[*[p for p in Iter[T]]], int]

def _(f: Mapped[Any]):
    reveal_type(f)  # revealed: (...) -> int
```

### Empty parameter lists

```py
from typing import Callable
from ty_extensions import Params

def _(f: Callable[Params[()], None]):
    reveal_type(f)  # revealed: () -> None
```

### Components of a `Param`

Like `Member`, `Param` is a regular class. Its components can be read with dot access in type
expressions (`.name`, `.type`, `.kind`, `.default`) and as properties on values.

```py
from typing import Literal
from ty_extensions import Param, ParamKind

type P = Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]]
type Q = Param[None, str]

def _(
    name: P.name,
    type_: P.type,
    kind: P.kind,
    default: P.default,
    q_name: Q.name,
    q_kind: Q.kind,
    q_default: Q.default,
):
    reveal_type(name)  # revealed: Literal["x"]
    reveal_type(type_)  # revealed: int
    reveal_type(kind)  # revealed: Literal[ParamKind.KEYWORD_ONLY]
    reveal_type(default)  # revealed: Literal[0]
    reveal_type(q_name)  # revealed: None
    reveal_type(q_kind)  # revealed: Literal[ParamKind.POSITIONAL_OR_KEYWORD]
    reveal_type(q_default)  # revealed: Never

def _(p: P):
    reveal_type(p)  # revealed: Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]]
    reveal_type(p.kind)  # revealed: Literal[ParamKind.KEYWORD_ONLY]
```

`Param` is covariant in all of its type arguments, so a `Param` with a different name or kind is not
assignable:

```py
from typing import Literal
from ty_extensions import Param, ParamKind, static_assert
from ty_extensions._internal import is_assignable_to

static_assert(is_assignable_to(Param[Literal["x"], bool], Param[Literal["x"], int]))
static_assert(not is_assignable_to(Param[Literal["x"], int], Param[Literal["y"], int]))
static_assert(
    not is_assignable_to(
        Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY]],
        Param[Literal["x"], int],
    )
)
```

### Building parameter lists with comprehensions

The arguments of `Params` are a variadic position: unpacked comprehensions and unpacked tuples are
spliced in.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Iter, Param, ParamKind, Params

class Point:
    x: int
    y: int = 0

type KwOnlyInit[T] = Callable[
    Params[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]],
    T,
]

type Suffix = tuple[Param[Literal["flag"], bool], Param[Literal["level"], int]]

def _(make: KwOnlyInit[Point], g: Callable[Params[Param[None, str], *Suffix], None]):
    reveal_type(make)  # revealed: (*, x: int, y: int = 0) -> Point
    reveal_type(make(x=1))  # revealed: Point
    reveal_type(g)  # revealed: (str, /, flag: bool, level: int) -> None
```

### Deferred evaluation

When the parameter list depends on a type variable, the callable type is evaluated once the type
variable is specialized: when a generic alias is specialized, or at a call site.

```py
from typing import Callable, Literal
from ty_extensions import Param, ParamKind, Params

type Setter[N: str, T] = Callable[Params[Param[N, T, Literal[ParamKind.KEYWORD_ONLY]]], None]

def _(a: Setter[Literal["width"], int], b: Setter[Literal["name"], str]):
    reveal_type(a)  # revealed: (*, width: int) -> None
    reveal_type(b)  # revealed: (*, name: str) -> None
    a(width=1)
    a(width=1, height=1)  # error: [unknown-argument]

def keyword_setter[T](default: T) -> Callable[Params[Param[Literal["value"], T, Literal[ParamKind.KEYWORD_ONLY], T]], None]:
    raise NotImplementedError

def _():
    reveal_type(keyword_setter(1.5))  # revealed: (*, value: float = ...) -> None
```

### `classmethod` and `staticmethod`

The `ParamSpec` argument of `classmethod` and `staticmethod` accepts `Params` as well.

```py
from typing import Literal
from ty_extensions import Param, Params

class C: ...

def _(
    c: classmethod[C, Params[Param[Literal["x"], int]], str],
    s: staticmethod[Params[Param[Literal["x"], int]], str],
):
    reveal_type(c)  # revealed: classmethod[C, (x: int), str]
    reveal_type(s)  # revealed: staticmethod[(x: int), str]
```


## Inspecting callables

PEP 827 exposes the parameters of any callable in the extended callable format:
`GetArg[F, Callable, Literal[0]]` is a `Params[...]` with one `Param` per parameter, and
`GetArg[F, Callable, Literal[1]]` is the return type. `GetArgs[P, Params]` turns a `Params` into a
tuple that can be iterated with `Iter`.

ty keeps a single representation of callable types. `Params` is only a view that is computed when it
is asked for, so the method types returned by `Members` are displayed exactly as before.

### Functions

Every parameter keeps its name and kind, including the names of `*args` and `**kwargs`. The default
is a literal type for `None`, `int`, `str`, `bytes`, `bool` and enum defaults, and the annotated type
of the parameter for other defaults. Parameters without a default have `Never`.

```py
from typing import Callable, Literal
from ty_extensions import GetArg, ParamKind
from ty_extensions._internal import TypeOf

def f(
    a: int, /, b: str = "x", *args: bytes, c: bool, e: tuple[int, ...] = (), **kwargs: float
) -> None: ...

def _(params: GetArg[TypeOf[f], Callable, Literal[0]], ret: GetArg[TypeOf[f], Callable, Literal[1]]):
    # revealed: Params[Param[Literal["a"], int, Literal[ParamKind.POSITIONAL_ONLY], Never], Param[Literal["b"], str, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal["x"]], Param[Literal["args"], bytes, Literal[ParamKind.VAR_POSITIONAL], Never], Param[Literal["c"], bool, Literal[ParamKind.KEYWORD_ONLY], Never], Param[Literal["e"], tuple[int, ...], Literal[ParamKind.KEYWORD_ONLY], tuple[int, ...]], Param[Literal["kwargs"], float, Literal[ParamKind.VAR_KEYWORD], Never]]
    reveal_type(params)
    reveal_type(ret)  # revealed: None
```

Converting the `Params` back into a callable gives the callable type of the function:

```py
from typing import Callable, Literal
from ty_extensions import GetArg, static_assert
from ty_extensions._internal import RegularCallableTypeOf, TypeOf, is_equivalent_to

def g(a: int, /, b: str, *, c: bool = True) -> bytes:
    return b""

type Rebuilt = Callable[GetArg[TypeOf[g], Callable, Literal[0]], GetArg[TypeOf[g], Callable, Literal[1]]]

static_assert(is_equivalent_to(Rebuilt, RegularCallableTypeOf[g]))
```

### `Callable` types

Parameters from the list form of `Callable` have no name. `...` is `*args: Any` together with
`**kwargs: Any`.

```py
from typing import Any, Callable, Literal
from ty_extensions import GetArg, GetArgs, ParamKind

def _(
    a: GetArg[Callable[[int, str], bool], Callable, Literal[0]],
    b: GetArg[Callable[..., bool], Callable, Literal[0]],
    c: GetArgs[Callable[[], bool], Callable],
    d: GetArg[Any, Callable, Literal[0]],
):
    # revealed: Params[Param[None, int, Literal[ParamKind.POSITIONAL_ONLY], Never], Param[None, str, Literal[ParamKind.POSITIONAL_ONLY], Never]]
    reveal_type(a)
    # revealed: Params[Param[None, Any, Literal[ParamKind.VAR_POSITIONAL], Never], Param[None, Any, Literal[ParamKind.VAR_KEYWORD], Never]]
    reveal_type(b)
    reveal_type(c)  # revealed: tuple[Params[()], bool]
    reveal_type(d)  # revealed: Any
```

### Methods

#### Instance methods

A method's receiver is a regular parameter. When it is not annotated, its type is the class that
defines the method (this is what the reference implementation does; the PEP does not say). ty
internally annotates such receivers with a synthetic `Self` type variable; the `Params` view replaces
it with the defining class, so the method is not treated as generic.

```py
from typing import Callable, Literal
from ty_extensions import GetArg, GetMemberType, ParamKind

class C:
    def m(self, x: int, y: str = "") -> bool:
        return True

type M = GetMemberType[C, Literal["m"]]

def _(params: GetArg[M, Callable, Literal[0]], ret: GetArg[M, Callable, Literal[1]]):
    # revealed: Params[Param[Literal["self"], C, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never], Param[Literal["x"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never], Param[Literal["y"], str, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal[""]]]
    reveal_type(params)
    reveal_type(ret)  # revealed: bool
```

#### Methods of generic classes

For a method of a generic class, the receiver and the class's type parameters are specialized like
the rest of the member type.

```py
from typing import Callable, Literal
from ty_extensions import GetArg, GetMemberType, ParamKind

class Box[T]:
    def put(self, item: T) -> "Box[T]":
        return self

type Put = GetMemberType[Box[int], Literal["put"]]

def _(params: GetArg[Put, Callable, Literal[0]], ret: GetArg[Put, Callable, Literal[-1]]):
    # revealed: Params[Param[Literal["self"], Box[int], Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never], Param[Literal["item"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never]]
    reveal_type(params)
    reveal_type(ret)  # revealed: Box[int]
```

#### Methods without a return annotation

A method with annotated parameters but no return annotation is a member (phase 2). Its receiver is
handled the same way. ty only infers return types at call sites, so the return type in the signature
is `Unknown`.

```py
from typing import Callable, Literal
from ty_extensions import GetArg, GetMemberType, ParamKind

class Counter:
    def add(self, n: int):
        return n + 1

type Add = GetMemberType[Counter, Literal["add"]]

def _(params: GetArg[Add, Callable, Literal[0]], ret: GetArg[Add, Callable, Literal[1]]):
    # revealed: Params[Param[Literal["self"], Counter, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never], Param[Literal["n"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never]]
    reveal_type(params)
    reveal_type(ret)  # revealed: Unknown
```

#### Class methods and static methods

Class methods and static methods are inspected through `classmethod` and `staticmethod`.
`GetArg[F, classmethod, ...]` gives the receiver class, the parameters without `cls`, and the return
type; `GetArg[F, staticmethod, ...]` gives the parameters and the return type.

```py
from typing import Literal
from ty_extensions import GetArg, GetMemberType, ParamKind

class C:
    @classmethod
    def make(cls, n: int) -> int:
        return n

    @staticmethod
    def util(n: int) -> str:
        return ""

type Make = GetMemberType[C, Literal["make"]]
type Util = GetMemberType[C, Literal["util"]]

def _(
    receiver: GetArg[Make, classmethod, Literal[0]],
    make_params: GetArg[Make, classmethod, Literal[1]],
    make_ret: GetArg[Make, classmethod, Literal[2]],
    util_params: GetArg[Util, staticmethod, Literal[0]],
    util_ret: GetArg[Util, staticmethod, Literal[1]],
):
    reveal_type(receiver)  # revealed: C
    reveal_type(make_params)  # revealed: Params[Param[Literal["n"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never]]
    reveal_type(make_ret)  # revealed: int
    reveal_type(util_params)  # revealed: Params[Param[Literal["n"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Never]]
    reveal_type(util_ret)  # revealed: str
```

### Method types from `Members` are unchanged

The `.type` of a method member is still ty's callable type of the function; the `Params` view does
not change it. Calling it works as in phase 2:

```py
from ty_extensions import Members

class A:
    def run(self, x: int) -> str:
        return ""

def _(m: Members[A]):
    reveal_type(m[0].name)  # revealed: Literal["run"]
    reveal_type(m[0].type(A(), 1))  # revealed: str
```

### Iterating over parameters

`GetArgs[P, Params]` is a tuple of the `Param`s, so a comprehension can map or filter them.

```py
from typing import Callable, Literal, Union
from ty_extensions import GetArg, GetArgs, IsAssignable, Iter, ParamKind, Params
from ty_extensions._internal import TypeOf

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type ParamNames[F] = tuple[*[p.name for p in Iter[ParamsOf[F]]]]
type KeywordNames[F] = Union[
    *[
        p.name
        for p in Iter[ParamsOf[F]]
        if IsAssignable[p.kind, Literal[ParamKind.POSITIONAL_OR_KEYWORD, ParamKind.KEYWORD_ONLY]]
    ]
]

def g(a: int, /, b: str, *, c: bool) -> None: ...

def _(names: ParamNames[TypeOf[g]], keywords: KeywordNames[TypeOf[g]]):
    reveal_type(names)  # revealed: tuple[Literal["a"], Literal["b"], Literal["c"]]
    reveal_type(keywords)  # revealed: Literal["b", "c"]

def keyword_names[F](f: F) -> KeywordNames[F]:
    raise NotImplementedError

reveal_type(keyword_names(g))  # revealed: Literal["b", "c"]
```

### Callables that cannot be inspected as a single `Callable`

An overloaded function has several signatures; use `Overloaded` to inspect it (see
`overloaded.md`). A generic function has its own type variables, and inspecting it is not supported
yet. Both are errors, and the result is `Unknown`.

```py
from typing import Callable, Literal, overload
from ty_extensions import GetArg
from ty_extensions._internal import TypeOf

@overload
def parse(x: int) -> str: ...
@overload
def parse(x: str) -> int: ...
def parse(x: int | str) -> int | str:
    return x

def ident[T](x: T) -> T:
    return x

def _(
    a: GetArg[TypeOf[parse], Callable, Literal[0]],  # error: [invalid-type-form]
    b: GetArg[TypeOf[ident], Callable, Literal[0]],  # error: [invalid-type-form]
    c: GetArg[int, Callable, Literal[0]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```


## Overloaded function types

`Overloaded[C1, C2, ...]` from PEP 827 is the type of an overloaded function whose signatures are
the callables `C1`, `C2`, ... in order. ty displays it like the type of an `@overload`ed function.

### Construction

```py
from typing import Callable, Literal
from ty_extensions import Overloaded, Param, Params

type Parse = Overloaded[
    Callable[[int], str],
    Callable[Params[Param[Literal["text"], str]], int],
]

def _(f: Parse):
    reveal_type(f)  # revealed: Overload[(int, /) -> str, (text: str) -> int]
    reveal_type(f(1))  # revealed: str
    reveal_type(f(text="a"))  # revealed: int
    f(b"x")  # error: [no-matching-overload]
```

Nested `Overloaded` arguments are flattened, and a single argument is just that callable:

```py
from typing import Callable
from ty_extensions import Overloaded, static_assert
from ty_extensions._internal import is_equivalent_to

type A = Callable[[int], int]
type B = Callable[[str], str]
type C = Callable[[bytes], bytes]

static_assert(is_equivalent_to(Overloaded[Overloaded[A, B], C], Overloaded[A, B, C]))
static_assert(is_equivalent_to(Overloaded[A], A))
```

The arguments are a variadic position, so they can come from a comprehension:

```py
from typing import Callable, Union
from ty_extensions import FromUnion, Iter, Overloaded

type Identities[T] = Overloaded[*[Callable[[t], t] for t in Iter[FromUnion[T]]]]

def _(f: Identities[int | str]):
    reveal_type(f)  # revealed: Overload[(int, /) -> int, (str, /) -> str]
```

### Inspection

`GetArgs[F, Overloaded]` is a tuple with one single-signature callable per overload. For a callable
with a single signature, it is a tuple with one element, so all callables can be iterated the same
way (like `FromUnion` of a type that is not a union).

As the `Base` argument of `GetArg` and `GetArgs`, `Overloaded` is written without arguments, like
any other base class. That is the only place where a bare `Overloaded` is allowed; in an annotation
it is still an error (see the last section).

```py
from typing import Callable, Literal, overload
from ty_extensions import GetArg, GetArgs, Length, Overloaded
from ty_extensions._internal import TypeOf

@overload
def parse(x: int) -> str: ...
@overload
def parse(x: str) -> int: ...
def parse(x: int | str) -> int | str:
    return x

def single(x: bytes) -> bytes:
    return x

def _(
    all_: GetArgs[TypeOf[parse], Overloaded],
    first: GetArg[TypeOf[parse], Overloaded, Literal[0]],
    count: Length[GetArgs[TypeOf[parse], Overloaded]],
    one: GetArgs[TypeOf[single], Overloaded],
):
    reveal_type(all_)  # revealed: tuple[(x: int) -> str, (x: str) -> int]
    reveal_type(first)  # revealed: (x: int) -> str
    reveal_type(count)  # revealed: Literal[2]
    reveal_type(one)  # revealed: tuple[(x: bytes) -> bytes]
```

### Overloaded methods in `Members`

The `.type` of an `@overload`ed method is an overloaded callable. Rebuilding a protocol from the
members keeps the overloads.

```py
from typing import Literal, overload
from ty_extensions import GetArgs, GetMemberType, Iter, Length, Members, NewProtocol, Overloaded

class Parser:
    @overload
    def parse(self, x: int) -> str: ...
    @overload
    def parse(self, x: str) -> int: ...
    def parse(self, x: int | str) -> int | str:
        return x

type Copy[T] = NewProtocol[*[m for m in Iter[Members[T]]]]

def _(
    n: Length[GetArgs[GetMemberType[Parser, Literal["parse"]], Overloaded]],
    p: Copy[Parser],
):
    reveal_type(n)  # revealed: Literal[2]
    reveal_type(p.parse(1))  # revealed: str
    reveal_type(p.parse("a"))  # revealed: int
```

### Invalid arguments

Every argument must be a callable type, and there must be at least one. The number of arguments must
be known: an unpacked tuple of unbounded length is rejected.

```py
from typing import Callable
from ty_extensions import Overloaded

type Many = tuple[Callable[[int], int], ...]

def _(
    a: Overloaded[Callable[[int], int], int],  # error: [invalid-type-form]
    b: Overloaded[()],  # error: [invalid-type-form]
    c: Overloaded[*Many],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

### Bare `Overloaded` outside of `Base` positions

```py
from ty_extensions import Overloaded

def _(a: Overloaded):  # error: [invalid-type-form]
    reveal_type(a)  # revealed: Unknown
```


## Invalid callables and class updates
### Elements of `Params`

Every element of `Params` must be a `Param`, and a `Param` must be wrapped in `Params`.

```py
from typing import Callable, Literal
from ty_extensions import Param, Params

def _(
    a: Callable[Params[int], None],  # error: [invalid-type-form]
    b: Callable[[Param[Literal["x"], int]], None],  # error: [invalid-type-form] "`Param` types must be wrapped in `Params[...]`"
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

### Names and kinds

The name of a `Param` must be a string literal or `None`. The kind must be exactly one `ParamKind`
literal. Keyword-only parameters need a name.

```py
from typing import Callable, Literal, Never
from ty_extensions import Param, ParamKind, Params

def _(
    a: Callable[Params[Param[int, int]], None],  # error: [invalid-type-arguments]
    # error: [invalid-type-form]
    b: Callable[Params[Param[Literal["x"], int, Literal[ParamKind.POSITIONAL_ONLY, ParamKind.KEYWORD_ONLY]]], None],
    c: Callable[Params[Param[Literal["x"], int, Never]], None],  # error: [invalid-type-form]
    d: Callable[Params[Param[Literal["x"], int, ParamKind]], None],  # error: [invalid-type-form]
    e: Callable[Params[Param[None, int, Literal[ParamKind.KEYWORD_ONLY]]], None],  # error: [invalid-type-form]
):
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
    reveal_type(d)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

### Parameter order and defaults

The parameters must form a valid signature: positional-only, then positional-or-keyword, then
`*args`, then keyword-only, then `**kwargs`; at most one `*args` and one `**kwargs`, neither with a
default; no duplicate names; and no positional parameter without a default after one with a default.
The reference implementation leaves these checks to `inspect.Signature` at runtime.

```py
from typing import Callable, Literal
from ty_extensions import ArgsParam, KwargsParam, NamedParam, Param, ParamKind, Params

type X = Param[Literal["x"], int]
type Y = Param[Literal["y"], int]
type DefaultX = Param[Literal["x"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal[0]]

def _(
    a: Callable[Params[NamedParam[Literal["k"], int], X], None],  # error: [invalid-type-form]
    b: Callable[Params[KwargsParam[int], X], None],  # error: [invalid-type-form]
    c: Callable[Params[ArgsParam[int], ArgsParam[str]], None],  # error: [invalid-type-form]
    d: Callable[Params[X, X], None],  # error: [invalid-type-form]
    e: Callable[Params[DefaultX, Y], None],  # error: [invalid-type-form]
    # error: [invalid-type-form]
    f: Callable[Params[Param[None, int, Literal[ParamKind.VAR_POSITIONAL], Literal[0]]], None],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

Errors that only appear after specialization are not reported, but the result is still `Unknown`:

```py
from typing import Callable, Literal
from ty_extensions import Param, Params

type Two[N1: str, N2: str] = Callable[Params[Param[N1, int], Param[N2, int]], None]

def _(ok: Two[Literal["a"], Literal["b"]], duplicate: Two[Literal["a"], Literal["a"]]):
    reveal_type(ok)  # revealed: (a: int, b: int) -> None
    reveal_type(duplicate)  # revealed: Unknown
```

### Unbounded parameter lists

The number of parameters must be known.

```py
from typing import Callable
from ty_extensions import Param, Params

type Many = tuple[Param[None, int], ...]

def _(f: Callable[Params[*Many], None]):  # error: [invalid-type-form]
    reveal_type(f)  # revealed: Unknown
```

### Dot access

`Param` has the components `name`, `type`, `kind` and `default`; `Member` has `name`, `type`,
`quals`, `init` and `definer`. Each class only has its own components: `.default` on a `Member` is
still an error, as in phase 2.

```py
from typing import Literal
from ty_extensions import Member, Param

type P = Param[Literal["x"], int]
type M = Member[Literal["x"], int]

def _(
    # error: [invalid-type-form] "`.quals` is not a component of `Param`; expected one of `name`, `type`, `kind` or `default`"
    a: P.quals,
    # error: [invalid-type-form] "`.kind` is not a component of `Member`; expected one of `name`, `type`, `quals`, `init` or `definer`"
    b: M.kind,
    c: M.default,  # error: [invalid-type-form]
    d: P.init,  # error: [invalid-type-form]
    e: P.foo,  # error: [invalid-type-form]
    f: int.kind,  # error: [invalid-type-form] "`.kind` in a type expression requires a `Member` or `Param` type, not `int`"
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Playground examples

### Extended callables

`Callable[Params[...], R]` describes any signature, one `Param[name, type, kind, default]` per
parameter. Unlike `Callable[[...], R]`, it can express names, keyword-only parameters and defaults.
The result is an ordinary callable type.

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Param, ParamKind, Params

type Handler = Callable[
    Params[
        Param[None, bytes],
        Param[Literal["retries"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[3]],
        Param[None, str, Literal[ParamKind.VAR_KEYWORD]],
    ],
    bool,
]

def run(handler: Handler):
    reveal_type(handler)  # revealed: (bytes, /, *, retries: int = 3, **kwargs: str) -> bool
    handler(b"data", retries=1, user="me")
    handler(b"data", retries="many")  # error: [invalid-argument-type]
```

### Inspecting parameters

`GetArg[F, Callable, Literal[0]]` is the parameter list of any callable as a `Params[...]`, and
`GetArgs[P, Params]` turns it into a tuple that a comprehension can iterate over. Here we list the
names of the parameters that can be passed by keyword.

```py
from typing import TYPE_CHECKING, Callable, Literal, Union

if TYPE_CHECKING:
    from ty_extensions import GetArg, GetArgs, IsAssignable, Iter, ParamKind, Params

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type Keyword = Literal[ParamKind.POSITIONAL_OR_KEYWORD, ParamKind.KEYWORD_ONLY]
type KeywordNames[F] = Union[*[p.name for p in Iter[ParamsOf[F]] if IsAssignable[p.kind, Keyword]]]
type ReturnOf[F] = GetArg[F, Callable, Literal[1]]

def keyword_names[F](f: F) -> KeywordNames[F]:
    raise NotImplementedError

def returns[F](f: F) -> ReturnOf[F]:
    raise NotImplementedError

def connect(host: str, /, port: int = 80, *, timeout: float = 1.0) -> bytes:
    return b""

def demo():
    reveal_type(keyword_names(connect))  # revealed: Literal["port", "timeout"]
    reveal_type(returns(connect))  # revealed: bytes
```

### Decorators that add a keyword parameter

`ParamSpec` cannot add a keyword parameter to an arbitrary function. With extended callables, a
decorator can copy the parameters of the function and append a keyword-only `timeout`.

```py
from typing import TYPE_CHECKING, Callable, Literal, cast

if TYPE_CHECKING:
    from ty_extensions import GetArg, GetArgs, Param, ParamKind, Params

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type Timeout = Param[Literal["timeout"], float, Literal[ParamKind.KEYWORD_ONLY], float]
type WithTimeout[F] = Callable[Params[*ParamsOf[F], Timeout], GetArg[F, Callable, Literal[1]]]

def with_timeout[F](f: F) -> WithTimeout[F]:
    return cast(WithTimeout[F], f)

@with_timeout
def fetch(url: str, retries: int = 0) -> bytes:
    return b""

def demo():
    reveal_type(fetch)  # revealed: (url: str, retries: int = 0, *, timeout: float = ...) -> bytes
    fetch("https://example.com", timeout=2.5)
    fetch("https://example.com", timeout="soon")  # error: [invalid-argument-type]
```
