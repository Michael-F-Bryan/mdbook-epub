# Chapter 1

Epub3 footnotes [^example] with back-references can be enabled in `book.toml`.

``` toml
[output.epub]
epub-version = 3
footnote-backrefs = true
```

[^example]: This footnote should include a link back to the referencing paragraph. The footnote definition is displayed
as a pop-up or a bottom panel in ebook readers such as Calibre on desktop, ReadEra and Moon+ Reader on Android, Kindle
Paperwhite, iBooks, KOReader, ReMarkable 2, Onyx devices.
