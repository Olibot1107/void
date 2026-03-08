# Void

Void is a fast scripting runtime written in Rust, plus a package ecosystem (`vpm` + web registry).
It runs as a native Rust runtime and does not require Node.js.

- [Installer](./Install.md)
- [Updater](./Update.md)
- [Uninstaller](./Uninstall.md)

## Repo Layout

- `language/` - Void runtime, parser, core modules, examples
- `package-manager/` - VPM CLI and package registry website/API
- `docs/` - quick docs for usage, language functions, and VPM

## Quick Start

### 1. Run the language runtime

```bash
cd /Users/olie/Desktop/void/language
./void
./void ./examples/hello.void
./void ./examples/main.void
```

`./void` with no args opens the interactive REPL.

### 2. Start the package registry

```bash
cd /Users/olie/Desktop/void/package-manager
./bin/void-registry
```

Open `http://127.0.0.1:4090` to create an account, log in, and publish packages.

### 3. Use VPM

```bash
cd /path/to/your-void-project
vpm
vpm init my_project
vpm search util --registry http://127.0.0.1:4090
vpm install some_pkg --registry http://127.0.0.1:4090
```

Running `vpm` with no arguments now shows the default VPM help screen.
For install-specific help, use `vpm install --help`.

## Examples

- Runtime examples: `/Users/olie/Desktop/void/language/examples`

## More Docs

- Unified docs index: `/Users/olie/Desktop/void/docs/README.md`
- Language docs: `/Users/olie/Desktop/void/language/README.md`
- Package manager docs: `/Users/olie/Desktop/void/package-manager/README.md`
