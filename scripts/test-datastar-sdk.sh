#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
REPO_ROOT=$PWD
RUNNER=native

usage() {
    echo "usage: $0 [--native|--docker] [all|axum|rocket|warp]"
    echo
    echo "  --native  run the official suite with the local Go toolchain (default)"
    echo "  --docker  run the official suite with the Go Docker image"
    echo
    echo "environment:"
    echo "  DATASTAR_SDK_TEST_VERSION  Go module version (default: latest)"
    echo "  DATASTAR_GO_IMAGE          Docker image (default: golang:1.25)"
}

case "${1:-}" in
    --native) shift ;;
    --docker) RUNNER=docker; shift ;;
    -h|--help) usage; exit 0 ;;
esac

if [ "$#" -gt 1 ]; then
    usage >&2
    exit 2
fi

FRAMEWORK=${1:-all}
case "$FRAMEWORK" in
    all) FRAMEWORKS=(axum rocket warp) ;;
    axum|rocket|warp) FRAMEWORKS=("$FRAMEWORK") ;;
    *) usage >&2; exit 2 ;;
esac

for command in cargo curl; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command not found: $command" >&2
        exit 1
    fi
done

if [ "$RUNNER" = native ]; then
    runner_command=go
else
    runner_command=docker
fi

if ! command -v "$runner_command" >/dev/null 2>&1; then
    echo "required command not found: $runner_command" >&2
    exit 1
fi

if [ "$RUNNER" = docker ] && ! docker info >/dev/null 2>&1; then
    echo "Docker daemon is not available" >&2
    exit 1
fi

target_dir="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
case "$target_dir" in
    /*) ;;
    *) target_dir="$REPO_ROOT/$target_dir" ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/datastar-rust-sdk.XXXXXX")
server_pid=""
server_log=""

stop_server() {
    if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
        kill "$server_pid"
        wait "$server_pid" 2>/dev/null || true
    fi
    server_pid=""
}

cleanup() {
    status=$?
    trap - EXIT
    stop_server

    if [ "$status" -ne 0 ] && [ -n "$server_log" ]; then
        echo "Datastar SDK server log:" >&2
        cat "$server_log" >&2
    fi

    rm -rf "$work_dir"
    exit "$status"
}
trap cleanup EXIT

run_official_suite() {
    local test_package=github.com/starfederation/datastar/sdk/tests/cmd/datastar-sdk-tests
    local test_version="${DATASTAR_SDK_TEST_VERSION:-latest}"

    if [ "$RUNNER" = native ]; then
        go run "$test_package@$test_version" \
            -v \
            -server http://127.0.0.1:9200
    elif [ "$(uname -s)" = Linux ]; then
        docker run --rm --network host "${DATASTAR_GO_IMAGE:-golang:1.25}" \
            go run "$test_package@$test_version" \
            -v \
            -server http://127.0.0.1:9200
    else
        docker run --rm "${DATASTAR_GO_IMAGE:-golang:1.25}" \
            go run "$test_package@$test_version" \
            -v \
            -server http://host.docker.internal:9200
    fi
}

run_framework() {
    local framework=$1
    local example features
    case "$framework" in
        axum)
            example=axum-test-suite
            features=axum,tracing
            ;;
        rocket)
            example=rocket-test-suite
            features=rocket
            ;;
        warp)
            example=warp-test-suite
            features=warp,tracing
            ;;
    esac

    printf '\n==> Testing %s\n' "$framework"
    cargo build --example "$example" --features "$features"

    server_log="$work_dir/$framework.log"
    "$target_dir/debug/examples/$example" >"$server_log" 2>&1 &
    server_pid=$!

    local server_ready=false
    local server_attempt
    for ((server_attempt = 1; server_attempt <= 60; server_attempt++)); do
        if ! kill -0 "$server_pid" 2>/dev/null; then
            echo "$framework server exited before becoming ready" >&2
            return 1
        fi

        if curl --silent --output /dev/null --max-time 1 \
            http://127.0.0.1:9200/test \
            && kill -0 "$server_pid" 2>/dev/null; then
            server_ready=true
            break
        fi
        sleep 1
    done

    if [ "$server_ready" != true ]; then
        echo "$framework server did not become ready" >&2
        return 1
    fi

    run_official_suite
    stop_server
    server_log=""
}

for framework in "${FRAMEWORKS[@]}"; do
    run_framework "$framework"
done
