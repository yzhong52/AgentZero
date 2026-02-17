#!/usr/bin/env python3
"""Strip a saved webpage down to just the parts the parser needs:
title, meta tags, and JSON-LD blocks.

Usage:
  python3 strip.py input.html output.html
  python3 strip.py input.html          # writes input.stripped.html
"""

import re
import sys
import os

def strip(src: str, dst: str):
    with open(src, encoding="utf-8") as f:
        content = f.read()

    title = re.search(r'<title[^>]*>.*?</title>', content, re.DOTALL)
    metas = re.findall(r'<meta[^>]+>', content)
    json_lds = re.findall(r'<script[^>]+application/ld\+json[^>]*>.*?</script>', content, re.DOTALL)

    parts = ['<!DOCTYPE html>', '<html>', '<head>']
    if title:
        parts.append(title.group(0))
    parts.extend(metas)
    parts.extend(json_lds)
    parts.extend(['</head>', '<body></body>', '</html>'])

    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))

    print(f"{os.path.getsize(src):>10,} bytes  {src}")
    print(f"{os.path.getsize(dst):>10,} bytes  {dst}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    src = sys.argv[1]
    dst = sys.argv[2] if len(sys.argv) > 2 else os.path.splitext(src)[0] + ".stripped.html"
    strip(src, dst)
