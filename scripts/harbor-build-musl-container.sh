#!/usr/bin/env bash
# Persistent musl build container for Harbor-compatible BitFun CLI binaries.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCKERFILE="${ROOT}/scripts/harbor-build-musl/Dockerfile"
IMAGE="${BITFUN_HARBOR_MUSL_IMAGE:-bitfun-agent-kernel-harbor-musl-build:bookworm}"
CONTAINER="${BITFUN_HARBOR_MUSL_CONTAINER:-bitfun-agent-kernel-harbor-musl-build}"
REGISTRY_VOLUME="${BITFUN_HARBOR_MUSL_REGISTRY_VOLUME:-bitfun-agent-kernel-harbor-musl-cargo-registry}"
GIT_VOLUME="${BITFUN_HARBOR_MUSL_GIT_VOLUME:-bitfun-agent-kernel-harbor-musl-cargo-git}"
TARGET_TRIPLE="x86_64-unknown-linux-musl"
RELEASE_DIR="${ROOT}/target/${TARGET_TRIPLE}/release"
PRIMARY="${RELEASE_DIR}/bitfun"
LEGACY="${RELEASE_DIR}/bitfun-cli"

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  build-image       Build (or rebuild) the persistent musl build image
  start             Create/start the long-running build container
  stop              Stop the build container
  restart           stop + start
  shell             Open an interactive shell in the build container
  compile           Build both bitfun and bitfun-cli for ${TARGET_TRIPLE}
  test-binaries     Verify static linkage and run both binaries in Ubuntu and Alpine
  compile-and-test  compile + test-binaries
  print-mounts      Print the two Harbor/Pier bind mounts as JSON
  status            Show container/image/binary status
  logs              Follow container logs (usually empty for sleep infinity)

Environment overrides:
  BITFUN_HARBOR_MUSL_IMAGE, BITFUN_HARBOR_MUSL_CONTAINER
  BITFUN_HARBOR_MUSL_REGISTRY_VOLUME, BITFUN_HARBOR_MUSL_GIT_VOLUME

Output binaries:
  ${PRIMARY}
  ${LEGACY}

Both files are required. bitfun-cli is a deprecated compatibility launcher and
resolves its sibling bitfun binary at runtime.
EOF
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker not found" >&2
    exit 1
  fi
}

container_running() {
  docker inspect -f '{{.State.Running}}' "${CONTAINER}" 2>/dev/null | grep -q true
}

container_exists() {
  docker inspect "${CONTAINER}" >/dev/null 2>&1
}

docker_exec() {
  if [[ -t 0 && -t 1 ]]; then
    docker exec -it "${CONTAINER}" "$@"
  else
    docker exec "${CONTAINER}" "$@"
  fi
}

ensure_container_git_safe_directory() {
  docker_exec git config --global --add safe.directory /src >/dev/null 2>&1 || true
}

resolve_build_commit() {
  if [[ -n "${BITFUN_CLI_BUILD_COMMIT:-}" ]]; then
    echo "${BITFUN_CLI_BUILD_COMMIT}"
    return 0
  fi
  git -C "${ROOT}" rev-parse --short=12 HEAD 2>/dev/null || true
}

require_binary_pair() {
  local binary
  for binary in "${PRIMARY}" "${LEGACY}"; do
    if [[ ! -x "${binary}" ]]; then
      echo "error: binary not found or not executable: ${binary}" >&2
      echo "run: $(basename "$0") compile" >&2
      exit 1
    fi
  done
}

cmd_build_image() {
  docker build -f "${DOCKERFILE}" -t "${IMAGE}" "${ROOT}"
  echo "Built image: ${IMAGE}"
}

cmd_start() {
  docker volume create "${REGISTRY_VOLUME}" >/dev/null
  docker volume create "${GIT_VOLUME}" >/dev/null

  if container_exists; then
    if container_running; then
      echo "Container already running: ${CONTAINER}"
      return 0
    fi
    docker start "${CONTAINER}" >/dev/null
    echo "Started existing container: ${CONTAINER}"
    return 0
  fi

  cmd_build_image
  docker run -d \
    --name "${CONTAINER}" \
    -v "${ROOT}:/src" \
    -v "${REGISTRY_VOLUME}:/usr/local/cargo/registry" \
    -v "${GIT_VOLUME}:/usr/local/cargo/git" \
    -w /src \
    "${IMAGE}" \
    sleep infinity >/dev/null

  echo "Created and started container: ${CONTAINER}"
  echo "  source mount  : ${ROOT} -> /src"
  echo "  cargo registry: volume ${REGISTRY_VOLUME}"
  echo "  cargo git     : volume ${GIT_VOLUME}"
}

cmd_stop() {
  if container_exists; then
    docker stop "${CONTAINER}" >/dev/null || true
    echo "Stopped: ${CONTAINER}"
  else
    echo "Container not found: ${CONTAINER}"
  fi
}

cmd_shell() {
  cmd_start
  ensure_container_git_safe_directory
  docker exec -it "${CONTAINER}" bash
}

cmd_compile() {
  cmd_start
  ensure_container_git_safe_directory

  local commit
  commit="$(resolve_build_commit)"
  if [[ -n "${commit}" ]]; then
    echo "Build commit: ${commit}"
  else
    echo "warning: could not resolve the source commit" >&2
  fi

  docker_exec bash -lc "
    set -euo pipefail
    git config --global --add safe.directory /src 2>/dev/null || true
    cargo build --locked --release -p bitfun-cli --target ${TARGET_TRIPLE} --bins
  "
  require_binary_pair
  echo "Primary binary: ${PRIMARY}"
  echo "Compatibility entrypoint: ${LEGACY}"
}

verify_no_elf_interpreter() {
  local binary="$1"
  local container_path="/src/${binary#"${ROOT}/"}"
  if docker_exec readelf -l "${container_path}" | grep -q 'INTERP'; then
    echo "error: dynamically linked interpreter found in ${binary}" >&2
    exit 1
  fi
}

run_image_smoke() {
  local image="$1"
  echo
  echo "${image} smoke test:"
  docker run --rm \
    -v "${PRIMARY}:/usr/local/bin/bitfun:ro" \
    -v "${LEGACY}:/usr/local/bin/bitfun-cli:ro" \
    "${image}" \
    sh -ec '/usr/local/bin/bitfun --version; /usr/local/bin/bitfun --help >/dev/null; /usr/local/bin/bitfun-cli --version'
}

cmd_test_binaries() {
  cmd_start
  require_binary_pair

  echo "Host binary metadata:"
  file "${PRIMARY}" "${LEGACY}"
  verify_no_elf_interpreter "${PRIMARY}"
  verify_no_elf_interpreter "${LEGACY}"

  run_image_smoke ubuntu:22.04
  run_image_smoke debian:bookworm-slim
  run_image_smoke alpine:3.20
}

cmd_print_mounts() {
  require_binary_pair
  printf '[\n'
  printf '  {"type":"bind","source":"%s","target":"/usr/local/bin/bitfun","read_only":true},\n' "${PRIMARY}"
  printf '  {"type":"bind","source":"%s","target":"/usr/local/bin/bitfun-cli","read_only":true}\n' "${LEGACY}"
  printf ']\n'
}

cmd_status() {
  echo "Image: ${IMAGE}"
  docker image inspect "${IMAGE}" --format '  created: {{.Created}}' 2>/dev/null || echo "  (image not built yet)"
  echo "Container: ${CONTAINER}"
  if container_exists; then
    docker inspect "${CONTAINER}" --format '  status: {{.State.Status}}'
    docker inspect "${CONTAINER}" --format '  started: {{.State.StartedAt}}'
  else
    echo "  status: not created"
  fi
  echo "Volumes:"
  echo "  ${REGISTRY_VOLUME}"
  echo "  ${GIT_VOLUME}"
  echo "Binaries:"
  for binary in "${PRIMARY}" "${LEGACY}"; do
    if [[ -e "${binary}" ]]; then
      ls -lh "${binary}"
    else
      echo "  ${binary} (not built yet)"
    fi
  done
}

cmd_logs() {
  docker logs -f "${CONTAINER}"
}

main() {
  require_docker
  local cmd="${1:-}"
  case "${cmd}" in
    build-image) cmd_build_image ;;
    start) cmd_start ;;
    stop) cmd_stop ;;
    restart) cmd_stop; cmd_start ;;
    shell) cmd_shell ;;
    compile) cmd_compile ;;
    test-binaries|test-binary) cmd_test_binaries ;;
    compile-and-test) cmd_compile; cmd_test_binaries ;;
    print-mounts) cmd_print_mounts ;;
    status) cmd_status ;;
    logs) cmd_logs ;;
    -h|--help|help|"") usage ;;
    *)
      echo "error: unknown command: ${cmd}" >&2
      usage
      exit 1
      ;;
  esac
}

main "$@"
