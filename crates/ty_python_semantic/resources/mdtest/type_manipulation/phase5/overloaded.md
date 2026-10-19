# Overloaded function types

`Overloaded[C1, C2, ...]` from [PEP 827] is the type of an overloaded function whose signatures are
the callables `C1`, `C2`, ... in order. ty displays it like the type of an `@overload`ed function.

```toml
[environment]
python-version = "3.14"
```

## Construction

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

## Inspection

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

## Overloaded methods in `Members`

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

## Invalid arguments

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

## Bare `Overloaded` outside of `Base` positions

```py
from ty_extensions import Overloaded

def _(a: Overloaded):  # error: [invalid-type-form]
    reveal_type(a)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
