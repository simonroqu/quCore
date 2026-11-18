#!/bin/sh
# Android-Bilder einer Qu-App aus ihrem Desktop-Icon (SVG, Pixel-Art mit viewBox 48×48 oder 32×32) erzeugen: App-Icon 192×192 (4-fach, Pixel
# bleiben scharf) und TV-Banner 320×180 (Icon 2-fach + Titel auf dunklem Grund). Nur nach Icon-Änderungen von Hand
# laufen lassen, die PNGs liegen im Repo (Bauen braucht dann kein rsvg-convert/ImageMagick).
# Aufruf aus dem App-Ordner: sh <qu-core>/android/make-icons.sh data/<id>.svg <Titel>
set -e
svg=$1
title=$2
res=android/app/src/main/res
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

rsvg-convert -w 192 -h 192 "$svg" -o "$res/mipmap-xxxhdpi/ic_launcher.png"
rsvg-convert -w 96 -h 96 "$svg" -o "$tmp/icon.png"
# Hintergrund wie das bisherige Banner (qu-core DEFAULT_COLORS „Last Horizon“), Schrift hell.
magick -size 320x180 xc:'#1e1e24' \
  "$tmp/icon.png" -gravity west -geometry +24+0 -composite \
  -font "$(fc-match -f '%{file}' 'sans:bold')" -pointsize 38 -fill '#e8e6e3' \
  -gravity west -annotate +136+0 "$title" \
  "$res/drawable-xhdpi/banner.png"
echo "Neu: $res/mipmap-xxxhdpi/ic_launcher.png, $res/drawable-xhdpi/banner.png"
