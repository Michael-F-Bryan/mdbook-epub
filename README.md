 # MDBook EPUB Backend

[![Build Status](https://travis-ci.org/Michael-F-Bryan/mdbook-epub.svg?branch=master)](https://travis-ci.org/Michael-F-Bryan/mdbook-epub)

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
$ mdbook-epub --standalone ./path/to/book/dir
```


## Configuration

Configuration is fairly bare bones at the moment. All you can do is add 
additional CSS files and disable the default stylesheet.


```toml
[output.epub]
additional-css = ["./path/to/main.css"]
use-default-css = false
```


## Planned Features

The following features are planned (a checked box indicates it's complete). This
list is by no means complete, so feature requests are most welcome!

- [x] Make a valid `EPUB` file with the bare chapter contents
- [x] Generate a basic TOC
- [x] Nested chapters - currently they're all inserted at the top level
- [x] Include a default CSS stylesheet ([master.css])
   - [ ] Actually make that stylesheet pretty enough for human consumption
- [x] Include user-defined stylesheets and themes
- [ ] Allow users to tweak the generated page by providing their own template
- [ ] Ensure the generated document is viewable on the following platforms
  - [ ] Amazon Kindle
  - [ ] Sony PRS-T3


## Contributing

This backend is still very much in the development phase and as such a large 
number of features are missing. If you think of something you'd like please 
create an issue on the [issue tracker]!


[issue tracker]: https://github.com/Michael-F-Bryan/mdbook-epub/issues
[master.css]: https://github.com/Michael-F-Bryan/mdbook-epub/blob/master/src/master.css