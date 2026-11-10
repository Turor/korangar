# How to use a different font

It is possible to use a different font in Korangar. This may be needed, if you want to support languages that are not
included in the distributed font file. Currently, we use "Noto Sans", which includes all glyphs for the Latin, Cyrillic,
and Greek alphabets. Hangul and Japanese fall back to the bundled OFL faces
`NotoSansKR.otf` and `NotoSansJP.otf` (Noto Sans CJK region subsets). Those files
are also compiled into the client, so a missing GRF copy still renders CJK.
We support having fallback fonts, so if for example the primary font doesn't have a specific
glyph, the fallback font is tried. Multiple fallback fonts are supported.

1. Copy the font file into the `archive/data/font` folder (`.ttf` or `.otf`).
2. Add the name of the font file without the extension to the font list passed to `FontLoader::new` in the
   `korangar/src/lib.rs` file.
