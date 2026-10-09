#!/usr/bin/env bash
# Run against the local Rust server; this script has no Python dependency.
set -euo pipefail

BASE_URL="${1:-http://127.0.0.1:3210}"
SAMPLES_PER_LEVEL="${2:-10}"
OUT_DIR="${3:-benchmark-$(date -u +%Y%m%dT%H%M%SZ)}"
SCOPE="${4:-levels}"
JOB_TIMEOUT_SECONDS="${BENCHMARK_JOB_TIMEOUT_SECONDS:-150}"
SEED_BASE="${BENCHMARK_SEED_BASE:-20261009}"

if [[ "${1:-}" == --help ]]; then
  cat <<'USAGE'
Usage: scripts/benchmark.sh [base-url] [samples-per-case] [output-directory] [levels|families]
Defaults: http://127.0.0.1:3210, 10 samples for each of Lv1–Lv5.
The families scope explicitly verifies each catalog family at every supported level.
Example: scripts/benchmark.sh http://127.0.0.1:3210 1 test-results/families families
Requirements: Bash, curl, jq; the Rust application must already be running.
Optional environment: BENCHMARK_JOB_TIMEOUT_SECONDS=150, BENCHMARK_SEED_BASE=20261009
Saves status/environment.json, one JSON record per attempt, results.csv and summary.json.
USAGE
  exit 0
fi

for dependency in curl jq; do
  command -v "$dependency" >/dev/null || { echo "Required command missing: $dependency" >&2; exit 1; }
done
[[ "$SAMPLES_PER_LEVEL" =~ ^[1-9][0-9]*$ ]] || { echo "samples-per-level must be a positive integer" >&2; exit 1; }
[[ "$JOB_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]] || { echo "BENCHMARK_JOB_TIMEOUT_SECONDS must be a positive integer" >&2; exit 1; }
[[ "$SEED_BASE" =~ ^[0-9]+$ ]] || { echo "BENCHMARK_SEED_BASE must be a nonnegative integer" >&2; exit 1; }
[[ "$SCOPE" == levels || "$SCOPE" == families ]] || { echo "scope must be levels or families" >&2; exit 1; }
BASE_URL="${BASE_URL%/}"
mkdir -p "$OUT_DIR/attempts"
curl --fail --silent --show-error --max-time 10 "$BASE_URL/api/status" > "$OUT_DIR/status.json"
jq -e '.ready == true' "$OUT_DIR/status.json" >/dev/null || {
  jq -r '.verification.message // .message // "Lean environment is not ready"' "$OUT_DIR/status.json" >&2
  exit 1
}
if [[ "$SCOPE" == families ]]; then
  jq -r '.families[] | .id as $family | .levels[] | [.,$family] | @tsv' "$OUT_DIR/status.json" > "$OUT_DIR/cases.tsv"
else
  printf '1\tmixed\n2\tmixed\n3\tmixed\n4\tmixed\n5\tmixed\n' > "$OUT_DIR/cases.tsv"
fi
[[ -s "$OUT_DIR/cases.tsv" ]] || { echo "No supported family/level cases found" >&2; exit 1; }

HOST_OS="$(uname -srm)"
HOST_CPU="$(awk -F ': ' '/model name/ {print $2; exit}' /proc/cpuinfo 2>/dev/null || true)"
HOST_MEMORY_KIB="$(awk '/MemTotal:/ {print $2}' /proc/meminfo 2>/dev/null || true)"
HOST_DISK="$(df -PT "$OUT_DIR" | tail -n 1)"
jq -n --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg url "$BASE_URL" \
  --arg os "$HOST_OS" --arg cpu "$HOST_CPU" --arg memory "$HOST_MEMORY_KIB" \
  --arg disk "$HOST_DISK" --argjson count "$SAMPLES_PER_LEVEL" \
  --argjson seed "$SEED_BASE" --arg scope "$SCOPE" --slurpfile status "$OUT_DIR/status.json" \
  '{started_at_utc:$at,url:$url,scope:$scope,samples_per_case:$count,seed_base:$seed,
    environment:{os:$os,cpu:$cpu,memory_kib:$memory,filesystem:$disk},
    readiness:$status[0],measurement:"prepared environment; serial requests; client polling included in request_elapsed_ms"}' \
  > "$OUT_DIR/environment.json"
echo 'level,trial,seed,status,family,difficulty_score,route_cost,request_elapsed_ms,job_elapsed_ms,lean_elapsed_ms,problem_hash,proof_hash,job_id,message' > "$OUT_DIR/results.csv"
attempt_files=()

now_ms() { date +%s%3N; }

while IFS=$'\t' read -r level requested_family; do
  [[ "$level" =~ ^[1-5]$ && "$requested_family" =~ ^[a-z_]+$ ]] || { echo "Invalid catalog case" >&2; exit 1; }
  for ((trial = 1; trial <= SAMPLES_PER_LEVEL; trial++)); do
    seed=$((SEED_BASE + level * 10000 + trial))
    prefix="lv${level}"
    if [[ "$SCOPE" == families ]]; then prefix="family-${requested_family}-lv${level}"; fi
    attempt_file="$OUT_DIR/attempts/${prefix}-$(printf '%02d' "$trial").json"
    request_file="$OUT_DIR/attempts/${prefix}-$(printf '%02d' "$trial").request.json"
    started_ms="$(now_ms)"
    body="$(jq -n --argjson level "$level" --argjson seed "$seed" --arg family "$requested_family" '{level:$level,family:$family,seed:$seed}')"
    if ! curl --fail --silent --show-error --max-time 15 -H 'Content-Type: application/json' \
      --data "$body" "$BASE_URL/api/generate" > "$request_file"; then
      echo "Generation request failed for Lv${level}, trial ${trial}." >&2
      exit 1
    fi
    job_id="$(jq -er '.id' "$request_file")"
    job_url="$BASE_URL/api/jobs/$job_id"
    while true; do
      curl --fail --silent --show-error --max-time 15 "$job_url" > "$attempt_file.tmp"
      state="$(jq -er '.status' "$attempt_file.tmp")"
      case "$state" in
        succeeded|failed|timeout|cancelled) break ;;
        generating|verifying) ;;
        *) echo "Unknown job state: $state" >&2; exit 1 ;;
      esac
      elapsed_ms=$(( $(now_ms) - started_ms ))
      if ((elapsed_ms > JOB_TIMEOUT_SECONDS * 1000)); then
        # Preserve observed state and stop rather than submit overlapping jobs.
        jq --argjson elapsed "$elapsed_ms" \
          '. + {benchmark_client_timeout:true,benchmark_client_elapsed_ms:$elapsed}' \
          "$attempt_file.tmp" > "$attempt_file"
        rm -f "$attempt_file.tmp"
        curl --silent --show-error --max-time 10 -X POST "$job_url/cancel" > "$OUT_DIR/attempts/cancel-$job_id.json" || true
        echo "Client timeout for $job_id; observed job saved to $attempt_file. Stop/cancel the job before rerunning." >&2
        exit 1
      fi
      sleep 0.25
    done
    elapsed_ms=$(( $(now_ms) - started_ms ))
    jq --argjson level "$level" --argjson trial "$trial" --argjson seed "$seed" \
      --argjson elapsed "$elapsed_ms" --arg family "$requested_family" \
      '. + {benchmark:{level:$level,trial:$trial,seed:$seed,requested_family:$family,request_elapsed_ms:$elapsed}}' \
      "$attempt_file.tmp" > "$attempt_file"
    attempt_files+=("$attempt_file")
    rm -f "$attempt_file.tmp"
    if [[ "$state" == succeeded ]]; then
      jq -e --arg family "$requested_family" --argjson level "$level" '
        .problem.classification.level == $level and
        ($family == "mixed" or .problem.recipe.family == $family) and
        .verification.status == "success" and .verification.exit_code == 0 and
        .verification.problem_hash == .problem.id and
        (.verification.checked_theorems | length) >= 4 and
        all(.verification.axioms[]; . == "propext" or . == "Classical.choice" or . == "Quot.sound")
      ' "$attempt_file" >/dev/null || { echo "Invalid success record: $attempt_file" >&2; exit 1; }
    fi
    jq -r '[.benchmark.level,.benchmark.trial,.benchmark.seed,.status,
      (.problem.recipe.family // ""),
      (.problem.classification.score // ""),
      (.problem.classification.total_score // ""),
      .benchmark.request_elapsed_ms,(.elapsed_ms // ""),(.verification.elapsed_ms // ""),
      (.verification.problem_hash // .problem.problem_hash // ""),(.verification.proof_hash // ""),.id,(.message // "")] | @csv' \
      "$attempt_file" >> "$OUT_DIR/results.csv"
    printf 'Lv%s %s %s/%s: %s, %s ms\n' "$level" "$requested_family" "$trial" "$SAMPLES_PER_LEVEL" "$state" "$elapsed_ms"
  done
done < "$OUT_DIR/cases.tsv"

jq -s '
  def distribution:
    sort as $v | length as $n |
    if $n == 0 then {count:0,median_ms:null,p95_ms:null,max_ms:null}
    else {count:$n,
      median_ms:(if ($n % 2) == 1 then $v[($n/2|floor)] else (($v[$n/2-1]+$v[$n/2])/2) end),
      p95_ms:$v[((0.95*$n|ceil)-1)],max_ms:$v[-1]} end;
  {attempts:length,
   states:(group_by(.status)|map({key:.[0].status,value:length})|from_entries),
   successful_request_times:([.[]|select(.status=="succeeded")|.benchmark.request_elapsed_ms]|distribution),
   successful_lean_times:([.[]|select(.status=="succeeded")|.verification.elapsed_ms|select(type=="number")]|distribution),
   explicit_family_levels:([.[]|select(.benchmark.requested_family!="mixed")]
     |group_by([.benchmark.requested_family,.benchmark.level])
     |map({family:.[0].benchmark.requested_family,level:.[0].benchmark.level,
       attempts:length,successful:(map(select(.status=="succeeded"))|length)})),
   levels:(group_by(.benchmark.level)|map({level:.[0].benchmark.level,
     attempts:length,successful:(map(select(.status=="succeeded"))|length),
     request_times:([.[]|select(.status=="succeeded")|.benchmark.request_elapsed_ms]|distribution),
     lean_times:([.[]|select(.status=="succeeded")|.verification.elapsed_ms|select(type=="number")]|distribution),
     families:(group_by(.problem.recipe.family)|map({family:.[0].problem.recipe.family,count:length}))}))}
' "${attempt_files[@]}" > "$OUT_DIR/summary.json"
jq . "$OUT_DIR/summary.json"
printf 'Saved benchmark records in %s\n' "$OUT_DIR"
