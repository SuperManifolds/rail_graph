#!/bin/bash

# Generate asset manifest for service worker
# This script scans the dist directory and creates a JSON manifest of all assets to cache

DIST_DIR="dist"
MANIFEST_FILE="$DIST_DIR/asset-manifest.json"

echo "Generating asset manifest..."

# Start JSON array
echo '{' > "$MANIFEST_FILE"
echo '  "version": "1",' >> "$MANIFEST_FILE"
echo '  "assets": [' >> "$MANIFEST_FILE"

# Find all .css, .js, .wasm files in dist (not in subdirectories)
# Also include index.html
ASSETS=$(find "$DIST_DIR" -maxdepth 1 -type f \( -name "*.css" -o -name "*.js" -o -name "*.wasm" -o -name "index.html" \) | sed "s|^$DIST_DIR||" | sort)

# Find all files in dist/static recursively
STATIC_ASSETS=$(find "$DIST_DIR/static" -type f 2>/dev/null | sed "s|^$DIST_DIR||" | sort)

# Combine all assets
ALL_ASSETS=$(printf "%s\n%s\n" "$ASSETS" "$STATIC_ASSETS" | grep -v '^$')

# Build JSON array
FIRST=true
echo "$ALL_ASSETS" | while IFS= read -r asset; do
    if [ -n "$asset" ]; then
        if [ "$FIRST" = true ]; then
            echo -n "    \"$asset\"" >> "$MANIFEST_FILE"
            FIRST=false
        else
            echo "," >> "$MANIFEST_FILE"
            echo -n "    \"$asset\"" >> "$MANIFEST_FILE"
        fi
    fi
done

# Close JSON array and object
echo "" >> "$MANIFEST_FILE"
echo '  ]' >> "$MANIFEST_FILE"
echo '}' >> "$MANIFEST_FILE"

echo "Asset manifest generated at $MANIFEST_FILE"
echo "Total assets: $(echo "$ALL_ASSETS" | grep -c .)"
