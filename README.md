# sandbox-links

A library for working with [Try It Online](https://tio.run/) and
[Attempt This Online](https://ato.pxeger.com/) code share links.

It can successfully decode and re-encode [all ATO links](tests/ato_links.txt)
found in the wild on Code Golf and the Internet Archive. For 96% of those, even
the recompressed data is byte-equivalent.
