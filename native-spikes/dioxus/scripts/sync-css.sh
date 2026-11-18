#!/bin/sh
# Concatenates the real Orphyn stylesheets (unmodified) into assets/orphyn.css.
# Only mechanical rewrite: selectors anchored on <html> (theme/backdrop attributes) are anchored on
# the .jr-root wrapper instead, since the Rust app sets those attributes on its root element.
S=src/renderer/src
OUT=native-spikes/dioxus/assets/orphyn.css
: > $OUT
for f in $S/styles/tokens.css $S/styles/base.css $S/styles/typography.css $S/styles/materials.css \
  $S/styles/interface-styles.css $S/design/controls/controls.css $S/design/feedback/feedback.css \
  $S/design/lists/lists.css $S/design/media/media.css $S/design/navigation/navigation.css \
  $S/shell/shell.css $S/features/music/music.css $S/features/music/components/now-playing-layer.css; do
  echo "/* ==== $f ==== */" >> $OUT
  cat $f >> $OUT
done
sed -i -E 's/(^|[ ,{}])html(\[|:is\()/\1.jr-root\2/g' $OUT
wc -c $OUT
