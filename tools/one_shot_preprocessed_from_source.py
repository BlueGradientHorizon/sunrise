#!/usr/bin/env python3
"""One-shot: rewrite the test helpers that build a `Preprocessed` by struct literal (code from
`code`, one origin per line of file 0, a `files` list of one name, a `version`) into
`Preprocessed::from_source(file, code)` plus the version, now that its fields are private."""
import re
import sys

LITERAL = re.compile(
    r"^    Preprocessed \{\n        code: code\.into\(\),\n(?P<body>.*?)^    \}\n",
    re.S | re.M,
)


def rewrite(match):
    body = match.group("body")
    file = re.search(r'files: vec!\["([^"]+)"\.into\(\)\],', body).group(1)
    version = re.search(r"^        version: (.*),\n", body, re.M).group(1)
    return (
        f'    let mut pp = Preprocessed::from_source("{file}", code);\n'
        f"    pp.version = {version};\n"
        "    pp\n"
    )


for path in sys.argv[1:]:
    text = open(path).read()
    text, n = LITERAL.subn(rewrite, text)
    if n and len(re.findall(r"\bOrigin\b", text)) == 1:
        text = re.sub(r"\bOrigin, ", "", text, count=1)
    open(path, "w").write(text)
    print(path, n)
