# Void

Void is a fast scripting runtime written in Rust, plus a package ecosystem (`vpm` + web registry).

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
vpm
vpm init my_project
vpm search util --registry http://127.0.0.1:4090
vpm install some_pkg --registry http://127.0.0.1:4090
```

`vpm` now defaults to install mode help when run without arguments.

## NPM -> Void Import

You can convert npm packages into Void/VPM-compatible packages:

```bash
cd /Users/olie/Desktop/void/language
vpm npm-import discord.js --as discord_js
# installs into void_modules only if you opt in:
# vpm npm-import discord.js --as discord_js --install
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

- Unified docs index: `/Users/olie/Desktop/void/docs/README.md`
- Language docs: `/Users/olie/Desktop/void/language/README.md`
- Package manager docs: `/Users/olie/Desktop/void/package-manager/README.md`
