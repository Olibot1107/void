# Void (Rust Runtime)

Void is its own scripting runtime in Rust, with `vpm` packages plus Void-native syntax and runtime APIs.

## What is stronger now

- package-style imports from `void_modules/<name>`
- control flow: `if/else`, `while`, `repeat`
- booleans, `null`, comparisons, logical operators
- module exports via `module.exports.*`
- runtime modules:
  - `process`: args/env/cwd/chdir/exit/platform/arch/pid
  - `fs`: read/write/list/mkdir/remove
  - `path`: join/dirname/basename/extname/normalize
  - `cmd`: run shell commands
  - `http`: simple blocking GET/POST
  - `json`: parse/stringify
  - `time`: clock + sleep
  - `void`: high-res time, cpu count, ids, random, uptime
- globals:
  - `console.log(...)`, `console.error(...)`, `say(...)`, `print(...)`

## Build

```bash
cargo build --release
```

## Run

```bash
./void
./void ./examples/hello.void
./void ./examples/main.void
./void ./examples/app
./void ./examples/void_native.void
./void ./examples/hyperdrive.void
```

The `./void` launcher auto-rebuilds when source changes.

Running `./void` with no arguments starts the native Void REPL:

- color prompt
- `.help`, `.exit`, `.quit`, `.clear`
- expression result printing
- persistent variables/functions between lines

## Syntax sample

```void
use "void" as v

repeat 3 {
  console.log("id:", v.id("job"), "cpu:", v.cpu_count())
}
```

## Void-native

- `repeat N { ... }` loops a fixed number of times
- `void.id(prefix)` for unique IDs
- `void.now_us()` for microsecond clock access
- `void.cpu_count()` for CPU-aware scripts
- `void.uptime_ms()` for runtime uptime
- `void.rand()` or `void.rand(min, max)` for fast random values

## Package resolution

`use "pkg" as x` resolves in this order:

1. local/relative file paths (`./`, `../`, absolute)
2. built-in runtime modules (`process`, `fs`, `path`, `cmd`, `http`, `json`, `time`, `void`)
3. `void_modules/pkg` entry (`index.void`, `main.void`, `void.json`/`package.json`, repo entries)

Directory modules are supported. For folder imports and entrypoints, Void checks:
- `void.json` (`main`, `module`, or `entry`)
- `package.json` (`main`, `module`, or `entry`)
- fallback files: `index.void`, `main.void`, `src/index.void`, `src/main.void`
