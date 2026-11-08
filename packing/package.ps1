param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin')]
    [string]$Target,
    [Parameter(Mandatory = $true)]
    [string]$BuildLog
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$metadataText = & cargo metadata --format-version=1 --no-deps --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo metadata' }
$metadata = $metadataText | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq 'xcx' })
if ($package.Count -ne 1) { throw 'Expected exactly one xcx package' }
$package = $package[0]
$buildEvents = @(Get-Content -LiteralPath $BuildLog | ForEach-Object { $_ | ConvertFrom-Json })
$build = @($buildEvents | Where-Object { $_.reason -eq 'build-script-executed' -and $_.package_id -eq $package.id })
if ($build.Count -eq 0) { throw 'The build log does not contain the xcx launcher build' }
$launcher = Join-Path $build[-1].out_dir 'xcx_launcher.o'
$release = Join-Path $metadata.target_directory 'release'
$windows = $Target -eq 'x86_64-pc-windows-msvc'
$compilerName = if ($windows) { 'xcx-legacy.exe' } else { 'xcx-legacy' }
$libraryName = if ($windows) { 'xcx.lib' } else { 'libxcx.a' }
$inputs = @((Join-Path $release $compilerName), (Join-Path $release $libraryName), $launcher, (Join-Path $root 'LICENSE'))
foreach ($inputFile in $inputs) {
    if (-not (Test-Path -LiteralPath $inputFile -PathType Leaf)) { throw "Missing release artifact: $inputFile" }
}
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
if ((Get-Item -LiteralPath $dist -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw 'The distribution directory cannot be a symlink or junction'
}
$staging = Join-Path $dist ('.staging-' + [guid]::NewGuid().ToString('N'))
$payload = Join-Path $staging 'xcx-legacy'
New-Item -ItemType Directory -Path $payload | Out-Null
try {
    foreach ($inputFile in $inputs) { Copy-Item -LiteralPath $inputFile -Destination $payload }
    if ($windows) {
        foreach ($file in @('install.cmd', 'uninstall.cmd', 'setup.ps1')) {
            Copy-Item -LiteralPath (Join-Path $PSScriptRoot "windows/$file") -Destination $payload
            if ($file.EndsWith('.cmd')) {
                $commandPath = Join-Path $payload $file
                $commandText = [IO.File]::ReadAllText($commandPath)
                [IO.File]::WriteAllText($commandPath, [regex]::Replace($commandText, '\r?\n', "`r`n"), [Text.UTF8Encoding]::new($false))
            }
        }
        $install = @'
Run `install.cmd`. The default location is `%LOCALAPPDATA%\Programs\xcx-legacy`.
The installer adds this directory to your user PATH; open a new terminal afterward.
No administrator rights are needed. To choose a directory or skip PATH changes:

```bat
install.cmd -Prefix "D:\Tools\xcx-legacy"
install.cmd -NoPath
```
'@
        $uninstall = 'Run `uninstall.cmd` from the installed directory or this package. For a custom installation use `uninstall.cmd -Prefix "D:\Tools\xcx-legacy"`. The installer removes only the PATH entry it added and preserves additional user files.'
        $toolchain = 'MSVC Build Tools (Desktop development with C++) and the Windows SDK'
    } else {
        foreach ($file in @('install.sh', 'uninstall.sh')) {
            Copy-Item -LiteralPath (Join-Path $PSScriptRoot "unix/$file") -Destination $payload
        }
        & chmod 755 (Join-Path $payload $compilerName) (Join-Path $payload 'install.sh') (Join-Path $payload 'uninstall.sh')
        if ($LASTEXITCODE -ne 0) { throw 'Cannot set executable permissions' }
        $install = @'
Run `./install.sh`. The default prefix is `~/.local`: the command is linked in
`~/.local/bin`, while compiler files live in `~/.local/lib/xcx-legacy`.
If that bin directory is not on PATH, the installer prints the line to add to your
shell startup file (`~/.bashrc` on Bash or `~/.zshrc` on Zsh).

```sh
./install.sh
./install.sh --prefix "$HOME/tools"
sudo ./install.sh --prefix /usr/local
```

The last command installs system-wide in `/usr/local/bin` and `/usr/local/lib`.
'@
        $uninstall = 'Run `~/.local/lib/xcx-legacy/uninstall.sh`, or `./uninstall.sh --prefix "$HOME/tools"` for a custom prefix. For a system-wide installation use `sudo /usr/local/lib/xcx-legacy/uninstall.sh`. Shared prefix directories, shell configuration and additional user files are preserved.'
        $toolchain = if ($Target -like '*apple*') { 'Xcode Command Line Tools (`xcode-select --install`)' } else { 'GCC and the system development libraries (for example, the `build-essential` package on Debian/Ubuntu)' }
    }
    $readme = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'README-package.md') -Raw
    $readme = $readme.Replace('@VERSION@', $package.version).Replace('@TARGET@', $Target)
    $readme = $readme.Replace('@INSTALL@', $install).Replace('@UNINSTALL@', $uninstall).Replace('@TOOLCHAIN@', $toolchain)
    [IO.File]::WriteAllText((Join-Path $payload 'README.md'), $readme, [Text.UTF8Encoding]::new($false))
    $extension = if ($windows) { 'zip' } else { 'tar.gz' }
    $archive = Join-Path $dist "xcx-legacy-$($package.version)-$Target.$extension"
    if ($windows) {
        Compress-Archive -LiteralPath $payload -DestinationPath $archive -CompressionLevel Optimal -Force
    } else {
        & tar -czf $archive -C $staging xcx-legacy
        if ($LASTEXITCODE -ne 0) { throw 'Cannot create the distribution archive' }
    }
    Write-Output "Packaged: $archive"
} finally {
    $resolvedStaging = [IO.Path]::GetFullPath($staging)
    if ((Split-Path -Parent $resolvedStaging) -ne [IO.Path]::GetFullPath($dist) -or (Split-Path -Leaf $resolvedStaging) -notmatch '^\.staging-[0-9a-f]{32}$') {
        throw "Unexpected staging directory: $resolvedStaging"
    }
    if (Test-Path -LiteralPath $resolvedStaging) { Remove-Item -LiteralPath $resolvedStaging -Recurse -Force }
}
