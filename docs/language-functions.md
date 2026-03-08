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

Core operators include: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`.

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

### `math`

- `math.pi()`
- `math.tau()`
- `math.e()`
- `math.abs(x)`
- `math.sqrt(x)`
- `math.pow(base, exponent)`
- `math.sin(radians)`
- `math.cos(radians)`
- `math.tan(radians)`
- `math.asin(x)`
- `math.acos(x)`
- `math.atan(x)`
- `math.atan2(y, x)`
- `math.floor(x)`
- `math.ceil(x)`
- `math.round(x)`
- `math.min(a, b, ...)`
- `math.max(a, b, ...)`
- `math.clamp(value, min, max)`
- `math.lerp(start, end, t)`
- `math.deg_to_rad(degrees)`
- `math.rad_to_deg(radians)`

### `array`

- `array.len(array)`
- `array.get(array, index)` (returns `null` if missing)
- `array.set(array, index, value)` (grows `length` as needed)
- `array.push(array, value)` (returns new length)
- `array.pop(array)` (returns popped value or `null`)
- `array.clear(array)`

### `object`

- `object.get(object, key)` (returns `null` if missing)
- `object.set(object, key, value)`
- `object.has(object, key)`
- `object.remove(object, key)` (returns bool)
- `object.keys(object)` (returns an array of keys)
