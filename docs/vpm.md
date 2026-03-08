# Package Manager (VPM)

## Start local registry

```bash
cd /Users/olie/Desktop/void/package-manager
./bin/void-registry
```

Default URL: `http://127.0.0.1:4090`

You can also start it with:

```bash
vpm server
```

Note: `vpm` only enters server mode when you explicitly use the `server` command.

## Common commands

```bash
vpm init my_project
vpm search util --registry http://127.0.0.1:4090
vpm install some_pkg --registry http://127.0.0.1:4090
vpm info some_pkg --registry http://127.0.0.1:4090
vpm uninstall some_pkg
# aliases: vpm remove some_pkg / vpm delete some_pkg / vpm rm some_pkg
```

## Maintenance / diagnostics

```bash
vpm clean
vpm clean --all
vpm doctor
vpm --verbose install some_pkg
vpm --color always info some_pkg
```

## Auth flow

```bash
vpm login your_user your_password \
  --registry http://127.0.0.1:4090
vpm whoami --registry http://127.0.0.1:4090
vpm logout --registry http://127.0.0.1:4090
```

## NPM import

```bash
vpm npm-import discord.js --as discord_js
```

Use `--install` if you also want it installed into `void_modules`.
