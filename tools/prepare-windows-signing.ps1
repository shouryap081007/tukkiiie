[CmdletBinding()]
param(
    [switch]$Required
)

$ErrorActionPreference = "Stop"

$pfxBase64 = $env:BONGOCAT_WINDOWS_SIGNING_PFX_BASE64
$pfxPassword = [string]$env:BONGOCAT_WINDOWS_SIGNING_PFX_PASSWORD
$thumbprint = $env:BONGOCAT_WINDOWS_SIGNING_THUMBPRINT

if (-not [string]::IsNullOrWhiteSpace($pfxBase64)) {
    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw "RUNNER_TEMP is missing; this script is intended for a Windows CI runner."
    }

    $pfxPath = Join-Path $env:RUNNER_TEMP "tukkiiie-windows-codesign.pfx"
    try {
        $bytes = [Convert]::FromBase64String($pfxBase64)
        [System.IO.File]::WriteAllBytes($pfxPath, $bytes)
        $securePassword = ConvertTo-SecureString -String $pfxPassword -AsPlainText -Force
        $imported = Import-PfxCertificate -FilePath $pfxPath -CertStoreLocation "Cert:\CurrentUser\My" -Password $securePassword

        $usable = @($imported | Where-Object { $_.HasPrivateKey })
        if ($usable.Count -ne 1) {
            throw "The PFX must contain exactly one certificate with its private key."
        }

        if (-not [string]::IsNullOrWhiteSpace($thumbprint) -and $thumbprint.Replace(" ", "").ToUpperInvariant() -ne $usable[0].Thumbprint.ToUpperInvariant()) {
            throw "The configured thumbprint does not match the imported PFX certificate."
        }
        $thumbprint = $usable[0].Thumbprint
    }
    finally {
        if (Test-Path -LiteralPath $pfxPath) {
            Remove-Item -LiteralPath $pfxPath -Force
        }
    }
}

if ([string]::IsNullOrWhiteSpace($thumbprint)) {
    if ($Required) {
        throw "A trusted Windows code-signing certificate is required for this production release."
    }

    Write-Warning "No Authenticode certificate is configured. This development build will remain unsigned and Smart App Control may block it."
    exit 0
}

$thumbprint = $thumbprint.Replace(" ", "").ToUpperInvariant()
$certificate = Get-ChildItem -Path "Cert:\CurrentUser\My\$thumbprint" -ErrorAction SilentlyContinue
if ($null -eq $certificate) {
    throw "Certificate $thumbprint is not installed in the current user's Personal certificate store."
}
if (-not $certificate.HasPrivateKey) {
    throw "Certificate $thumbprint has no private key available to this runner."
}

$codeSigningOid = "1.3.6.1.5.5.7.3.3"
$hasCodeSigningEku = @($certificate.EnhancedKeyUsageList | Where-Object {
    $_.ObjectId.Value -eq $codeSigningOid
}).Count -gt 0
if (-not $hasCodeSigningEku) {
    throw "Certificate $thumbprint does not contain the Code Signing enhanced key usage."
}

$signTool = $env:BONGOCAT_WINDOWS_SIGNTOOL
if ([string]::IsNullOrWhiteSpace($signTool)) {
    $command = Get-Command "signtool.exe" -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        $signTool = $command.Source
    }
    else {
        $programFilesX86 = [Environment]::GetEnvironmentVariable("ProgramFiles(x86)")
        $kits = Join-Path $programFilesX86 "Windows Kits\10\bin"
        $candidate = Get-ChildItem -Path $kits -Filter "signtool.exe" -File -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } |
            Sort-Object -Property FullName -Descending |
            Select-Object -First 1
        if ($null -eq $candidate) {
            throw "Windows SDK SignTool was not found on this runner."
        }
        $signTool = $candidate.FullName
    }
}

if (-not (Test-Path -LiteralPath $signTool -PathType Leaf)) {
    throw "SignTool was not found at: $signTool"
}

$timestampUrl = $env:BONGOCAT_WINDOWS_TIMESTAMP_URL
if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
    $timestampUrl = "http://timestamp.digicert.com"
}

if ([string]::IsNullOrWhiteSpace($env:GITHUB_ENV) -or [string]::IsNullOrWhiteSpace($env:GITHUB_PATH)) {
    throw "GITHUB_ENV and GITHUB_PATH are required."
}

Add-Content -Path $env:GITHUB_ENV -Value "BONGOCAT_WINDOWS_SIGNING_THUMBPRINT=$thumbprint"
Add-Content -Path $env:GITHUB_ENV -Value "BONGOCAT_WINDOWS_SIGNTOOL=$signTool"
Add-Content -Path $env:GITHUB_ENV -Value "BONGOCAT_WINDOWS_TIMESTAMP_URL=$timestampUrl"
Add-Content -Path $env:GITHUB_PATH -Value (Split-Path -Parent $signTool)

Write-Host "Authenticode signing is configured for certificate $thumbprint."
