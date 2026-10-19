# `GetSpecialAttr`

`GetSpecialAttr[T, Attr]` reads the special attribute `Attr` (`"__name__"`, `"__module__"` or
`"__qualname__"`) of a class as a string literal type. For a type `T` that is not a class, the PEP's
principle applies: if `x` has type `T`, the result is `type(x).<attr>`. The operator is lifted over
unions.

```toml
[environment]
python-version = "3.14"
```

## Classes

```py
from typing import Literal
from ty_extensions import GetSpecialAttr
from models import Outer

class Local: ...

def _(
    a: GetSpecialAttr[int, Literal["__name__"]],
    b: GetSpecialAttr[int, Literal["__module__"]],
    c: GetSpecialAttr[list[int], Literal["__qualname__"]],
    d: GetSpecialAttr[Local, Literal["__name__"]],
    e: GetSpecialAttr[Outer.Inner, Literal["__name__"]],
    f: GetSpecialAttr[Outer.Inner, Literal["__qualname__"]],
    g: GetSpecialAttr[Outer.Inner, Literal["__module__"]],
):
    reveal_type(a)  # revealed: Literal["int"]
    reveal_type(b)  # revealed: Literal["builtins"]
    reveal_type(c)  # revealed: Literal["list"]
    reveal_type(d)  # revealed: Literal["Local"]
    reveal_type(e)  # revealed: Literal["Inner"]
    reveal_type(f)  # revealed: Literal["Outer.Inner"]
    reveal_type(g)  # revealed: Literal["models"]
```

`models.py`:

```py
class Outer:
    class Inner: ...
```

## Qualified names of local classes

`__qualname__` follows Python's rules, including the `<locals>` marker for classes defined in a
function.

```py
from typing import Literal
from ty_extensions import GetSpecialAttr

def outer():
    class Hidden: ...

    def inner(x: GetSpecialAttr[Hidden, Literal["__qualname__"]]):
        reveal_type(x)  # revealed: Literal["outer.<locals>.Hidden"]
```

## Types that are not classes

The class of a literal is the class of its value; the class of `type[C]` is the metaclass of `C`.

```py
from enum import Enum
from typing import Literal
from ty_extensions import GetSpecialAttr

class Color(Enum):
    RED = 1

class Meta(type): ...
class WithMeta(metaclass=Meta): ...

def _(
    a: GetSpecialAttr[Literal[1], Literal["__name__"]],
    b: GetSpecialAttr[Literal["x"], Literal["__name__"]],
    c: GetSpecialAttr[Literal[Color.RED], Literal["__name__"]],
    d: GetSpecialAttr[type[int], Literal["__name__"]],
    e: GetSpecialAttr[type[WithMeta], Literal["__name__"]],
    f: GetSpecialAttr[tuple[int, str], Literal["__name__"]],
):
    reveal_type(a)  # revealed: Literal["int"]
    reveal_type(b)  # revealed: Literal["str"]
    reveal_type(c)  # revealed: Literal["Color"]
    reveal_type(d)  # revealed: Literal["type"]
    reveal_type(e)  # revealed: Literal["Meta"]
    reveal_type(f)  # revealed: Literal["tuple"]
```

A `TypedDict` gives its own class name. (At runtime, `type(x)` of a `TypedDict` value is `dict`; ty
follows the reference implementation here, because the class name is the useful answer.)

```py
from typing import Literal, TypedDict
from ty_extensions import GetSpecialAttr

class Movie(TypedDict):
    title: str

def _(a: GetSpecialAttr[Movie, Literal["__name__"]]):
    reveal_type(a)  # revealed: Literal["Movie"]
```

## Lifting over unions

```py
from typing import Literal
from ty_extensions import GetSpecialAttr

def _(
    a: GetSpecialAttr[int | str, Literal["__name__"]],
    b: GetSpecialAttr[int, Literal["__name__", "__module__"]],
):
    reveal_type(a)  # revealed: Literal["int", "str"]
    reveal_type(b)  # revealed: Literal["int", "builtins"]
```

## `Any` and `Never`

```py
from typing import Any, Literal, Never
from ty_extensions import GetSpecialAttr

def _(a: GetSpecialAttr[Any, Literal["__name__"]], b: GetSpecialAttr[Never, Literal["__name__"]]):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Never
```

## Invalid attributes

Only the three attributes are supported, and the attribute must be a string literal type.

```py
from typing import Callable, Literal
from ty_extensions import GetSpecialAttr

def _(
    # error: [invalid-type-form] "`GetSpecialAttr` supports only `__name__`, `__module__` and `__qualname__`"
    a: GetSpecialAttr[int, Literal["__doc__"]],
    # error: [invalid-type-form] "The second argument to `GetSpecialAttr` must be a string literal type, but got `str`"
    b: GetSpecialAttr[int, str],
    # error: [invalid-type-form] "`GetSpecialAttr` requires a class, but got `(int, /) -> str`"
    c: GetSpecialAttr[Callable[[int], str], Literal["__name__"]],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Deferred evaluation

```py
from typing import Literal
from ty_extensions import GetSpecialAttr

type NameOf[T] = GetSpecialAttr[T, Literal["__name__"]]

def name_of[T](x: T) -> GetSpecialAttr[T, Literal["__name__"]]:
    raise NotImplementedError

class Dog: ...

def _(a: NameOf[Dog], dog: Dog):
    reveal_type(a)  # revealed: Literal["Dog"]
    reveal_type(name_of(dog))  # revealed: Literal["Dog"]
    reveal_type(name_of(1))  # revealed: Literal["int"]
```

[PEP 827]: https://peps.python.org/pep-0827/
