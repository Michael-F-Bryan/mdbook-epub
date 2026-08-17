Nested include example: this file is itself pulled in via `{{#include}}`, and it
contains another `{{#include}}` which must be expanded recursively relative to
this file's own directory.

```console
{{#include ../ch03-common-programming-concepts/no-listing-01-variables-are-immutable/output.txt}}
```
