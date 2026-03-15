#!/usr/bin/env bash
# docker/dev.sh — Start (or attach to) the Orkester dev container.
#
# Run from anywhere:
#   ./docker/dev.sh              — open an interactive shell in the dev container
#   ./docker/dev.sh cargo check  — run a single command inside the container

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

CONTAINER_NAME="orkester-dev"
IMAGE_NAME="orkester-dev"
VOLUME_NAME="orkester-build-cache"

# Create the build-cache volume if it doesn't exist yet.
podman volume exists "$VOLUME_NAME" 2>/dev/null || podman volume create "$VOLUME_NAME"

# If the container is already running, exec into it.
if podman ps --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
    echo ">>> Attaching to running container '${CONTAINER_NAME}'"
    if [ $# -eq 0 ]; then
        podman exec -it "$CONTAINER_NAME" bash
    else
        podman exec -it "$CONTAINER_NAME" "$@"
    fi
    exit 0
fi

# Locate the host Podman socket (rootless first, then root).
# Inside the container it is presented as /var/run/docker.sock so the Docker
# CLI (and Orkester's container executor) can find it without extra config.
_uid="$(id -u)"
if [ -S "/run/user/${_uid}/podman/podman.sock" ]; then
    PODMAN_SOCK="/run/user/${_uid}/podman/podman.sock"
elif [ -S "/run/podman/podman.sock" ]; then
    PODMAN_SOCK="/run/podman/podman.sock"
else
    echo "WARNING: no Podman socket found; container executor tasks will fail" >&2
    PODMAN_SOCK=""
fi

# Otherwise start a new background container and exec into it.
echo ">>> Starting dev container '${CONTAINER_NAME}'"

_socket_mount=""
[ -n "$PODMAN_SOCK" ] && _socket_mount="-v ${PODMAN_SOCK}:/var/run/docker.sock:z"

podman run -d \
    --name "$CONTAINER_NAME" \
    --replace \
    -v "${PROJECT_ROOT}:/orkester:z" \
    -v "${VOLUME_NAME}:/orkester/target:z" \
    ${_socket_mount} \
    -w /orkester \
    "$IMAGE_NAME" \
    sleep infinity

if [ $# -eq 0 ]; then
    podman exec -it "$CONTAINER_NAME" bash
else
    podman exec -it "$CONTAINER_NAME" "$@"
fi
