# Void

Void is a fast scripting runtime written in Rust, plus a package ecosystem (`vpm` + web registry).

## Repo Layout

- `language/` - Void runtime, parser, core modules, examples
- `package-manager/` - VPM CLI and package registry website/API

## Quick Start

### 1. Run the language runtime

```bash
cd /Users/olie/Desktop/void/language
./void ./examples/hello.void
./void ./examples/main.void
```

### 2. Start the package registry

```bash
cd /Users/olie/Desktop/void/package-manager
./bin/void-registry
```

Open `http://127.0.0.1:4090` to create an account, log in, and publish packages.

### 3. Use VPM

```bash
cd /path/to/your-void-project
/Users/olie/Desktop/void/package-manager/bin/vpm init
/Users/olie/Desktop/void/package-manager/bin/vpm search util --registry http://127.0.0.1:4090
/Users/olie/Desktop/void/package-manager/bin/vpm install some_pkg --registry http://127.0.0.1:4090
```

## NPM -> Void Import

You can convert npm packages into Void/VPM-compatible packages:

```bash
cd /Users/olie/Desktop/void/language
/Users/olie/Desktop/void/package-manager/bin/vpm npm-import discord.js --as discord_js
# installs into void_modules only if you opt in:
# /Users/olie/Desktop/void/package-manager/bin/vpm npm-import discord.js --as discord_js --install
```

Default conversion output goes to `vpm-imports/<void_name>`.

If you used `--install`, then in Void:

```void
use "discord_js" as djs
console.log(djs.name, djs.version, djs.kind)
```

## Examples

- Discord bot example: `/Users/olie/Desktop/void/examples/discord-bot/README.md`

## More Docs

- Language docs: `/Users/olie/Desktop/void/language/README.md`
- Package manager docs: `/Users/olie/Desktop/void/package-manager/README.md`
