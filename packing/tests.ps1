param(
    [Parameter(Mandatory = $true)]
    [string]$Archive,
    [switch]$TestUserPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$archivePath = (Resolve-Path -LiteralPath $Archive).Path
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('xcx-legacy-package-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$windows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
$originalUserPath = if ($windows -and $TestUserPath) { [Environment]::GetEnvironmentVariable('Path', 'User') } else { $null }
$packageDir = Join-Path $testRoot 'extracted/xcx-legacy'

function Assert([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Setup([string]$Action, [string]$Prefix, [string]$Source = $packageDir, [bool]$ShouldSucceed = $true) {
    if ($windows) {
        [string[]]$arguments = @()
        if (-not ($Action -eq 'uninstall' -and $Source -ne $packageDir)) { $arguments += @('-Prefix', $Prefix) }
        if (-not $TestUserPath) { $arguments += '-NoPath' }
        & (Join-Path $Source "$Action.cmd") @arguments
    } else {
        if ($Action -eq 'uninstall' -and $Source -ne $packageDir) {
            & bash (Join-Path $Source "$Action.sh")
        } else {
            & bash (Join-Path $Source "$Action.sh") --prefix $Prefix
        }
    }
    $code = $LASTEXITCODE
    Assert (($code -eq 0) -eq $ShouldSucceed) "Unexpected $Action exit code: $code"
}

function InstalledDirectory([string]$Prefix) {
    if ($windows) { return $Prefix }
    return Join-Path $Prefix 'lib/xcx-legacy'
}

try {
    if ($windows) {
        Expand-Archive -LiteralPath $archivePath -DestinationPath (Join-Path $testRoot 'extracted')
        $expected = @('xcx-legacy.exe', 'xcx.lib', 'xcx_launcher.o', 'README.md', 'LICENSE', 'install.cmd', 'uninstall.cmd', 'setup.ps1')
    } else {
        New-Item -ItemType Directory -Path (Join-Path $testRoot 'extracted') | Out-Null
        & tar -xzf $archivePath -C (Join-Path $testRoot 'extracted')
        Assert ($LASTEXITCODE -eq 0) 'Cannot extract distribution'
        $expected = @('xcx-legacy', 'libxcx.a', 'xcx_launcher.o', 'README.md', 'LICENSE', 'install.sh', 'uninstall.sh')
        foreach ($file in @('xcx-legacy', 'install.sh', 'uninstall.sh')) {
            & test -x (Join-Path $packageDir $file)
            Assert ($LASTEXITCODE -eq 0) "Missing executable mode: $file"
        }
        foreach ($file in @('install.sh', 'uninstall.sh')) {
            & bash -n (Join-Path $packageDir $file)
            Assert ($LASTEXITCODE -eq 0) "Invalid shell syntax: $file"
        }
    }
    $actual = @(Get-ChildItem -LiteralPath $packageDir -File | ForEach-Object { $_.Name })
    Assert (@(Compare-Object ($expected | Sort-Object) ($actual | Sort-Object)).Count -eq 0) 'Unexpected package contents'
    $licenseHash = (Get-FileHash -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'LICENSE')).Hash
    Assert ((Get-FileHash -LiteralPath (Join-Path $packageDir 'LICENSE')).Hash -eq $licenseHash) 'Package license differs from the repository license'
    $readme = Get-Content -LiteralPath (Join-Path $packageDir 'README.md') -Raw
    Assert ($readme.Contains('https://github.com/xcxlang-org/xcx-legacy')) 'Missing legacy repository link'
    Assert ($readme.Contains('https://github.com/xcxlang-org/xcx')) 'Missing modern repository link'
    Assert ($readme.Contains('--release')) 'Missing CLI quick reference'
    Assert ($readme -notmatch '@(VERSION|TARGET|INSTALL|UNINSTALL|TOOLCHAIN)@') 'Unexpanded README template'

    $prefix = Join-Path $testRoot 'prefix with spaces'
    Setup install $prefix
    Setup install $prefix
    $installed = InstalledDirectory $prefix
    Assert ((Get-FileHash -LiteralPath (Join-Path $installed 'LICENSE')).Hash -eq $licenseHash) 'Installed license differs from the repository license'
    $compiler = if ($windows) { Join-Path $installed 'xcx-legacy.exe' } else { Join-Path $prefix 'bin/xcx-legacy' }
    $version = & $compiler --version
    Assert ($LASTEXITCODE -eq 0 -and $version -like 'xcx-legacy *') 'Installed compiler cannot run'
    if ($windows -and $TestUserPath) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        Assert (@($userPath -split ';' | Where-Object { $_ -ieq $installed }).Count -eq 1) 'Install did not add exactly one PATH entry'
    }
    $source = Join-Path $testRoot 'hello.xcx'
    [IO.File]::WriteAllText($source, '>! "Installed compiler works";', [Text.UTF8Encoding]::new($false))
    $program = Join-Path $testRoot $(if ($windows) { 'hello.exe' } else { 'hello' })
    & $compiler build $source --release -o $program
    Assert ($LASTEXITCODE -eq 0) 'Installed compiler cannot link a native program'
    $stdout = & $program
    Assert ($LASTEXITCODE -eq 0 -and $stdout -eq 'Installed compiler works') 'Built program has incorrect behavior'
    Setup uninstall $prefix $installed
    Assert (-not (Test-Path -LiteralPath $compiler)) 'Uninstall left the command behind'
    Setup uninstall $prefix
    if ($windows -and $TestUserPath) {
        Assert ([Environment]::GetEnvironmentVariable('Path', 'User') -eq $originalUserPath) 'Uninstall changed unrelated PATH entries'
    }

    $preservedPrefix = Join-Path $testRoot 'preserved prefix'
    Setup install $preservedPrefix
    $preserved = InstalledDirectory $preservedPrefix
    $userFile = Join-Path $preserved 'user-notes.txt'
    Set-Content -LiteralPath $userFile -Value 'keep this file'
    Setup uninstall $preservedPrefix
    Assert (Test-Path -LiteralPath $userFile) 'Uninstall removed an unrelated user file'
    Setup install $preservedPrefix $packageDir $false
    Assert (Test-Path -LiteralPath $userFile) 'Failed install changed an unmanaged directory'

    $incomplete = Join-Path $testRoot 'incomplete'
    New-Item -ItemType Directory -Path $incomplete | Out-Null
    $missingLibrary = if ($windows) { 'xcx.lib' } else { 'libxcx.a' }
    foreach ($file in ($expected | Where-Object { $_ -ne $missingLibrary })) {
        Copy-Item -LiteralPath (Join-Path $packageDir $file) -Destination $incomplete
    }
    $missingPrefix = Join-Path $testRoot 'must not be installed'
    Setup install $missingPrefix $incomplete $false
    Assert (-not (Test-Path -LiteralPath $missingPrefix)) 'Incomplete package created an installation'
    Write-Output 'Package tests passed: contents, README, installation, upgrade, native build, uninstall, file preservation, collision rejection, incomplete-package rejection.'
} finally {
    if ($windows -and $TestUserPath) { [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User') }
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    $expectedParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
    if ((Split-Path -Parent $resolvedTestRoot).TrimEnd('\', '/') -ne $expectedParent -or (Split-Path -Leaf $resolvedTestRoot) -notmatch '^xcx-legacy-package-test-[0-9a-f]{32}$') {
        throw "Unexpected test directory: $resolvedTestRoot"
    }
    if (Test-Path -LiteralPath $resolvedTestRoot) { Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force }
}
exit 0
