#!/bin/bash
# Generate small test fixtures for computer use scenarios.
# Run once before running the harness for the first time.
#
# Requirements: ffmpeg, imagemagick (brew install ffmpeg imagemagick)

set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Generating fixtures in $DIR..."

# PNG: 100x100 solid blue square
magick -size 100x100 xc:blue "$DIR/test.png"
echo "  test.png"

# JPEG: same source, different format
magick -size 100x100 xc:tomato "$DIR/test.jpg"
echo "  test.jpg"

# MP4: 3-second 320x240 silent blue video
ffmpeg -f lavfi -i color=c=blue:size=320x240:rate=30 -t 3 -y "$DIR/test.mp4" 2>/dev/null
echo "  test.mp4"

# WAV: 2-second 440Hz sine wave, mono
ffmpeg -f lavfi -i sine=frequency=440:duration=2 -ar 44100 -y "$DIR/test.wav" 2>/dev/null
echo "  test.wav"

# CSV
cat > "$DIR/test.csv" <<'EOF'
name,age,city
Alice,30,New York
Bob,25,London
Carol,35,Sydney
Dave,28,Tokyo
EOF
echo "  test.csv"

# JSON
cat > "$DIR/test.json" <<'EOF'
{"name":"test","value":42,"items":[1,2,3],"nested":{"key":"val"}}
EOF
echo "  test.json"

# ZIP: contains a single text file
echo "hello from zip" > /tmp/cu_fixture_content.txt
zip -j "$DIR/test.zip" /tmp/cu_fixture_content.txt >/dev/null
rm /tmp/cu_fixture_content.txt
echo "  test.zip"

echo ""
echo "Done. $(ls "$DIR" | grep -c '\.' ) files generated."
