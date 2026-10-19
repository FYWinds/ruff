# Inspecting callables

[PEP 827] exposes the parameters of any callable in the extended callable format:
`GetArg[F, Callable, Literal[0]]` is a `Params[...]` with one `Param` per parameter, and
`GetArg[F, Callable, Literal[1]]` is the return type. `GetArgs[P, Params]` turns a `Params` into a
tuple that can be iterated with `Iter`.

ty keeps a single representation of callable types. `Params` is only a view that is computed when it
is asked for, so the method types returned by `Members` are displayed exactly as before.

```toml
[environment]
python-version = "3.14"
```

## Functions

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

## `Callable` types

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

## Methods

### Instance methods

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

### Methods of generic classes

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

### Methods without a return annotation

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

### Class methods and static methods

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

## Method types from `Members` are unchanged

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

## Iterating over parameters

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

## Callables that cannot be inspected as a single `Callable`

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

[PEP 827]: https://peps.python.org/pep-0827/
