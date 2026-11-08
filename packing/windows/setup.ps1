param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Install', 'Uninstall')]
    [string]$Action,
    [string]$Prefix,
    [switch]$NoPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$payload = @('xcx-legacy.exe', 'xcx.lib', 'xcx_launcher.o', 'README.md', 'LICENSE', 'install.cmd', 'uninstall.cmd', 'setup.ps1')
$markerName = '.xcx-legacy-install.json'

function Read-Installation([string]$Directory) {
    $marker = Join-Path $Directory $markerName
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        throw "Refusing to modify an unmanaged directory: $Directory"
    }
    $state = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
    if ($state.product -ne 'xcx-legacy-v1' -or $state.directory -ne $Directory) {
        throw "Invalid installation marker: $marker"
    }
    return $state
}

try {
    if (-not $Prefix) {
        if ($Action -eq 'Uninstall' -and (Test-Path -LiteralPath (Join-Path $PSScriptRoot $markerName))) {
            $Prefix = $PSScriptRoot
        } else {
            $Prefix = Join-Path $env:LOCALAPPDATA 'Programs\xcx-legacy'
        }
    }
    $installDir = [IO.Path]::GetFullPath($Prefix).TrimEnd('\', '/')
    if (-not $installDir -or $installDir -eq [IO.Path]::GetPathRoot($installDir).TrimEnd('\', '/')) {
        throw 'The filesystem root is not an installation directory'
    }
    if (Test-Path -LiteralPath $installDir) {
        $item = Get-Item -LiteralPath $installDir -Force
        if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "The installation path must be a regular directory: $installDir"
        }
    }
    if ($Action -eq 'Install') {
        foreach ($file in $payload) {
            if (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot $file) -PathType Leaf)) {
                throw "Incomplete package: missing $file"
            }
        }
        $pathAdded = $false
        if (Test-Path -LiteralPath $installDir) {
            $state = Read-Installation $installDir
            $pathAdded = [bool]$state.pathAdded
        }
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        $state = [ordered]@{ product = 'xcx-legacy-v1'; directory = $installDir; pathAdded = $pathAdded }
        $state | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $installDir $markerName) -Encoding UTF8
        if ($PSScriptRoot -ne $installDir) {
            foreach ($file in $payload) {
                Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $installDir $file) -Force
            }
        }
        if (-not $NoPath) {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $entries = @($userPath -split ';')
            $present = @($entries | Where-Object {
                [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\', '/') -ieq $installDir
            }).Count -gt 0
            if (-not $present) {
                $updatedPath = if ([string]::IsNullOrEmpty($userPath)) { $installDir } else { "$userPath;$installDir" }
                [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
                $state.pathAdded = $true
                $state | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $installDir $markerName) -Encoding UTF8
            }
        }
        Write-Output "Installed: $installDir\xcx-legacy.exe"
        if (-not $NoPath) { Write-Output 'Open a new terminal to use xcx-legacy from PATH.' }
        Write-Output 'Compiling programs requires MSVC Build Tools and the Windows SDK.'
    } else {
        if (-not (Test-Path -LiteralPath $installDir)) {
            Write-Output 'xcx-legacy is not installed in this directory.'
            exit 0
        }
        $state = Read-Installation $installDir
        if ($state.pathAdded -and -not $NoPath) {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $entries = @($userPath -split ';' | Where-Object {
                [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\', '/') -ine $installDir
            })
            [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'User')
        }
        foreach ($file in ($payload + $markerName)) {
            $path = Join-Path $installDir $file
            if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path -Force }
        }
        if (@(Get-ChildItem -LiteralPath $installDir -Force).Count -eq 0) {
            Remove-Item -LiteralPath $installDir
        } else {
            Write-Output "Additional user files were preserved in $installDir"
        }
        Write-Output 'xcx-legacy uninstalled. Open a new terminal to refresh PATH.'
    }
    exit 0
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
