# Invalid type manipulation

All misuses of the [PEP 827] type operators are reported as `invalid-type-form`. An operation with an
invalid operand evaluates to `Unknown`, so that one mistake does not cause a cascade of errors.

```toml
[environment]
python-version = "3.14"
```

## Bare special forms

The operators must be subscripted.

```py
from ty_extensions import Bool, IsAssignable, IsEquivalent

def _(
    a: IsAssignable,  # error: [invalid-type-form]
    b: IsEquivalent,  # error: [invalid-type-form]
    c: Bool,  # error: [invalid-type-form]
):
    pass
```

## Wrong number of arguments

`IsAssignable` and `IsEquivalent` take exactly two arguments, `Bool` takes exactly one.

```py
from ty_extensions import Bool, IsAssignable, IsEquivalent

def _(
    a: IsAssignable[int],  # error: [invalid-type-form]
    b: IsAssignable[int, str, bytes],  # error: [invalid-type-form]
    c: IsEquivalent[int],  # error: [invalid-type-form]
    d: IsEquivalent[int, str, bytes],  # error: [invalid-type-form]
    e: Bool[int, str],  # error: [invalid-type-form]
    f: Bool[()],  # error: [invalid-type-form]
):
    pass
```

## Operands that are not types

Each argument must itself be a valid type expression.

```py
from ty_extensions import Bool, IsAssignable

def _(
    a: IsAssignable[1, int],  # error: [invalid-type-form]
    b: Bool[[int]],  # error: [invalid-type-form]
):
    pass
```

## Condition is not a type boolean

The condition of a conditional type must evaluate to a type boolean: `Literal[True]`,
`Literal[False]`, `bool`, an unevaluated type boolean, or a dynamic type. Other types are rejected.

```py
from ty_extensions import IsAssignable

type NotABoolean = int

def _(
    # error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
    a: int if str else bytes,
    # error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
    b: int if NotABoolean else bytes,
    # error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
    c: int if list[IsAssignable[int, int]] else bytes,
):
    reveal_type(a)  # revealed: Unknown
```

The same applies when the condition is deferred:

```py
from ty_extensions import IsAssignable

# error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
type Pick[T] = int if list[T] else bytes
```

## Operands of `not`, `and` and `or` are not type booleans

```py
from ty_extensions import IsAssignable

def _(
    # error: [invalid-type-form] "Operands of `and`/`or`/`not` in a type expression must be type booleans"
    a: not int,
    # error: [invalid-type-form] "Operands of `and`/`or`/`not` in a type expression must be type booleans"
    b: IsAssignable[int, int] and str,
    # error: [invalid-type-form] "Operands of `and`/`or`/`not` in a type expression must be type booleans"
    c: bytes or IsAssignable[int, int],
):
    reveal_type(a)  # revealed: Unknown
```

## Other syntax keeps the existing diagnostics

`if`, `and`, `or` and `not` are only treated as type-level operations when their conditions and
operands are written as type expressions (names, attributes, subscripts, or nested `not`/`and`/`or`
of those). Other forms, such as literals, are still reported with the same messages as before PEP
827 support was added.

```py
from ty_extensions import IsAssignable

def _(
    a: 1 and 2,  # error: [invalid-type-form] "Boolean operations are not allowed in parameter annotations"
    b: 1 or 2,  # error: [invalid-type-form] "Boolean operations are not allowed in parameter annotations"
    c: not 1,  # error: [invalid-type-form] "Unary operations are not allowed in parameter annotations"
    d: 1 if True else 2,  # error: [invalid-type-form] "`if` expressions are not allowed in parameter annotations"
    e: int if True else str,  # error: [invalid-type-form] "`if` expressions are not allowed in parameter annotations"
    f: int if 1 > 0 else str,  # error: [invalid-type-form] "`if` expressions are not allowed in parameter annotations"
    g: IsAssignable[int, int] and 1,  # error: [invalid-type-form] "Boolean operations are not allowed in parameter annotations"
    h: not (1 or IsAssignable[int, int]),  # error: [invalid-type-form] "Unary operations are not allowed in parameter annotations"
):
    pass
```

The same holds in string annotations:

```py
def _(
    a: "1 and 2",  # error: [invalid-type-form] "Boolean operations are not allowed in parameter annotations"
    b: "not 1",  # error: [invalid-type-form] "Unary operations are not allowed in parameter annotations"
    c: "1 if True else 2",  # error: [invalid-type-form] "`if` expressions are not allowed in parameter annotations"
):
    pass
```

[PEP 827]: https://peps.python.org/pep-0827/
