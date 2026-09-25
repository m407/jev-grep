#!/bin/sh
set -eu

dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(git -C "$dir" rev-parse --show-toplevel)
bin=${LG_BIN:-$repo/target/release/lg}
endpoint=${LG_API_URL:-}
model=${LG_MODEL:-}

if [ -z "$endpoint" ] || [ -z "$model" ]; then
  echo 'Set LG_API_URL to an explicitly authorized Laya /v1/systemone endpoint and LG_MODEL to the exact checkpoint/model ID.' >&2
  exit 2
fi
case "$endpoint" in */v1/systemone) ;; *) echo 'LG_API_URL must end in /v1/systemone' >&2; exit 2;; esac
[ -x "$bin" ] || { echo "lg executable not found: $bin (build it separately)" >&2; exit 2; }
command -v curl >/dev/null 2>&1 || { echo 'curl is required for the health preflight' >&2; exit 2; }
health=${endpoint%/v1/systemone}/health
health_json=$(curl --fail --silent --show-error --max-time 5 "$health") || {
  echo "Selected endpoint health probe failed: $health (no inference requests sent)" >&2
  exit 2
}
out=${LG_EXPERIMENT_OUT:-$dir/results}
mkdir -p "$out"
: > "$out/run-index.jsonl"
started=$(date -u +%FT%TZ)
version=$("$bin" --version)
printf 'dataset_sha256='; sha256sum "$dir/dataset.jsonl" | cut -d' ' -f1
printf 'endpoint=%s\nmodel=%s\n' "$endpoint" "$model"
printf 'health_metadata=%s\n' "$health_json"
metadata_probe=$(curl --fail --silent --show-error --max-time 30 -X POST "$endpoint" \
  -H 'Content-Type: application/json' \
  --data "$(jq -cn --arg model "$model" '{state:"synthetic metadata-only identity probe",questions:{match:{type:"noul",instructions:"Evaluate whether this synthetic metadata-only text is an error; return a noul probability"}},model:$model}')") || {
  echo 'Metadata probe failed; no experiment data requests sent.' >&2
  exit 2
}
printf 'metadata_probe_model=%s\nmetadata_probe_routing=%s\n' \
  "$(printf '%s' "$metadata_probe" | jq -c '.model // null')" \
  "$(printf '%s' "$metadata_probe" | jq -c '.routing // null')"
printf 'started_utc=%s\n' "$started"
dataset_sha=$(sha256sum "$dir/dataset.jsonl" | cut -d' ' -f1)
probe_model=$(printf '%s' "$metadata_probe" | jq -c '.model // null')
probe_routing=$(printf '%s' "$metadata_probe" | jq -c '.routing // null')
context_probe_json=null
for id in gradle-stacktrace retry-recovery useful-neighbor independent-errors npm buildah context-edge context-edge-english-512; do
  query=${LG_QUERY:-'Does this log contain an error?'}
  case "$id" in
    context-edge) limit=${LG_EDGE_LIMIT:-128}; threshold=${LG_EDGE_THRESHOLD:-0.5} ;;
    context-edge-english-512) limit=${LG_CONTEXT_EDGE_LIMIT:-16384}; threshold=${LG_CONTEXT_EDGE_THRESHOLD:-0.5} ;;
    *) limit=${LG_MAX_FRAGMENT_BYTES:-16384}; threshold=${LG_THRESHOLD:-0.5} ;;
  esac
  if [ "$id" = context-edge-english-512 ]; then
    prefix=$(jq -r --arg id "$id" 'select(.id==$id) | .text' "$dir/dataset.jsonl")
    repeat_text=$(jq -r --arg id "$id" 'select(.id==$id) | .repeat_text' "$dir/dataset.jsonl")
    repeat_count=$(jq -r --arg id "$id" 'select(.id==$id) | .repeat_count' "$dir/dataset.jsonl")
    printf '%s' "$prefix" > "$out/$id.log"
    i=0
    while [ "$i" -lt "$repeat_count" ]; do
      printf '%s' "$repeat_text" >> "$out/$id.log"
      i=$((i + 1))
    done
    printf '\n' >> "$out/$id.log"
    context_request=$(jq -cn --rawfile state "$out/$id.log" --arg model "$model" --arg query "$query" \
      '{state:$state,questions:{match:{type:"noul",instructions:$query}},model:$model}')
    context_probe_json=$(curl --fail --silent --show-error --max-time 60 -X POST "$endpoint" \
      -H 'Content-Type: application/json' --data-binary "$context_request") || {
      echo 'Near-context real-inference telemetry probe failed.' >&2
      exit 2
    }
    printf '%s\n' "$context_probe_json" > "$out/$id.telemetry.json"
    usage=$(printf '%s' "$context_probe_json" | jq -er '.usage.input_tokens | numbers') || {
      echo 'Endpoint response lacks numeric usage.input_tokens; cannot verify actual context occupancy.' >&2
      exit 2
    }
    target=${LG_CONTEXT_TARGET_TOKENS:-500}
    if [ "$usage" -lt "$target" ]; then
      echo "Near-context probe used $usage tokens, below required target $target of published 512; not a verified edge run." >&2
      exit 2
    fi
    context_probe_json=$(printf '%s' "$context_probe_json" | jq -c '{usage,model,routing}')
  else
    jq -r --arg id "$id" 'select(.id==$id) | .text' "$dir/dataset.jsonl" > "$out/$id.log"
  fi
  start=$(date +%s%N)
  set +e
  "$bin" --api-url "$endpoint" --model "$model" --input-format text --format json \
    --threshold "$threshold" --join-threshold "${LG_JOIN_THRESHOLD:-0.5}" \
    --max-fragment-bytes "$limit" --max-line-bytes "${LG_MAX_LINE_BYTES:-16384}" "$query" "$out/$id.log" \
    >"$out/$id.stdout.jsonl" 2>"$out/$id.stderr"
  code=$?
  set -e
  end=$(date +%s%N)
  calls=$(jq -r --arg id "$id" 'select(.id==$id) | if .repeat_count then 1 else ((.text | split("\n") | length) - (if (.text | endswith("\n")) then 1 else 0 end)) end' "$dir/dataset.jsonl")
  if [ "$code" -eq 2 ]; then calls=null; fi
  jq -cn --arg id "$id" --argjson code "$code" --argjson elapsed "$(( (end-start)/1000000 ))" \
    --arg query "$query" --arg endpoint "$endpoint" --arg model "$model" \
    --argjson threshold "$threshold" --argjson joinThreshold "${LG_JOIN_THRESHOLD:-0.5}" \
    --argjson maxFragmentBytes "$limit" --argjson maxLineBytes "${LG_MAX_LINE_BYTES:-16384}" \
    --argjson calls "$calls" \
    '{id:$id,exit:$code,elapsed_ms:$elapsed,output:($id+".stdout.jsonl"),stderr:($id+".stderr"),query:$query,endpoint:$endpoint,model:$model,threshold:$threshold,join_threshold:$joinThreshold,max_fragment_bytes:$maxFragmentBytes,max_line_bytes:$maxLineBytes,flow_derived_request_count:$calls,request_count_basis:"successful text-reader control flow; null on command error; not independently server-instrumented"}' \
    >> "$out/run-index.jsonl"
done
completed=$(date -u +%FT%TZ)
jq -cn --arg endpoint "$endpoint" --arg model "$model" --arg version "$version" \
  --arg started "$started" --arg completed "$completed" --arg hash "$dataset_sha" \
  --argjson health "$health_json" --argjson probeModel "$probe_model" --argjson probeRouting "$probe_routing" \
  --argjson contextProbe "$context_probe_json" \
  --argjson cases "$(jq -s '.' "$out/run-index.jsonl")" \
  '{endpoint:$endpoint,model:$model,lg_version:$version,started_utc:$started,completed_utc:$completed,dataset_sha256:$hash,health:$health,metadata_probe:{model:$probeModel,routing:$probeRouting},context_usage_probe:$contextProbe,cases:$cases}' \
  > "$out/run-metadata.json"
printf 'completed_utc=%s\n' "$completed"
echo "Wrote raw outputs and per-invocation wall latency under $out; each record gets one initial request plus one four-question request per considered adjacent pair. Review outputs against dataset labels; do not treat this script's exit alone as a quality score." >&2
