#!/bin/sh
# Append a JSON progress event to progress.jsonl. Usage:
#   progress.sh <stage> <event> [k=v ...]
# Strings must be quoted (name='"foo"'); ints/bools must not be (i=1).
set -eu
PROJECT="${PROJECT:-/Users/akeles/Programming/projects/PbtBenchmark/faultloc/workloads/Rust/regex-syntax}"
stage="$1"; event="$2"; shift 2
extras=""
for kv in "$@"; do extras="${extras},\"${kv%%=*}\":${kv#*=}"; done
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
line="{\"ts\":\"${ts}\",\"stage\":\"${stage}\",\"event\":\"${event}\"${extras}}"
printf '%s\n' "$line" | tee -a "$PROJECT/progress.jsonl"
