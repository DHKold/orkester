# docker/dev.ps1 — Start (or attach to) the Orkester dev container.
#
# Run from anywhere inside the project:
#   .\docker\dev.ps1              — open an interactive shell in the dev container
#   .\docker\dev.ps1 cargo check  — run a single command inside the container

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path

$ContainerName = "orkester-dev"
$ImageName     = "orkester-dev"
$VolumeName    = "orkester-build-cache"

# ── 1. Build the dev image ────────────────────────────────────────────────────
Write-Host ">>> Building image '$ImageName' (target: dev)..."
podman build --target dev -t $ImageName -f "$ScriptDir\Dockerfile" $ProjectRoot

# ── 2. Ensure the build-cache volume exists ───────────────────────────────────
$volumeExists = podman volume ls --format "{{.Name}}" | Select-String -Quiet "^$VolumeName$"
if (-not $volumeExists) {
    Write-Host ">>> Creating volume '$VolumeName'..."
    podman volume create $VolumeName
}

# ── 3. Find the Podman socket inside the Podman machine and start the container ──
# The dev container runs inside the Podman WSL2 machine.  We mount the daemon
# socket as /var/run/docker.sock so the Docker CLI (and Orkester's container
# executor) can reach Podman without any extra config.
# Derive the Linux-side socket path from the Rootful flag in machine inspect.
$PodmanSock = ""
try {
    $machineInfo = (podman machine inspect 2>$null | ConvertFrom-Json)[0]
    if ($machineInfo.Rootful) {
        $PodmanSock = "/run/podman/podman.sock"
    } else {
        $PodmanSock = "/run/user/1000/podman/podman.sock"
    }
} catch {
    Write-Warning "Could not inspect Podman machine; container executor tasks will fail"
}

# Always (re)create the container so it picks up the freshly built image.
Write-Host ">>> (Re)creating dev container '$ContainerName'..."
podman rm -f $ContainerName 2>$null

$RunArgs = @(
    "run", "-d",
    "--name", $ContainerName,
    "-v", "${ProjectRoot}:/orkester:z",                            # Mount the project source as a volume so changes are reflected inside the container.
    "-v", "${VolumeName}:/orkester/target:z"                       # Mount the build cache as a volume so it persists across container restarts.
)
if ($PodmanSock) {
    Write-Host ">>> Mounting Podman socket: $PodmanSock"
    $RunArgs += @("-v", "${PodmanSock}:/var/run/docker.sock:z")
} else {
    Write-Warning "No Podman socket found; container executor tasks will fail"
}
$RunArgs += @(
    "-p", "8080:8080",
    "-w", "/orkester",
    $ImageName,
    "sleep", "infinity"
)
podman @RunArgs

# ── 4. Open a shell (or run the supplied command) ─────────────────────────────
# You can run `cargo` commands here, or start Orkester with:
#   cargo run -p orkester
if ($args.Count -eq 0) {
    podman exec -it $ContainerName bash
} else {
    podman exec -it $ContainerName @args
}