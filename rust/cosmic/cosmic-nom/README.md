# COSMIC NOM
Is a collection of utilities for making using the great [nom](https://crates.io/crates/nom)
parser combinator easier to use.

## THE COSMIC INITIATIVE
`cosmic-nom` is part of a group of packages that compose [`The Cosmic Initiative`](http://thecosmicinitiative.io) although you can use it for any project you want.

## A DERIVATIVE OF DERIVATIVE WORKS
`cosmic-nom` synthesizes the utilities of two other derivatives of nom: 
* [nom-supreme](https://crates.io/crates/nom-supreme) A collection of excellent utilities for nom (which `cosmic-nom` uses for improved error handling)
* [nom_locate](https://crates.io/crates/nom_locate) A special input type for nom to locate tokens (which `cosmic-nom` uses to locate error spans in the parsed content)

## COSMIC NOMS CONTRIBUTIONS

### IMPLEMENTATION
To use `cosmic-nom` you must accept an input implementing trait `cosmic_nom::Span` and return a result of type `cosmic_nom::Res`: 
```rust
pub fn name<I:Span>( input:I ) -> Res<I,I> {
  alpha(input)
}
```

### SPAN
Since It is hard to compose a combinator if your input type doesn't implement all of the traits used in every condition: InputLength, InputTakeAtPosition, AsBytes, etc... 
So `cosmic-nom` provides a single trait that also supports location captures via `nom_locate` and seems to work with every combinator
in the `complete` package (not tested on streaming).

```rust
pub trait Span:
    Clone
    + ToString
    + AsBytes
    + Slice<Range<usize>>
    + Slice<RangeTo<usize>>
    + Slice<RangeFrom<usize>>
    + InputLength
    + Offset
    + InputTake
    + InputIter<Item = char>
    + InputTakeAtPosition<Item = char>
    + Compare<&'static str>
    + FindSubstring<&'static str>
    + core::fmt::Debug
```

To create a span call `cosmic_nom::new_span("scott williams")`

```rust
name(new_span("scott williams"));
```

### RESULT
use `result` to transform your `Res` into a regular `Result<O,E>`

```rust
let name: I = result(name(new_span("scott williams")))?;
```

### LOG
and you can wrap your result in a log() which will output to stdout if an error occurs:
```rust
let name: I = log(result(name(new_span("scott williams"))))?;
```

## IMPORTANT!
Be warned that in the service of making things easier and more reportable `cosmic-nom`
makes nom a little less awesome in some other ways... for instance `nom` has great efficiencies due to its "zero-copy"
input strategy and it seems to accomplish this by passing and slicing a single &str around...
`cosmic-nom` wraps input in an Arc and still takes great care to minimize overhead,
but it breaks with the pure spirit of nom in order to provide a little ease of use.
