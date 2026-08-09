#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$ROOT_DIR/validation/tla"
EXPECTED_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
JAR_PATH="${SOPHIA_TLA2TOOLS_JAR:-}"

if [[ -z "$JAR_PATH" ]]; then
    echo "SOPHIA_TLA2TOOLS_JAR must name the pinned TLA+ Tools v1.7.4 jar" >&2
    exit 2
fi
if [[ "$JAR_PATH" != /* ]]; then
    echo "SOPHIA_TLA2TOOLS_JAR must be an absolute path" >&2
    exit 2
fi
if [[ ! -f "$JAR_PATH" ]]; then
    echo "TLA+ tools jar not found: $JAR_PATH" >&2
    exit 2
fi
if ! command -v java >/dev/null 2>&1; then
    echo "Java 11 or newer is required to run TLC" >&2
    exit 2
fi

actual_sha256="$(sha256sum "$JAR_PATH" | awk '{print $1}')"
if [[ "$actual_sha256" != "$EXPECTED_SHA256" ]]; then
    echo "TLA+ tools jar checksum mismatch" >&2
    echo "expected: $EXPECTED_SHA256" >&2
    echo "actual:   $actual_sha256" >&2
    exit 2
fi

java_major="$(java -version 2>&1 | sed -n '1s/.*version "\([0-9][0-9]*\).*/\1/p')"
if [[ -z "$java_major" ]] || (( java_major < 11 )); then
    echo "Java 11 or newer is required to run TLC" >&2
    exit 2
fi

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT
for model in VisualRetirement AdmissionRecovery PresentFrameOwnership PresentCopyOwnership SurfaceContentStream GeometryFeedback PolicyConnection PolicyProjection PolicyLifecycle PolicySettlementRecovery PolicyOutputSettlement ShellObservation ShellWorkAreaCoordination IndicatorTransfer IndicatorAction TargetResolvedInput TargetInputPacing InputAuthorityArbitration LegacyWmProjection LegacyWmResponseBoundary PixelSilentAdmission; do
    cp "$MODEL_DIR/$model.tla" "$TEMP_DIR/"
    cp "$MODEL_DIR/$model.cfg" "$TEMP_DIR/"
    (
        cd "$TEMP_DIR"
        java -XX:+UseParallelGC -jar "$JAR_PATH" \
            -deadlock \
            -workers 1 \
            -fp 0 \
            -config "$model.cfg" \
            "$model.tla"
    )
done
