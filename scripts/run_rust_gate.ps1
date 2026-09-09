param(
    [string]$ToolchainRoot = "$env:LOCALAPPDATA\Temp\terminal-poker-rust-sprint0",
    [string]$Toolchain = "stable-x86_64-pc-windows-gnu",
    [string]$Target = "x86_64-pc-windows-gnu",
    [string]$ReleaseTargetDir = "target\sprint13-gate"
)

$ErrorActionPreference = "Stop"
$rustGateRoot = (Resolve-Path -LiteralPath $ToolchainRoot).Path
$cargoBin = Join-Path $rustGateRoot "cargo\bin"
$cargoExe = Join-Path $cargoBin "cargo.exe"
$rustupRoot = Join-Path $rustGateRoot "rustup"

if (-not (Test-Path -LiteralPath $cargoExe -PathType Leaf)) {
    throw "Cargo was not found at $cargoExe"
}

$env:RUSTUP_HOME = $rustupRoot
$env:CARGO_HOME = Join-Path $rustGateRoot "cargo"
$env:Path = "$cargoBin;$env:Path"

function Invoke-GateStep {
    param(
        [string]$Name,
        [string[]]$Arguments
    )

    Write-Host "[gate] $Name"
    & $cargoExe "+$Toolchain" @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

& $cargoExe "+$Toolchain" --version
& (Join-Path $cargoBin "rustup.exe") run $Toolchain rustc --version

Invoke-GateStep "format" @("fmt", "--all", "--", "--check")
Invoke-GateStep "strict clippy" @("clippy", "--all-targets", "--all-features", "--target", $Target, "--", "-D", "warnings")
Invoke-GateStep "full test suite" @("test", "--all-targets", "--all-features", "--target", $Target)
Invoke-GateStep "release build" @("build", "--release", "--all-features", "--target", $Target, "--target-dir", $ReleaseTargetDir)

$releaseBinary = Join-Path (Get-Location) "$ReleaseTargetDir\$Target\release\terminal-poker.exe"
if (-not (Test-Path -LiteralPath $releaseBinary -PathType Leaf)) {
    throw "Release binary was not found at $releaseBinary"
}

Write-Host "[gate] release help smoke"
& $releaseBinary --help | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Release help smoke failed with exit code $LASTEXITCODE"
}

Write-Host "[gate] release version smoke"
& $releaseBinary --version | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Release version smoke failed with exit code $LASTEXITCODE"
}

Write-Host "[gate] PASS"
