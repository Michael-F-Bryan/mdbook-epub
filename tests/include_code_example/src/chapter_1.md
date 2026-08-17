# Chapter 1: Include Code Examples

This chapter demonstrates every variation of the `{{ # include}}`, `{{# rustdoc_include}}` and `{{ # playground}}` shortcodes supported by mdbook's `LinkPreprocessor`.

## `{{ # include}}` — whole file

Filename: output.txt

```console
{{#include ../listings/ch03-common-programming-concepts/no-listing-01-variables-are-immutable/output.txt}}
```

## `{{ # include}}` — whole Rust file

```rust
{{#include ../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs}}
```

## `{{ # include}}` — single line (`:N`)

Only line 3 of the file is inserted:

```text
{{#include ../listings/ch05-using-structs-to-structure-related-data/listing-05-11/output.txt:3}}
```

## `{{ # include}}` — inclusive line range (`:A:B`)

Lines 9 through 10 of the file are inserted:

```text
{{#include ../listings/ch05-using-structs-to-structure-related-data/listing-05-11/output.txt:9:10}}
```

## `{{ # include}}` — from line N to end of file (`:N:`)

```toml
{{#include ../listings/ch02-guessing-game-tutorial/listing-02-02/Cargo.toml:8:}}
```

## `{{ # include}}` — from start of file to line N (`::N`)

```toml
{{#include ../listings/ch02-guessing-game-tutorial/listing-02-02/Cargo.toml::5}}
```

## `{{ # include}}` — anchored section (`:anchor`)

Only the lines between `ANCHOR: here` and `ANCHOR_END: here` are inserted:

```rust
{{#include ../listings/ch09-error-handling/listing-09-08/src/main.rs:here}}
```

## `{{ # include}}` — nested include

The included file itself contains another `{{ # include}}`, which is expanded recursively relative to the included file's
own directory:
{{#include ../listings/_nested/nested.md}}


## `{{ # rustdoc_include}}` — whole file

```rust
{{#rustdoc_include ../listings/ch11-writing-automated-tests/listing-11-01/src/lib.rs}}
```

## `{{ # rustdoc_include}}` — anchored section (`:anchor`)

Lines outside the anchor are hidden behind `#` but kept for rustdoc testing:

```rust
{{#rustdoc_include ../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs:here}}
```

## `{{ # rustdoc_include}}` — numeric anchor names

Anchor names may contain digits, e.g. `1st` and `3rd`:

```rust
{{#rustdoc_include ../listings/ch10-generic-types-traits-and-lifetimes/no-listing-10-lifetimes-on-methods/src/main.rs:1st}}
```

```rust
{{#rustdoc_include ../listings/ch10-generic-types-traits-and-lifetimes/no-listing-10-lifetimes-on-methods/src/main.rs:3rd}}
```

## `{{ # playground}}` — plain

Unlike `{{ # include}}`, the playground shortcode generates its own code fence, so it is used bare (not wrapped in one):

{{#playground ../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs}}

## `{{# playground}}` — editable

The `editable` attribute lets readers edit the code in the browser:
{{ # include
{{#playground ../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs editable}}

## `{{ # playground}}` — multiple attributes

{{#playground ../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs editable no_run should_panic}}
