# `RaiseError` in the return type of a decorator

A `RaiseError` selected by the specialized return type of a call is reported with
`raised-type-error` at the call. Applying the callable as a decorator is a call too, so the error is
reported at the decorator (instead of `dynamic-function-decorator-return` when the `RaiseError` is
the whole return type).

```toml
[environment]
python-version = "3.14"

[rules]
dynamic-function-decorator-return = "error"
```

## Function decorators

```py
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]

def nested[T](x: T) -> list[OnlyInts[T]]:
    raise NotImplementedError

def top[T](x: T) -> OnlyInts[T]:
    raise NotImplementedError

def f() -> None: ...

nested(f)  # error: [raised-type-error] "expected an int"
top(f)  # error: [raised-type-error] "expected an int"

@nested  # error: [raised-type-error] "expected an int"
def g() -> None: ...

@top  # error: [raised-type-error] "expected an int"
def h() -> None: ...

reveal_type(g)  # revealed: list[OnlyInts[def g() -> None]]
reveal_type(h)  # revealed: Unknown
```

## Class decorators

```py
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]

def nested[T](x: T) -> list[OnlyInts[T]]:
    raise NotImplementedError

def top[T](x: T) -> OnlyInts[T]:
    raise NotImplementedError

@nested  # error: [raised-type-error] "expected an int"
class C: ...

@top  # error: [raised-type-error] "expected an int"
class D: ...
```
