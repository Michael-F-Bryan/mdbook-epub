 # MDBook EPUB Backend

 - [![Build status](https://ci.appveyor.com/api/projects/status/94a37o6ffioapgoo/branch/master?svg=true)](https://ci.appveyor.com/project/blandger/mdbook-epub/branch/master)
 - [![Rust](https://github.com/blandger/mdbook-epub/actions/workflows/rust.yml/badge.svg)](https://github.com/blandger/mdbook-epub/actions/workflows/rust.yml)

[**(Rendered Docs)**](https://michael-f-bryan.github.io/mdbook-epub/)

An experimental backend to `mdbook` which will render your document as an `EPUB`
file, suitable for viewing on e-readers and other similar devices.

> **WARNING:** Not yet production ready. May eat your laundry.

> **Note:** At the moment the default stylesheet is quite bare bones, serving 
  mainly to reset the styling used on various devices back to a known default.
  This default isn't overly pretty, so you may want to include your own
  stylesheets.


## Getting Started

Before you can use the EPUB backend, you'll need to actually install it:

```
$ cargo install mdbook-epub
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
(`book/epub/` by default) should now contain an extra file called 
`mdBook Documentation.epub` (substituting in whatever your book's title is).

The `mdbook-epub` executable can be run in "standalone" mode. This is where
the backend can be used without needing to be called by `mdbook`, useful if
you only want to render the EPUB document.

```
$ mdbook-epub -s ./path/to/book/dir
$ mdbook-epub --standalone ./path/to/book/dir
```


## Configuration

Configuration is fairly bare bones at the moment.

Recognized options:

`additional-css`: A list of paths to CSS stylesheets to include.

`use-default-css`: Controls whether to include the default stylesheet.

`cover-image`: A path to a cover image file for the ebook.

`additional-resources`: A list of path to files which should be added to the
EPUB, such as typefaces. They will be added with path `OEBPS/<filename>`.

`no-section-label`: In the contents list, don't prefix the chapter title with
its section number.

`curly-quotes`: Enable converting straight quotes `'x'` and `"x"` to `‘x’` and
`“x”` (aka *smart quotes*).

```toml
[output.epub]
additional-css = ["./path/to/main.css"]
use-default-css = false
cover-image = "ebook-cover.png"
additional-resources = ["./assets/Open-Sans-Regular.ttf"]
no-section-label = true
curly-quotes = true
```

## Logging, seeing progress

In order to enable logging to the screen you need to set the `RUST_LOG` environment variable to `debug` or `info`.

On Linux and macOS this can be done in the following way:

```
RUST_LOG=debug  mdbook-epub
```

On Windows CMD you need to set it on a separate line:

```
set RUST_LOG=debug
mdbook-epub
```


## Planned Features

The following features are planned (a checked box indicates it's complete). This
list is by no means complete, so feature requests are most welcome!

- [x] Make a valid `EPUB` file with the bare chapter contents
- [x] Generate a basic TOC
- [x] Nested chapters - currently they're all inserted at the top level
- [x] Include a default CSS stylesheet ([master.css])
   - [X] Actually make that stylesheet pretty enough for human consumption
- [x] Include user-defined stylesheets and themes
- [ ] Allow users to tweak the generated page by providing their own template
- [x] Ensure the generated document is viewable on the following platforms
  - [x] Amazon Kindle
  - [x] Sony PRS-T3


## Contributing

This backend is still very much in the development phase and as such a large 
number of features are missing. If you think of something you'd like please 
create an issue on the [issue tracker]!


[issue tracker]: https://github.com/Michael-F-Bryan/mdbook-epub/issues
[master.css]: https://github.com/Michael-F-Bryan/mdbook-epub/blob/master/src/master.css
