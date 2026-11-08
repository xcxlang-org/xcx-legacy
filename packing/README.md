# Distribution packaging

GitHub Actions builds and tests the release compiler, then creates one archive per
platform using `packing/package.ps1`. Packaging uses PowerShell, Bash and native
archive tools; it does not use Python.

| Target | Archive | Installer |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | `xcx-legacy-<version>-x86_64-pc-windows-msvc.zip` | `install.cmd`, `uninstall.cmd` |
| `x86_64-unknown-linux-gnu` | `xcx-legacy-<version>-x86_64-unknown-linux-gnu.tar.gz` | `install.sh`, `uninstall.sh` |
| `aarch64-apple-darwin` | `xcx-legacy-<version>-aarch64-apple-darwin.tar.gz` | `install.sh`, `uninstall.sh` |

Each archive has a single `xcx-legacy/` directory containing the native compiler,
runtime static library, launcher object, `LICENSE`, a generated quick-start `README.md`, and
the platform's installation and uninstallation scripts. The Windows scripts share
`setup.ps1`. The README links to this legacy repository and modern XCX, and lists
basic commands and flags.

The runtime and launcher come from the same Cargo build as the compiler. The
packager obtains the exact launcher path from Cargo's JSON build events, avoiding
ambiguous wildcards and stale build directories. Generated output lives only under
`dist/` and is ignored by Git; package staging directories are removed afterward.

## Installation layout

| Platform | Command | Compiler and runtime |
| --- | --- | --- |
| Linux / macOS, current user | `~/.local/bin/xcx-legacy` symlink | `~/.local/lib/xcx-legacy/` |
| Linux / macOS, system-wide | `/usr/local/bin/xcx-legacy` symlink | `/usr/local/lib/xcx-legacy/` |
| Windows, current user | `%LOCALAPPDATA%\Programs\xcx-legacy\xcx-legacy.exe` | `%LOCALAPPDATA%\Programs\xcx-legacy\` |

Unix installers default to `~/.local`; `--prefix DIRECTORY` changes the prefix.
Use `sudo ./install.sh --prefix /usr/local` for a system-wide installation.
The installer reports a shell startup line if the bin directory is missing from
PATH. It does not edit shell startup files.

Windows defaults to the current user's Programs directory and adds it to the user
PATH. `-Prefix DIRECTORY` changes the installation directory; `-NoPath` skips PATH
changes. Open a new terminal after installation or uninstallation. Uninstallation
removes the PATH entry only when the installer originally added it.

Installation markers distinguish managed installations from unrelated directories.
Reinstallation upgrades a managed installation. Uninstallers remove only owned
files and the owned command link, preserving unrelated user files and shared
prefix directories. Neither installer downloads dependencies or changes the system
linker toolchain.

## Validation

`packing/tests.ps1` extracts the produced archive and checks its contents, README,
installation into a path containing spaces, reinstallation, native compilation,
execution, uninstallation, preservation of user files, rejection of directory
collisions and rejection of incomplete packages. Unix jobs also check executable
modes and shell syntax. Windows CI additionally checks user PATH add/remove behavior.
Local validation defaults to `-NoPath` and does not modify the user's PATH.

The installed compiler is exercised through the Unix command symlink. Runtime
lookup resolves the executable's canonical location before locating sidecar files.
