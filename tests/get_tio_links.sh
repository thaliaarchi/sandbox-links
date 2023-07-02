#!/bin/bash
set -euo pipefail

if [ ! -f QueryResults.csv ]; then
  echo 'Download QueryResults.csv from https://data.stackexchange.com/codegolf/query/1766445' >&2
  exit 1
fi

(
  rg -o 'https?://([a-z0-9]+\.)*(tio\.run|tryitonline\.net)[^"'\''<)]+' QueryResults.csv |
    sed 's/&amp;/\&/' &&
  (
    curl 'https://web.archive.org/web/timemap/?url=tio.run&collapse=digest&matchType=prefix&output=json&limit=10000' &&
    curl 'https://web.archive.org/web/timemap/?url=tryitonline.net&collapse=digest&matchType=prefix&output=json&limit=10000'
  ) |
    jq -r '.[1:][][2]'
) |
  sort | uniq > tio_links.txt
