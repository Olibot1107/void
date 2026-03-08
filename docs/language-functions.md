# Language and Runtime Functions

## Core syntax

```void
use "time" as time

let add = fn(a, b) {
  return a + b
}

let i = 0
while i < 3 {
  console.log("tick", i, "at", time.iso())
  i = i + 1
}

repeat 2 {
  console.error("example error line")
}
```

Supported statement forms:

- `use "module" as alias`
- `let name = expr`
- `name = expr` and `obj.prop = expr`
- `if condition { ... } else { ... }`
- `while condition { ... }`
- `repeat count { ... }`
- `return expr`
- `fn(args...) { ... }` function literals

## Global functions

- `say(...)` and `print(...)`: print values to stdout
- `console.log(...)`: print values to stdout
- `console.error(...)`: print values to stderr (red when color is supported)

## Built-in runtime modules

Import with `use "module_name" as m`.

### `time`

- `time.now_ms()`
- `time.sleep_ms(ms)`
- `time.iso()`

### `fs`

- `fs.read_text(path)`
- `fs.write_text(path, text)`
- `fs.exists(path)`
- `fs.list(path)` (path optional; defaults to current directory)
- `fs.mkdir(path)`
- `fs.mkdir_all(path)`
- `fs.remove_file(path)`
- `fs.remove_dir_all(path)`

### `path`

- `path.join(...parts)`
- `path.dirname(path)`
- `path.basename(path)`
- `path.extname(path)`
- `path.normalize(path)`

### `process`

- `process.args()` (newline-delimited args string)
- `process.argc()` (number of args)
- `process.arg(index)`
- `process.cwd()`
- `process.chdir(path)`
- `process.env(key)`
- `process.set_env(key, value)`
- `process.platform()`
- `process.arch()`
- `process.pid()`
- `process.exit(code)`

### `cmd`

- `cmd.run(command)` (stdout text)
- `cmd.status(command)` (numeric exit code)

### `http`

- `http.get(url)`
- `http.post(url, body)`

### `json`

- `json.parse(text)`
- `json.stringify(value)`

### `void`

- `void.id(prefix)`
- `void.now_us()`
- `void.cpu_count()`
- `void.uptime_ms()`
- `void.rand()`
- `void.rand(min, max)`
