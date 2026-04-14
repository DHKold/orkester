# docker/dev.ps1 — Start (or attach to) the Orkester dev container.
#
# Run from anywhere inside the project:
#   .\docker\dev.ps1              — open an interactive shell in the dev container
#   .\docker\dev.ps1 cargo check  — run a single command inside the container

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path

$ContainerName  = "orkester-dev"
$ImageName      = "orkester-dev"
$VolumeName     = "orkester-build-cache"
$DockerfilePath = "$ScriptDir\Dockerfile"

# -- 1. Build the dev image only when the Dockerfile has changed -----------------
$NeedsBuild    = $true
$imageCreatedRaw = podman image inspect $ImageName --format "{{.Created}}" 2>$null
if ($LASTEXITCODE -eq 0 -and $imageCreatedRaw) {
    try {
        $ImageCreated   = [DateTime]$imageCreatedRaw
        $DockerfileDate = (Get-Item $DockerfilePath).LastWriteTime
        if ($DockerfileDate -le $ImageCreated) { $NeedsBuild = $false }
    } catch {
        # Unparseable creation date -- rebuild to be safe
    }
}

if ($NeedsBuild) {
    Write-Host ">>> Building image '$ImageName' (target: dev)..."
    podman build --target dev -t $ImageName -f $DockerfilePath $ProjectRoot
} else {
    Write-Host ">>> Dockerfile unchanged -- skipping image build."
}

$NewImageId = (podman image inspect $ImageName --format "{{.Id}}" 2>$null)

# -- 2. Ensure the build-cache volume exists -------------------------------------
$volumeExists = podman volume ls --format "{{.Name}}" | Select-String -Quiet "^$VolumeName$"
if (-not $volumeExists) {
    Write-Host ">>> Creating volume '$VolumeName'..."
    podman volume create $VolumeName
}

# -- 3. Find the Podman socket inside the Podman machine and start the container -
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

# -- 4. Manage the dev container ------------------------------------------------
$ContainerImageId = try { podman inspect $ContainerName --format "{{.Image}}" 2>$null } catch { "" }
$ContainerStatus  = try { podman inspect $ContainerName --format "{{.State.Status}}" 2>$null } catch { "" }
$NeedsRecreate    = (-not $ContainerImageId) -or ($ContainerImageId -ne $NewImageId)

if ($NeedsRecreate) {
    Write-Host ">>> Recreating dev container '$ContainerName'..."
    podman rm -f $ContainerName 2>$null

    $RunArgs = @(
        "run", "-d",
        "--name", $ContainerName,
        "-v", "${ProjectRoot}:/orkester:z",
        "-v", "${VolumeName}:/orkester/target:z"
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
} elseif ($ContainerStatus -ne "running") {
    Write-Host ">>> Starting stopped container '$ContainerName'..."
    podman start $ContainerName
} else {
    Write-Host ">>> Container '$ContainerName' already running -- reusing."
}

# -- 5. Open a shell (or run the supplied command) ------------------------------
# You can run `cargo` commands here, or start Orkester with:
#   cargo run -p orkester
if ($args.Count -eq 0) {
    podman exec -it $ContainerName bash
} else {
    podman exec -it $ContainerName @args
}