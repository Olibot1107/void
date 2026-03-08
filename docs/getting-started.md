# Getting Started

## 1. Run the language

```bash
cd /Users/olie/Desktop/void/language
./void ./examples/hello.void
./void ./examples/main.void
```

Pass script args:

```bash
./void ./examples/main.void first second third
```

## 2. Error output is colorized

Runtime errors and `console.error(...)` output are printed in red when your terminal supports ANSI colors.

## 3. Start the package registry

```bash
cd /Users/olie/Desktop/void/package-manager
./bin/void-registry
```

Open `http://127.0.0.1:4090`.

## 4. Use VPM

```bash
cd /path/to/your-void-project
vpm
vpm init my_project
vpm search util --registry http://127.0.0.1:4090
vpm install some_pkg --registry http://127.0.0.1:4090
```

`vpm` with no args shows the default help screen. Use `vpm install --help` for install-specific options.
