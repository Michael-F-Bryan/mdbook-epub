# MDBook EPUB Backend

An experimental backend to `mdbook` which will render your document as an `EPUB`
file, suitable for viewing on e-readers and other similar devices.


## Getting Started

The support for alternative `mdbook` backends is still very much in the 
experimental phase, so getting everything working isn't as simple as a 
`cargo install mdbook-epub`.

First you'll need to install a patched version of `mdbook`.

```
$ cargo install --git https://github.com/Michael-F-Bryan/mdbook --branch alternate_backends
```

Then you'll need to install the `EPUB` backend (a program called `mdbook-epub`).

```
$ cargo install --git https://github.com/Michael-F-Bryan/mdbook-epub
```

Next you need to let `mdbook` know to use the alternate renderer by updating 
your `book.toml` file. This is done by simply adding an empty `output.epub` 
table.

```diff
[book]
title = "mdBook Documentation"
description = "Create book from markdown files. Like Gitbook but implemented in Rust"
author = "Mathieu David"

[output.html]
mathjax-support = true

+ [output.epub]
```

Now everything is set up, just run `mdbook` as normal and the output directory 
(`book/` by default) should now contain an extra file called 
`mdBook Documentation.epub` (substituting in whatever your book's title is).


## Features

The following features are completed, or planned:

- [x] Make a valid `EPUB` file with the bare chapter contents
- [x] Generate a basic TOC
- [ ] Include a default CSS stylesheet
- [ ] Include user-defined stylesheets and themes


## Contributing

This backend is still very much in the development phase and as such a large 
number of features are missing. If you think of something you'd like please 
create an issue on the [issue tracker]!


[issue tracker]: https://github.com/Michael-F-Bryan/mdbook-epub/issues