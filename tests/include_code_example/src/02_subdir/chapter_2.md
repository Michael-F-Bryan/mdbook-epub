# Chapter 2: Includes From a Subfolder

This chapter lives in `src/02_subdir/`, so every include path is resolved
relative to this file's own directory (`../../listings/...`).

## `{{ # include}}` — whole file

```console
{{#include ../../listings/ch03-common-programming-concepts/no-listing-01-variables-are-immutable/output.txt}}
```

## `{{ # include}}` — inclusive line range

```text
{{#include ../../listings/ch05-using-structs-to-structure-related-data/listing-05-11/output.txt:9:10}}
```

## `{{ # rustdoc_include}}` — whole file

```rust
{{#rustdoc_include ../../listings/ch11-writing-automated-tests/listing-11-01/src/lib.rs}}
```

## `{{ # rustdoc_include}}` — anchored section

```rust
{{#rustdoc_include ../../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs:here}}
```

## `{{ # playground}}` — editable

{{#playground ../../listings/ch06-enums-and-pattern-matching/listing-06-02/src/main.rs editable}}