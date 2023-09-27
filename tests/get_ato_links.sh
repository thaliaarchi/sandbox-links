#!/bin/bash
set -euo pipefail

if [ ! -f QueryResults.csv ]; then
  echo 'Download QueryResults.csv from https://data.stackexchange.com/codegolf/query/1722463' >&2
  exit 1
fi

(
  rg -o 'https://ato.pxeger.com/run[^"<)]+' QueryResults.csv &&
  curl 'https://web.archive.org/web/timemap/?url=ato.pxeger.com/run&collapse=digest&matchType=prefix&output=json&limit=10000' |
    jq -r '.[1:][][2]'
) |
  sed s,https://ato.pxeger.com/run1,https://ato.pxeger.com/run?1, |
  sort | uniq > ato_links.txt
