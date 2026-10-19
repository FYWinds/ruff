# `GetArg` and `GetArgs`

`GetArg[T, Base, Idx]` interprets `T` as an instance of `Base` and returns the type argument number
`Idx` of `Base`, looking through the base classes of `T`. `GetArgs[T, Base]` returns all of them as a
tuple. Both operators are lifted over unions.

```toml
[environment]
python-version = "3.14"
```

## Type arguments of the class itself

Negative indexes count from the end, as in Python.

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

def _(
    a: GetArg[list[int], list, Literal[0]],
    b: GetArg[dict[str, bytes], dict, Literal[1]],
    c: GetArg[dict[str, bytes], dict, Literal[-1]],
    d: GetArg[dict[str, bytes], dict, Literal[-2]],
    e: GetArgs[dict[str, bytes], dict],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: bytes
    reveal_type(d)  # revealed: str
    reveal_type(e)  # revealed: tuple[str, bytes]
```

## Type arguments of a base class

This is the example from the PEP: for `class A(B[C])`, `GetArg[A, B, Literal[0]]` is `C`. Type
parameters are substituted along the way, so a subclass can reorder the arguments of its base.

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

class C: ...
class B[T]: ...
class A(B[C]): ...
class Pair[K, V]: ...
class Swapped[X, Y](Pair[Y, X]): ...

def _(
    a: GetArg[A, B, Literal[0]],
    b: GetArg[Swapped[int, str], Pair, Literal[0]],
    c: GetArg[Swapped[int, str], Swapped, Literal[0]],
    d: GetArgs[Swapped[int, str], Pair],
):
    reveal_type(a)  # revealed: C
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: tuple[str, int]
```

Only the class of `Base` matters; type arguments written on `Base` are ignored:

```py
from typing import Literal
from ty_extensions import GetArg

def _(a: GetArg[list[int], list[str], Literal[0]]):
    reveal_type(a)  # revealed: int
```

## Errors

`A` is not generic, so it has no type argument `0`. `A` is not a `list` at all. And `list` has only
one type parameter.

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

class B[T]: ...
class A(B[int]): ...

def _(
    # error: [invalid-type-form] "Index 0 is out of range for `A` as `A`"
    a: GetArg[A, A, Literal[0]],
    # error: [invalid-type-form] "`GetArg` requires a subclass of `list`, but got `A`"
    b: GetArg[A, list, Literal[0]],
    # error: [invalid-type-form] "Index 1 is out of range for `list[int]` as `list`"
    c: GetArg[list[int], list, Literal[1]],
    # error: [invalid-type-form] "`GetArgs` requires a subclass of `dict`, but got `A`"
    d: GetArgs[A, dict],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
    reveal_type(d)  # revealed: Unknown
```

## Unspecialized generic classes

A bare generic class in a type expression uses the defaults of its type parameters, and `Unknown` for
type parameters without a default. (The reference implementation uses `Any` instead of `Unknown`.)

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

class Box[T]: ...
class WithDefault[T = str]: ...

def _(
    a: GetArg[list, list, Literal[0]],
    b: GetArg[WithDefault, WithDefault, Literal[0]],
    c: GetArg[Box, Box, Literal[-1]],
    d: GetArgs[WithDefault[int], WithDefault],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: Unknown
    reveal_type(d)  # revealed: tuple[int]
```

## Protocols from typeshed

Containers in typeshed inherit from `Iterable` and `Mapping` nominally, so their type arguments can be
read through those protocols. `tuple[int, str]` is a `Sequence[int | str]`.

```py
from collections.abc import Iterable, Mapping
from typing import Literal
from ty_extensions import GetArg

def _(
    a: GetArg[list[int], Iterable, Literal[0]],
    b: GetArg[dict[str, int], Mapping, Literal[1]],
    c: GetArg[tuple[int, str], Iterable, Literal[0]],
    d: GetArg[str, Iterable, Literal[0]],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: int | str
    reveal_type(d)  # revealed: str
```

A class that implements a protocol only structurally (without inheriting from it) is not supported
yet.

```py
from collections.abc import Iterable, Iterator
from typing import Literal
from ty_extensions import GetArg

class Counter:
    def __iter__(self) -> Iterator[int]:
        raise NotImplementedError

def _(
    # error: [invalid-type-form] "`GetArg` requires a subclass of `Iterable`, but got `Counter`"
    a: GetArg[Counter, Iterable, Literal[0]],
):
    reveal_type(a)  # revealed: Unknown
```

## `tuple` as the base

With `tuple` as the base, the type arguments are the elements of the tuple. This is what the PEP's
`Last[T] = GetArg[T, tuple, Literal[-1]]` relies on. Indexing a tuple of unbounded length gives the
same type as indexing a value of that type. (The reference implementation returns the raw arguments
of `tuple[...]` instead, including `...`.)

```py
from typing import Literal, NamedTuple
from ty_extensions import GetArg, GetArgs

class Point(NamedTuple):
    x: int
    label: str

def _(
    a: GetArg[tuple[int, str, bytes], tuple, Literal[-1]],
    b: GetArg[tuple[int, str], tuple, Literal[0]],
    c: GetArg[tuple[int, *tuple[str, ...]], tuple, Literal[0]],
    d: GetArg[tuple[int, *tuple[str, ...]], tuple, Literal[-1]],
    e: GetArg[tuple[int, *tuple[str, ...], bytes], tuple, Literal[-1]],
    f: GetArg[Point, tuple, Literal[1]],
    g: GetArgs[tuple[int, str], tuple],
    h: GetArgs[tuple[int, ...], tuple],
    i: GetArgs[tuple[()], tuple],
):
    reveal_type(a)  # revealed: bytes
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: int | str
    reveal_type(e)  # revealed: bytes
    reveal_type(f)  # revealed: str
    reveal_type(g)  # revealed: tuple[int, str]
    reveal_type(h)  # revealed: tuple[int, ...]
    reveal_type(i)  # revealed: tuple[()]
```

```py
from typing import Literal
from ty_extensions import GetArg

def _(
    # error: [invalid-type-form] "Index 1 is out of range for `tuple[int]` as `tuple`"
    a: GetArg[tuple[int], tuple, Literal[1]],
    # error: [invalid-type-form] "Index -1 is out of range for `tuple[()]` as `tuple`"
    b: GetArg[tuple[()], tuple, Literal[-1]],
):
    reveal_type(a)  # revealed: Unknown
```

## `type` and `Callable` as the base

`type[C]` has the type argument `C`. For a `Callable`, index `0` (or `-2`) is the parameter list as
an extended callable `Params[...]`, and index `1` (or `-1`) is the return type.

```py
from collections.abc import Callable
from typing import Literal
from ty_extensions import GetArg, GetArgs

def _(
    a: GetArg[type[int], type, Literal[0]],
    b: GetArg[Callable[[int], str], Callable, Literal[1]],
    c: GetArg[Callable[[int], str], Callable, Literal[-1]],
    d: GetArg[Callable[[int], str], Callable, Literal[0]],
    e: GetArgs[Callable[[int], str], Callable],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: str
    reveal_type(d)  # revealed: Params[Param[None, int, Literal[ParamKind.POSITIONAL_ONLY], Never]]
    # revealed: tuple[Params[Param[None, int, Literal[ParamKind.POSITIONAL_ONLY], Never]], str]
    reveal_type(e)
```

A callable whose signature has its own type parameters is rejected: its return type would mention a
type variable that only means something inside that signature. The synthetic `Self` type variable
that ty adds to methods is replaced by the class that defines the method.

```py
from collections.abc import Callable
from typing import Literal, Self
from ty_extensions import GetArg, GetMemberType

class Box[T]:
    def count(self) -> int:
        raise NotImplementedError

    def get(self) -> T:
        raise NotImplementedError

    def first[U](self, items: list[U]) -> U:
        raise NotImplementedError

    def clone(self) -> Self:
        raise NotImplementedError

type ReturnOf[F] = GetArg[F, Callable, Literal[-1]]

def _(
    a: ReturnOf[GetMemberType[Box[str], Literal["count"]]],
    b: ReturnOf[GetMemberType[Box[str], Literal["get"]]],
    # error: [invalid-type-form] "Getting the return type of a generic callable with `GetArg` is not supported yet"
    c: GetArg[GetMemberType[Box[str], Literal["first"]], Callable, Literal[-1]],
    d: GetArg[GetMemberType[Box[str], Literal["clone"]], Callable, Literal[-1]],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: Unknown
    reveal_type(d)  # revealed: Box[str]
```

## `TypeVarTuple` parameters

The arguments bound to a `TypeVarTuple` are flattened into the list of type arguments.

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

class Pack[*Ps]: ...
class Tagged[T, *Ps]: ...

def _(
    a: GetArgs[Pack[int, str], Pack],
    b: GetArg[Pack[int, str], Pack, Literal[1]],
    c: GetArgs[Tagged[bytes, int, str], Tagged],
    d: GetArg[Tagged[bytes, int, str], Tagged, Literal[-1]],
    e: GetArgs[Pack[()], Pack],
    f: GetArgs[Tagged[bytes, *tuple[int, ...]], Tagged],
):
    reveal_type(a)  # revealed: tuple[int, str]
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: tuple[bytes, int, str]
    reveal_type(d)  # revealed: str
    reveal_type(e)  # revealed: tuple[()]
    reveal_type(f)  # revealed: tuple[bytes, *tuple[int, ...]]
```

## `Any` and `Never`

If `T` is `Any`, the result is `Any`. `Never` is an empty union, so the lifted result is `Never`.

```py
from typing import Any, Literal, Never
from ty_extensions import GetArg, GetArgs

def _(
    a: GetArg[Any, list, Literal[0]],
    b: GetArgs[Any, list],
    c: GetArg[Never, list, Literal[0]],
    d: GetArgs[Never, list],
):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Any
    reveal_type(c)  # revealed: Never
    reveal_type(d)  # revealed: Never
```

## Lifting over unions

Every argument is lifted, including the index.

```py
from collections.abc import Iterable
from typing import Literal
from ty_extensions import GetArg, GetArgs

def _(
    a: GetArg[list[int] | set[str], Iterable, Literal[0]],
    b: GetArg[dict[str, bytes], dict, Literal[0, 1]],
    c: GetArgs[list[int] | tuple[str, bytes], Iterable],
):
    reveal_type(a)  # revealed: int | str
    reveal_type(b)  # revealed: str | bytes
    reveal_type(c)  # revealed: tuple[int] | tuple[str | bytes]
```

If one combination fails, the whole operation fails:

```py
from typing import Literal
from ty_extensions import GetArg

def _(
    # error: [invalid-type-form] "`GetArg` requires a subclass of `list`, but got `set[str]`"
    a: GetArg[list[int] | set[str], list, Literal[0]],
):
    reveal_type(a)  # revealed: Unknown
```

## Deferred evaluation

With a type variable as an argument, the operators are evaluated once the type variable is
specialized: in a generic alias, or at the call site of a generic function.

```py
from collections.abc import Iterable
from typing import Literal
from ty_extensions import GetArg, GetArgs

type ElemOf[T] = GetArg[T, Iterable, Literal[0]]

def first[T](items: T) -> GetArg[T, Iterable, Literal[0]]:
    raise NotImplementedError

def show[T](x: T, elem: GetArg[T, list, Literal[0]], args: GetArgs[T, dict]):
    reveal_type(elem)  # revealed: GetArg[T@show, list, Literal[0]]
    reveal_type(args)  # revealed: GetArgs[T@show, dict]

def _(a: ElemOf[list[bytes]], b: ElemOf[frozenset[int]], nums: list[int], names: tuple[str, ...]):
    reveal_type(a)  # revealed: bytes
    reveal_type(b)  # revealed: int
    reveal_type(first(nums))  # revealed: int
    reveal_type(first(names))  # revealed: str
```

## Prisma-style pointers

`PointerArg` from the PEP's Prisma example reads the type argument of `Pointer` from any of its
subclasses.

```py
from typing import Literal
from ty_extensions import GetArg, IsAssignable

class Pointer[T]: ...
class Property[T](Pointer[T]): ...
class Link[T](Pointer[T]): ...
class SingleLink[T](Link[T]): ...
class MultiLink[T](Link[T]): ...

class Post: ...

type PointerArg[T] = GetArg[T, Pointer, Literal[0]]
type AdjustLink[Tgt, LinkTy] = list[Tgt] if IsAssignable[LinkTy, MultiLink] else Tgt
type ConvertField[T] = AdjustLink[PointerArg[T], T] if IsAssignable[T, Link] else PointerArg[T]

def _(
    a: PointerArg[Property[str]],
    b: PointerArg[MultiLink[Post]],
    c: ConvertField[Property[int]],
    d: ConvertField[SingleLink[Post]],
    e: ConvertField[MultiLink[Post]],
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: Post
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: Post
    # The element type is the alias `PointerArg[MultiLink[Post]]`, which is displayed by name
    # inside `list[...]`; indexing resolves it.
    reveal_type(e[0])  # revealed: Post
```

[PEP 827]: https://peps.python.org/pep-0827/
