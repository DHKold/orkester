param(
    [Parameter()]
    [Alias("P")]
    [int32]$Port = 8080,

    [Parameter()]
    [Alias("NC")]
    [switch]$NoAutoCleanup
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$Namespace = "orkester"
$ReleaseName = "orkester"
$EcrUri = "058264326535.dkr.ecr.eu-west-1.amazonaws.com"
$RepoPath = "tests/infra/orkester:dev"
$FullImage = "$EcrUri/$RepoPath"

$portForwardProcess = $null

try {
    Write-Host ">>> Building dev image..."
    podman build -t orkester:dev -f "$ScriptDir\Dockerfile.dev" "$ScriptDir"

    Write-Host ">>> Tagging and pushing image to ECR: $FullImage"
    aws ecr get-login-password --region eu-west-1 | podman login --username AWS --password-stdin $EcrUri
    podman tag orkester:dev $FullImage
    podman push $FullImage

    Write-Host ">>> Deploying orkester to Kubernetes with Helm..."
    helm upgrade --install $ReleaseName "$ScriptDir\helm-charts\orkester" `
        --namespace $Namespace `
        --create-namespace

    Write-Host ">>> Waiting for orkester deployment to be ready..."
    kubectl wait -n $Namespace --for=condition=available deployment/$ReleaseName --timeout=120s

    Write-Host ">>> Port-forwarding orkester UI to http://localhost:$Port ..."
    $portForwardCommand = "kubectl port-forward -n $Namespace svc/$ReleaseName ${Port}:80"
    $portForwardProcess = Start-Process powershell.exe `
        -ArgumentList "-NoLogo", "-Command", $portForwardCommand `
        -WindowStyle Normal `
        -PassThru

    $podName = kubectl get pods -n $Namespace -l app.kubernetes.io/name=$ReleaseName -o jsonpath="{.items[0].metadata.name}"
    Write-Host ">>> Streaming logs from pod $podName (press Ctrl+C to stop)..."
    kubectl logs -n $Namespace -f $podName
}
finally {
    if (-not $NoAutoCleanup) {
        Write-Host ">>> Auto-cleanup enabled — stopping port-forwarding..."
        Stop-Process -Id $portForwardProcess.Id -Force -ErrorAction SilentlyContinue

        Write-Host ">>> Auto-cleanup enabled — uninstalling Helm release..."
        helm uninstall $ReleaseName -n $Namespace 2>$null

        Write-Host ">>> Deleting namespace..."
        kubectl delete namespace $Namespace --ignore-not-found=true
    }
}