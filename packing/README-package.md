# xcx-legacy @VERSION@

Native AOT compiler for historical XCX 1.2. Platform: @TARGET@.

- Legacy source and documentation: https://github.com/xcxlang-org/xcx-legacy
- Modern XCX: https://github.com/xcxlang-org/xcx
- Modern XCX website: https://xcxlang.com/

## Install

@INSTALL@

Compiler, runtime library and launcher object must stay together. The installer
keeps all three files in the installation directory. You can also use the extracted
compiler directly without installing it.

Compiling XCX programs requires @TOOLCHAIN@. Rust is not needed to use this package.

## Commands

Create `hello.xcx` containing:

```text
>! "Hello from XCX 1.2";
```

```text
xcx-legacy run hello.xcx
xcx-legacy run hello.xcx --release
xcx-legacy build hello.xcx --release
xcx-legacy build hello.xcx --release --opt-level=size
xcx-legacy --help
```

| Option | Purpose |
| --- | --- |
| `--debug` | Default: optimization disabled, compiler verification enabled |
| `--release` | Optimize native code and remove unused code during linking |
| `--opt-level none\|speed\|size` | Override optimization; shortcuts: `-O0`, `-O`, `-Os` |
| `--cpu generic\|native` | Architecture baseline (default) or host CPU instructions |
| `-o PATH` | Set the exact build output filename |
| `--emit exe\|obj` | Build an executable (default) or an object without linking |
| `--timings` | Print compilation stages to stderr, excluding program execution |
| `--verify`, `--frame-pointers` | Enable additional release verification or profiling support |
| `-q`, `--quiet` | Suppress compiler success messages |

Options may appear before or after the source path. `run` compiles a temporary
native executable and starts it. `build` leaves a standalone executable in the
current directory. Custom output and object emission are available only for `build`.

## Uninstall

@UNINSTALL@

## License

Licensed under the [Apache License 2.0](LICENSE).
