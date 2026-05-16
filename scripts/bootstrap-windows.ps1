# Install RaidhOS build prerequisites on Windows.
# Run from an elevated PowerShell session for the WebView2 install.

$ErrorActionPreference = "Stop"

function Ensure-Winget {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "winget is required. Install 'App Installer' from the Microsoft Store and re-run."
    }
}

Ensure-Winget

$packages = @(
    "Rustlang.Rustup",
    "Microsoft.EdgeWebView2Runtime",
    "Microsoft.VisualStudio.2022.BuildTools",
    "Git.Git"
)

foreach ($pkg in $packages) {
    Write-Host "Ensuring $pkg is installed..."
    winget install --silent --accept-package-agreements --accept-source-agreements --id $pkg | Out-Null
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Initialising stable Rust toolchain..."
    & "$env:USERPROFILE\.cargo\bin\rustup-init.exe" -y --default-toolchain stable --profile minimal
}
