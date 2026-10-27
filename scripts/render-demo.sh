#!/usr/bin/env bash
set -euo pipefail

# Terminal-window rendering approach adapted from bwestlin/gh-activity-report (MIT).
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
output_dir="$repo_root/assets"
demo_tmp=$(mktemp -d)

cleanup() {
  find "$demo_tmp" -type f -delete
  rmdir "$demo_tmp"
}
trap cleanup EXIT

font=/usr/share/fonts/truetype/firacode/FiraCode-Regular.ttf
if [[ ! -f "$font" ]]; then
  font=DejaVu-Sans-Mono
fi

mkdir -p "$output_dir"

terminal_frame() {
  local name=$1
  shift
  local -a draw=(
    -size 1200x720 "xc:none"
    -stroke "#30363d" -strokewidth 2 -fill "#0d1117"
    -draw "roundrectangle 1,1 1198,718 14,14"
    -stroke none -fill "#161b22"
    -draw "roundrectangle 2,2 1197,47 13,13 rectangle 2,28 1197,48"
    -fill "#ff5f56" -draw "circle 25,24 31,24"
    -fill "#ffbd2e" -draw "circle 46,24 52,24"
    -fill "#27c93f" -draw "circle 67,24 73,24"
    -font "$font" -pointsize 16 -fill "#8b949e" -gravity NorthWest
    -annotate +516+15 "gfind — Git search"
    "$@"
    -pointsize 13 -fill "#484f58" -annotate +1064+696 "mock data"
  )

  magick "${draw[@]}" "$demo_tmp/$name.png"
}

render_repos_frame() {
  local frame=$1
  local cursor=${2:-true}
  local -a content=()

  if ((frame >= 1)); then
    content+=(
      -pointsize 20 -fill "#e6edf3" -annotate +34+76 '$ gfind repos'
    )
  fi
  if ((frame >= 2)); then
    content+=(
      -pointsize 20 -fill "#56d4dd" -annotate +34+122 'clients/storefront'
    )
  fi
  if ((frame >= 3)); then
    content+=(
      -fill "#56d4dd" -annotate +34+158 'platform/api'
      -annotate +34+194 'tools/release-helper'
      -annotate +34+230 'docs/handbook'
    )
  fi
  if [[ "$cursor" == true ]]; then
    local cursor_y=91
    if ((frame >= 3)); then
      cursor_y=257
    elif ((frame >= 2)); then
      cursor_y=149
    fi
    content+=( -fill "#e6edf3" -draw "rectangle 34,$cursor_y 46,$((cursor_y + 3))" )
  fi

  terminal_frame "repos-$frame-$cursor" "${content[@]}"
}

render_branch_frame() {
  terminal_frame branch \
    -pointsize 20 -fill "#e6edf3" \
    -annotate +34+76 '$ gfind -p ~/src -p ~/work branch login --all --short-commit-hash' \
    -fill "#56d4dd" -annotate +34+126 'src/clients/storefront' \
    -pointsize 18 -fill "#7d8590" -annotate +54+164 'local:' \
    -fill "#56d4dd" -annotate +134+164 'feature/' \
    -undercolor "#30363d" -annotate +221+164 'login' \
    -undercolor none \
    -fill "#7d8590" -annotate +74+198 'commit:' \
    -fill "#e5c07b" -annotate +164+198 '9f31c7a' \
    -fill "#7d8590" -annotate +74+232 'status:' \
    -fill "#e5c07b" -annotate +164+232 'ahead 2 of origin/feature/login @ 8c4ad21' \
    -fill "#7d8590" -annotate +74+266 'current:' \
    -fill "#98c379" -annotate +174+266 'yes' \
    -pointsize 20 -fill "#56d4dd" -annotate +34+320 'work/platform/api' \
    -pointsize 18 -fill "#7d8590" -annotate +54+358 'remote:' \
    -fill "#56d4dd" -annotate +144+358 'origin/feature/' \
    -undercolor "#30363d" -annotate +307+358 'login' \
    -undercolor none \
    -fill "#7d8590" -annotate +74+392 'commit:' \
    -fill "#e5c07b" -annotate +164+392 '2dc38b4' \
    -fill "#e6edf3" -draw "rectangle 34,425 46,428"
}

render_status_frame() {
  terminal_frame status \
    -pointsize 20 -fill "#e6edf3" -annotate +34+76 '$ gfind status' \
    -fill "#56d4dd" -annotate +34+130 'clients/storefront' \
    -fill "#e6edf3" -annotate +260+130 '(' \
    -fill "#e06c75" -annotate +273+130 'dirty' \
    -fill "#e6edf3" -annotate +336+130 ', ' \
    -fill "#e5c07b" -annotate +360+130 'ahead 2' \
    -fill "#e6edf3" -annotate +440+130 ')' \
    -fill "#56d4dd" -annotate +34+170 'platform/api' \
    -fill "#e6edf3" -annotate +200+170 '(' \
    -fill "#e5c07b" -annotate +213+170 'ahead 1' \
    -fill "#e6edf3" -annotate +293+170 ')' \
    -fill "#56d4dd" -annotate +34+210 'docs/handbook' \
    -fill "#e6edf3" -annotate +202+210 '(' \
    -fill "#e06c75" -annotate +215+210 'dirty' \
    -fill "#e6edf3" -annotate +278+210 ')' \
    -fill "#e6edf3" -draw "rectangle 34,243 46,246"
}

render_stash_frame() {
  terminal_frame stash \
    -pointsize 20 -fill "#e6edf3" -annotate +34+76 '$ gfind -p ~/src -p ~/work stash login' \
    -fill "#56d4dd" -annotate +34+130 'src/clients/storefront' \
    -pointsize 18 -fill "#c678dd" -annotate +54+168 'stash@{0}:' \
    -fill "#e6edf3" -annotate +183+168 'On feature/' \
    -undercolor "#30363d" -annotate +302+168 'login' \
    -undercolor none -annotate +356+168 ': WIP ' \
    -undercolor "#30363d" -annotate +421+168 'login' \
    -undercolor none -annotate +486+168 'redirect' \
    -pointsize 20 -fill "#56d4dd" -annotate +34+222 'work/platform/api' \
    -pointsize 18 -fill "#c678dd" -annotate +54+260 'stash@{1}:' \
    -fill "#e6edf3" -annotate +183+260 'On main: ' \
    -undercolor "#30363d" -annotate +280+260 'login' \
    -undercolor none -annotate +345+260 'rate-limit experiment' \
    -fill "#e6edf3" -draw "rectangle 34,293 46,296"
}

render_remote_frame() {
  terminal_frame remote \
    -pointsize 20 -fill "#e6edf3" -annotate +34+76 '$ gfind repos acme/widget' \
    -fill "#56d4dd" -annotate +34+130 'vendor/widget-fork' \
    -pointsize 17 -fill "#7d8590" -annotate +54+168 'remote:' \
    -fill "#e6edf3" -annotate +134+168 'origin  https://github.com/' \
    -fill "#56d4dd" -undercolor "#30363d" -annotate +421+168 'acme/widget' \
    -undercolor none -fill "#e6edf3" -annotate +555+168 '.git (fetch)' \
    -fill "#7d8590" -annotate +54+202 'remote:' \
    -fill "#e6edf3" -annotate +134+202 'origin  https://github.com/' \
    -fill "#56d4dd" -undercolor "#30363d" -annotate +421+202 'acme/widget' \
    -undercolor none -fill "#e6edf3" -annotate +555+202 '.git (push)' \
    -fill "#e6edf3" -draw "rectangle 34,235 46,238"
}

render_repos_frame 0 true
render_repos_frame 1 true
render_repos_frame 2 true
render_repos_frame 3 true
render_repos_frame 3 false
render_branch_frame
render_status_frame
render_stash_frame
render_remote_frame

magick \
  -delay 30 "$demo_tmp/repos-0-true.png" \
  -delay 75 "$demo_tmp/repos-1-true.png" \
  -delay 55 "$demo_tmp/repos-2-true.png" \
  -delay 210 "$demo_tmp/repos-3-true.png" \
  -delay 40 "$demo_tmp/repos-3-false.png" \
  -delay 40 "$demo_tmp/repos-3-true.png" \
  -delay 260 "$demo_tmp/branch.png" \
  -delay 220 "$demo_tmp/status.png" \
  -delay 220 "$demo_tmp/stash.png" \
  -delay 240 "$demo_tmp/remote.png" \
  -loop 0 -layers Optimize "$output_dir/gfind-demo.gif"

echo "Rendered $output_dir/gfind-demo.gif"
