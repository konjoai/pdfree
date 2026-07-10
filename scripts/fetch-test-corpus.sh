#!/usr/bin/env bash
# Fetch the e2e test corpus: a spread of real, public-domain government forms
# (U.S. federal works are public domain under 17 U.S.C. § 105) covering the
# scenarios PDFree has to get right — AcroForm fields, ruled-grid tables,
# checkboxes, fill-in-the-blank underlines, and single- vs. multi-page docs.
#
# Same policy as scripts/fetch-pdfium.sh: fetched at need, never committed
# (tests/corpus/*.pdf is gitignored). CI runs this before the accuracy report.
#
# Idempotent: a form already present with the right %PDF header is left alone.
# A URL that fails (site rate-limit, moved form revision) is warned about and
# skipped, not fatal — the accuracy harness simply reports on whatever landed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS_DIR="$ROOT/tests/corpus"
mkdir -p "$CORPUS_DIR"

# name|url|what it exercises
FORMS=(
  "irs_w9|https://www.irs.gov/pub/irs-pdf/fw9.pdf|short AcroForm, text fields"
  "irs_w4|https://www.irs.gov/pub/irs-pdf/fw4.pdf|AcroForm + worksheet tables"
  "irs_1040|https://www.irs.gov/pub/irs-pdf/f1040.pdf|multi-page, dense ruled boxes"
  "irs_1099misc|https://www.irs.gov/pub/irs-pdf/f1099msc.pdf|boxed grid, copies"
  "irs_w2|https://www.irs.gov/pub/irs-pdf/fw2.pdf|numbered box grid"
  "irs_8949|https://www.irs.gov/pub/irs-pdf/f8949.pdf|wide transaction table"
  "irs_941|https://www.irs.gov/pub/irs-pdf/f941.pdf|multi-page, checkboxes + lines"
  "uscis_i9|https://www.uscis.gov/sites/default/files/document/forms/i-9.pdf|many fields + checkboxes"
  "uscis_g1145|https://www.uscis.gov/sites/default/files/document/forms/g-1145.pdf|small, sparse fields"
  "uscis_i765|https://www.uscis.gov/sites/default/files/document/forms/i-765.pdf|long AcroForm"
  "uscis_n400|https://www.uscis.gov/sites/default/files/document/forms/n-400.pdf|large multi-page"
  "uscis_i130|https://www.uscis.gov/sites/default/files/document/forms/i-130.pdf|multi-page, mixed layout"
)

MANIFEST="$CORPUS_DIR/manifest.tsv"
printf "name\tsha256\tbytes\tnotes\turl\n" > "$MANIFEST"

ok=0
for entry in "${FORMS[@]}"; do
  IFS='|' read -r name url notes <<< "$entry"
  out="$CORPUS_DIR/$name.pdf"

  if [ ! -s "$out" ] || ! head -c 5 "$out" | grep -q "%PDF"; then
    echo "↓ $name"
    if ! curl -fsSL --max-time 60 -o "$out.tmp" "$url" 2>/dev/null; then
      echo "  ⚠ skip $name — download failed ($url)"
      rm -f "$out.tmp"
      continue
    fi
    if ! head -c 5 "$out.tmp" | grep -q "%PDF"; then
      echo "  ⚠ skip $name — not a PDF (got $(head -c 5 "$out.tmp" | tr -d '\0'))"
      rm -f "$out.tmp"
      continue
    fi
    mv "$out.tmp" "$out"
  else
    echo "✓ $name (cached)"
  fi

  sha=$(shasum -a 256 "$out" | awk '{print $1}')
  bytes=$(wc -c < "$out" | tr -d ' ')
  printf "%s\t%s\t%s\t%s\t%s\n" "$name" "$sha" "$bytes" "$notes" "$url" >> "$MANIFEST"
  ok=$((ok + 1))
done

echo ""
echo "Corpus ready: $ok/${#FORMS[@]} forms in $CORPUS_DIR"
echo "Manifest: $MANIFEST"
