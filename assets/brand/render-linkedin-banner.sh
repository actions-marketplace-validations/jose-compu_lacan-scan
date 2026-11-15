#!/usr/bin/env bash
# Render a filled LinkedIn release banner PNG from linkedin-banner.template.svg.
# Usage (from repo root):
#   VERSION=v7.2.2 HEADLINE='...' BENEFIT_1='...' BENEFIT_2='...' BENEFIT_3='...' \
#     ACTION_REF='jose-compu/lacan-scan@v7' VERSION_PIN='7.2.2' \
#     ./assets/brand/render-linkedin-banner.sh
#
# Writes assets/brand/linkedin-banners/v${VERSION}.svg and .png (gitignored).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BRAND="$ROOT/assets/brand"
OUT_DIR="$BRAND/linkedin-banners"
TEMPLATE="$BRAND/linkedin-banner.template.svg"
LOGO="$BRAND/logo-linkedin.png"

: "${VERSION:?set VERSION, e.g. v7.2.2}"
: "${HEADLINE:?set HEADLINE}"
: "${BENEFIT_1:?set BENEFIT_1}"
: "${BENEFIT_2:?set BENEFIT_2}"
: "${BENEFIT_3:?set BENEFIT_3}"
: "${ACTION_REF:=jose-compu/lacan-scan@v7}"
: "${VERSION_PIN:=7.2.2}"
: "${WIDTH:=1200}"
: "${HEIGHT:=480}"
: "${LOGO_W:=188}"
: "${LOGO_H:=232}"
: "${LOGO_MARGIN_RIGHT:=40}"

ver_file="${VERSION#v}"
out_base="$OUT_DIR/v${ver_file}"
mkdir -p "$OUT_DIR"

svg_out="${out_base}.svg"
png_out="${out_base}.png"

python3 - "$TEMPLATE" "$svg_out" <<'PY'
import html
import os
import re
import sys

template_path, out_path = sys.argv[1], sys.argv[2]
body = open(template_path, encoding="utf-8").read()

def esc(text: str) -> str:
    return html.escape(text, quote=False)

def headline_markup(text: str, limit: int = 44) -> str:
    if len(text) <= limit:
        return esc(text)
    cut = text.rfind(" ", 0, limit + 1)
    if cut <= 0:
        cut = limit
    line1 = esc(text[:cut].rstrip())
    line2 = esc(text[cut:].lstrip())
    return (
        f'<tspan x="48" dy="0">{line1}</tspan>'
        f'<tspan x="48" dy="40">{line2}</tspan>'
    )

headline = headline_markup(os.environ["HEADLINE"])
replacements = {
    "__VERSION__": esc(os.environ["VERSION"]),
    "__HEADLINE__": headline,
    "__BENEFIT_1__": esc(os.environ["BENEFIT_1"]),
    "__BENEFIT_2__": esc(os.environ["BENEFIT_2"]),
    "__BENEFIT_3__": esc(os.environ["BENEFIT_3"]),
    "__ACTION_REF__": esc(os.environ["ACTION_REF"]),
    "__VERSION_PIN__": esc(os.environ["VERSION_PIN"]),
}

headline_open = (
    '<text x="48" y="142" fill="#24382f" xml:space="preserve"\n'
    "        font-family=\"Georgia, 'DejaVu Serif', 'Noto Serif', 'Times New Roman', serif\"\n"
    '        font-size="34">__HEADLINE__</text>'
)
headline_filled = headline_open.replace("__HEADLINE__", headline, 1)
if headline.startswith("<tspan"):
    headline_filled = headline_filled.replace(' xml:space="preserve"', "", 1)

body = body.replace(headline_open, headline_filled, 1)

plain_headline = esc(os.environ["HEADLINE"])

subs = [
    (r"(<title id=\"banner-title\">Lacan Scan )__VERSION__(</title>)", r"\1" + replacements["__VERSION__"] + r"\2"),
    (
        r"<desc id=\"banner-desc\">__HEADLINE__\. GitHub Action __ACTION_REF__\.</desc>",
        f"<desc id=\"banner-desc\">{plain_headline}. GitHub Action {replacements['__ACTION_REF__']}.</desc>",
    ),
    (r"(<text x=\"48\" y=\"72\"[^>]+>LACAN SCAN )__VERSION__(</text>)", r"\1" + replacements["__VERSION__"] + r"\2"),
    (r">-  __BENEFIT_1__</text>", f">-  {replacements['__BENEFIT_1__']}</text>"),
    (r">-  __BENEFIT_2__</text>", f">-  {replacements['__BENEFIT_2__']}</text>"),
    (r">-  __BENEFIT_3__</text>", f">-  {replacements['__BENEFIT_3__']}</text>"),
    (r"(<text x=\"48\" y=\"372\"[^>]+>)__ACTION_REF__(</text>)", r"\1" + replacements["__ACTION_REF__"] + r"\2"),
    (r">- uses: __ACTION_REF__</text>", f">- uses: {replacements['__ACTION_REF__']}</text>"),
    (
        r"  with: version: __VERSION_PIN__  fail-on: HIGH  output: sarif</text>",
        f"  with: version: {replacements['__VERSION_PIN__']}  fail-on: HIGH  output: sarif</text>",
    ),
]
for pattern, repl in subs:
    body, n = re.subn(pattern, repl, body, count=1)
    if n != 1:
        raise SystemExit(f"placeholder not found for pattern: {pattern}")

open(out_path, "w", encoding="utf-8").write(body)
PY

if command -v rsvg-convert >/dev/null 2>&1; then
  rsvg-convert -w "$WIDTH" -h "$HEIGHT" -o "$png_out" "$svg_out"
elif command -v resvg >/dev/null 2>&1; then
  resvg -w "$WIDTH" -h "$HEIGHT" "$svg_out" "$png_out"
else
  echo "need rsvg-convert or resvg on PATH" >&2
  exit 1
fi

if command -v magick >/dev/null 2>&1; then
  magick "$png_out" \
    \( "$LOGO" -resize "${LOGO_W}x${LOGO_H}" \) \
    -gravity East -geometry "+${LOGO_MARGIN_RIGHT}+0" -compose over -composite \
    "$png_out"
elif command -v convert >/dev/null 2>&1; then
  convert "$png_out" \
    \( "$LOGO" -resize "${LOGO_W}x${LOGO_H}" \) \
    -gravity East -geometry "+${LOGO_MARGIN_RIGHT}+0" -compose over -composite \
    "$png_out"
else
  python3 - "$png_out" "$LOGO" "$LOGO_W" "$LOGO_H" "$LOGO_MARGIN_RIGHT" <<'PY'
from PIL import Image
import sys
png, logo, lw, lh, margin = sys.argv[1:6]
lw, lh, margin = int(lw), int(lh), int(margin)
base = Image.open(png).convert("RGBA")
mark = Image.open(logo).convert("RGBA").resize((lw, lh), Image.Resampling.LANCZOS)
x = base.width - margin - lw
y = (base.height - lh) // 2
base.alpha_composite(mark, (x, y))
base.save(png)
PY
fi

echo "$png_out"