# Recursive type aliases with type operators

Generic aliases can refer to themselves inside a conditional type. When such an alias is specialized,
only the branch selected by the condition is expanded, so the recursion stops at the base case. A
recursion whose arguments keep growing is cut off at a fixed nesting depth and silently evaluates to
`Unknown`.

The first two sections only use phase 1 and 2 operators (milestone R1). "Reversing a tuple" needs the
phase 3 tuple operators (R2), and the `Zip` sections need `RaiseError` (R4).

Results that contain helper aliases (such as `Last[...]`) display those aliases by name, so the
`Zip` results are checked with `IsEquivalent` instead of `reveal_type`.

```toml
[environment]
python-version = "3.14"
```

## Only the selected branch is expanded

`Unroll` turns a linked list of classes into a tuple. At the base case `Nil`, the other branch would
ask for `Unroll[GetMemberType[Nil, Literal["tail"]]]`, which is `Unroll[Never]`, whose other branch
is `Unroll[Never]` again: a cycle. Only expanding the selected branch avoids it.

```py
from typing import Literal
from ty_extensions import GetMemberType, IsAssignable

class Nil: ...

class Cons[H, T]:
    head: H
    tail: T

type Unroll[L] = (
    tuple[()]
    if IsAssignable[L, Nil]
    else tuple[GetMemberType[L, Literal["head"]], *Unroll[GetMemberType[L, Literal["tail"]]]]
)

def _(
    a: Unroll[Cons[int, Cons[str, Nil]]],
    b: Unroll[Nil],
    c: Unroll[Cons[bytes, Nil]],
):
    reveal_type(a)  # revealed: tuple[int, str]
    reveal_type(b)  # revealed: tuple[()]
    reveal_type(c)  # revealed: tuple[bytes]
```

## Growing arguments are cut off

`Forever` never reaches its base case, and its argument grows by one `list` per step. ty stops
expanding once the argument is nested too deeply. We only assert that checking terminates; the
resulting type is not specified, and no diagnostic is reported for it.

```toml
[environment]
python-version = "3.14"

[rules]
cyclic-type-alias-definition = "ignore"
```

```py
from ty_extensions import IsAssignable

type Forever[T] = Forever[list[T]] if IsAssignable[T, object] else T

def _(a: Forever[int]):
    # Checking continues normally after the divergent alias.
    reveal_type(1)  # revealed: Literal[1]
```

## Reversing a tuple

```py
from typing import Literal
from ty_extensions import GetArg, IsAssignable, Length, Slice

type Reverse[T] = (
    tuple[()]
    if IsAssignable[Length[T], Literal[0]]
    else tuple[*Reverse[Slice[T, Literal[1], None]], GetArg[T, tuple, Literal[0]]]
)

def _(a: Reverse[tuple[int, str, bytes]], b: Reverse[tuple[()]], c: Reverse[tuple[int]]):
    reveal_type(a)  # revealed: tuple[bytes, str, int]
    reveal_type(b)  # revealed: tuple[()]
    reveal_type(c)  # revealed: tuple[int]
```

## `Zip` from the PEP

This is the `Zip` alias from the "zip-like functions" section of the PEP. It walks down both tuples
from the end until one of them is empty, and raises an error if only one of them is.

(The PEP's prose says that zipping `tuple[int, str]` and `tuple[str, bool]` gives
`tuple[tuple[int, float], tuple[str, bool]]`; the inputs in that sentence are a typo. The reference
implementation's tests zip `tuple[int, str]` with `tuple[float, bool]`, as below.)

The recursive reference is inside a conditional type, so `Zip` is a valid recursive alias: its
definition produces no `cyclic-type-alias-definition` diagnostic.

```py
from typing import Literal
from ty_extensions import Bool, GetArg, IsAssignable, IsEquivalent, Length, RaiseError, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
type Last[T] = GetArg[T, tuple, Literal[-1]]
type Empty[T] = IsAssignable[Length[T], Literal[0]]

type Zip[T, S] = (
    tuple[()]
    if Bool[Empty[T]] and Bool[Empty[S]]
    else RaiseError[Literal["Zip length mismatch"], T, S]
    if Bool[Empty[T]] or Bool[Empty[S]]
    else tuple[*Zip[DropLast[T], DropLast[S]], tuple[Last[T], Last[S]]]
)

def _(
    two: IsEquivalent[Zip[tuple[int, str], tuple[float, bool]], tuple[tuple[int, float], tuple[str, bool]]],
    one: IsEquivalent[Zip[tuple[int], tuple[str]], tuple[tuple[int, str]]],
    three: IsEquivalent[
        Zip[tuple[int, str, float], tuple[bool, bytes, list[int]]],
        tuple[tuple[int, bool], tuple[str, bytes], tuple[float, list[int]]],
    ],
    empty: Zip[tuple[()], tuple[()]],
    # error: [raised-type-error] "Zip length mismatch: tuple[int], tuple[()]"
    short: Zip[tuple[int, str], tuple[float]],
    # error: [raised-type-error] "Zip length mismatch: tuple[()], tuple[float, bool]"
    long: Zip[tuple[int], tuple[float, bool, str]],
):
    reveal_type(two)  # revealed: Literal[True]
    reveal_type(one)  # revealed: Literal[True]
    reveal_type(three)  # revealed: Literal[True]
    reveal_type(empty)  # revealed: tuple[()]
    # Unpacking the raised error in the recursive branch makes the whole result the error.
    reveal_type(short)  # revealed: Unknown
```

## `zip_pairs`

The PEP's `zip_pairs` uses two `TypeVarTuple`s; plain type variables bounded by `tuple` work the same
way here. A mismatch is reported on the call.

```py
from typing import Literal
from ty_extensions import Bool, GetArg, IsAssignable, Length, RaiseError, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
type Last[T] = GetArg[T, tuple, Literal[-1]]
type Empty[T] = IsAssignable[Length[T], Literal[0]]
type Zip[T, S] = (
    tuple[()]
    if Bool[Empty[T]] and Bool[Empty[S]]
    else RaiseError[Literal["Zip length mismatch"], T, S]
    if Bool[Empty[T]] or Bool[Empty[S]]
    else tuple[*Zip[DropLast[T], DropLast[S]], tuple[Last[T], Last[S]]]
)

def zip_pairs[T: tuple[object, ...], S: tuple[object, ...]](a: T, b: S) -> Zip[T, S]:
    raise NotImplementedError

def _(a: tuple[int, str], b: tuple[float, bool], c: tuple[float]):
    ok: tuple[tuple[int, float], tuple[str, bool]] = zip_pairs(a, b)
    wrong: tuple[tuple[int, int], tuple[str, bool]] = zip_pairs(a, b)  # error: [invalid-assignment]
    # error: [raised-type-error] "Zip length mismatch: tuple[int], tuple[()]"
    zip_pairs(a, c)
```

## `ZipN`

`ZipN` from the reference implementation's tests zips any number of tuples. It iterates over the
tuple of inputs, and checks that all lengths agree by comparing a union of lengths with one length.

```py
from typing import Literal, Union
from ty_extensions import Bool, GetArg, IsAssignable, IsEquivalent, Iter, Length, RaiseError, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
type Last[T] = GetArg[T, tuple, Literal[-1]]
type Empty[T] = IsAssignable[Length[T], Literal[0]]
type First[T] = GetArg[T, tuple, Literal[0]]
type DropLastEach[T] = tuple[*[DropLast[t] for t in Iter[T]]]
type LastEach[T] = tuple[*[Last[t] for t in Iter[T]]]
type AllSameLength[T] = IsEquivalent[Union[*[Length[t] for t in Iter[T]]], Length[First[T]]]

type ZipN[T] = (
    tuple[()]
    if Bool[Empty[First[T]]] and AllSameLength[T]
    else tuple[*ZipN[DropLastEach[T]], LastEach[T]]
    if AllSameLength[T]
    else RaiseError[Literal["ZipN length mismatch"], T]
)

def _(
    three: IsEquivalent[
        ZipN[tuple[tuple[int, str], tuple[float, bool], tuple[list[int], dict[str, int]]]],
        tuple[tuple[int, float, list[int]], tuple[str, bool, dict[str, int]]],
    ],
    single: IsEquivalent[ZipN[tuple[tuple[int, str]]], tuple[tuple[int], tuple[str]]],
    empty: ZipN[tuple[tuple[()], tuple[()]]],
    # error: [raised-type-error] "ZipN length mismatch: tuple[tuple[int, str], tuple[float]]"
    bad: ZipN[tuple[tuple[int, str], tuple[float]]],
):
    reveal_type(three)  # revealed: Literal[True]
    reveal_type(single)  # revealed: Literal[True]
    reveal_type(empty)  # revealed: tuple[()]
```

## `Zip` of unbounded tuples terminates

`Zip` of two unbounded tuples never reaches its base case: `Length` is `None`, and dropping the last
element of `tuple[int, ...]` gives `tuple[int, ...]` again, wrapped in one more `DropLast`. The
arguments keep growing, so the depth limit applies. We only assert that checking terminates.

```py
from typing import Literal
from ty_extensions import Bool, GetArg, IsAssignable, Length, RaiseError, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
type Last[T] = GetArg[T, tuple, Literal[-1]]
type Empty[T] = IsAssignable[Length[T], Literal[0]]
type Zip[T, S] = (
    tuple[()]
    if Bool[Empty[T]] and Bool[Empty[S]]
    else RaiseError[Literal["Zip length mismatch"], T, S]
    if Bool[Empty[T]] or Bool[Empty[S]]
    else tuple[*Zip[DropLast[T], DropLast[S]], tuple[Last[T], Last[S]]]
)

def _(a: Zip[tuple[int, ...], tuple[str, ...]]):
    reveal_type(1)  # revealed: Literal[1]
```

[PEP 827]: https://peps.python.org/pep-0827/
