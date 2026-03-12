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

# ── 3. Start the container if it is not already running ───────────────────────
$running = podman ps --format "{{.Names}}" | Select-String -Quiet "^$ContainerName$"

if ($running) {
    Write-Host ">>> Attaching to running container '$ContainerName'..."
} else {
    Write-Host ">>> Starting dev container '$ContainerName'..."
    podman run -d `
        --name $ContainerName `
        --replace `
        -v "${ProjectRoot}:/orkester:z" `
        -v "${VolumeName}:/orkester/target:z" `
        -p 8080:8080 `
        -w /orkester `
        $ImageName `
        sleep infinity
}

# ── 4. Open a shell (or run the supplied command) ─────────────────────────────
# You can run `cargo` commands here, or start Orkester with:
#   cargo run -p orkester
if ($args.Count -eq 0) {
    podman exec -it $ContainerName bash
} else {
    podman exec -it $ContainerName @args
}