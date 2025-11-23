#!/usr/bin/env bash

p=$(pwd)

if [[ "$p" == *"/typ2anki-rust" ]]; then
    echo "Running releaser-before.sh in typ2anki-rust/ ..."
else
    echo "This script must be run from within the typ2anki-rust/ directory"
    exit 1
fi

cd ..
./bundle-ankiaddon.sh || touch ./typ2anki.ankiaddon
cd "$p"
mv ../typ2anki.ankiaddon ./

FILE=".goreleaser.yaml"
PROCESSED_FILE=".goreleaser-processed.yaml"

if [ -z "$TARGET" ]; then
    export TARGET=$(rustc -vV | grep host | cut -d ' ' -f2)
    echo "Inferred TARGET=$TARGET"
elif [ "$TARGET" = "all" ]; then
    echo "Using all targets as TARGET=all"
    cp "$FILE" "$PROCESSED_FILE"
    exit 0
fi

if ! command -v yq &>/dev/null; then
    awk -v tgt="$TARGET" '
  # Detect start of the build block we care about
  /^ *- id: typ2anki/ { in_build=1 }
  
  # Detect targets section
  in_build && /^ *targets:/ {
    print $0       # print "targets:" line
    print "         - " tgt  # replace all items with just our target
    skip=1
    next
  }

  # Skip old target lines (lines starting with spaces + dash) if we are inside targets
  skip && /^[[:space:]]*-/ { next }

  # Stop skipping when indentation decreases or next top-level key
  skip && /^[^ ]/ { skip=0 }

  { print }
' "$FILE" >"$PROCESSED_FILE"
else
    yq ".builds[0].targets = [\"$TARGET\"]" ./.goreleaser.yaml >"$PROCESSED_FILE"
fi
echo "Generated $PROCESSED_FILE with TARGET=$TARGET"
