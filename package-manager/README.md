# Void Package Manager

This folder contains an npm-like package ecosystem for Void:

- `registry/`: web registry + HTTP API with account login and authenticated publishing
- `registry/templates/index.html`: website HTML template (not hardcoded in Rust)
- `vpm/`: command-line package manager (Void Package Manager)
- `bin/void-registry`: launcher for the registry binary
- `bin/vpm`: launcher for the package manager binary

## Rules

- Publishing and uploads require an account/login.
- Public install/search/list endpoints do not require login.

## Quick start

Start the registry website/API:

```bash
cd /Users/olie/Desktop/void/package-manager
./bin/void-registry
```

Open `http://127.0.0.1:4090`, create an account, login, and publish.

## Publish sources

A package can be published from either:

- GitHub repo URL (`https://github.com/owner/repo`)
- Uploaded file (`.tgz`, `.zip`, etc.)
- Tarball URL

## CLI

Use `vpm` in any Void project:

```bash
cd /path/to/your-void-project
vpm
vpm init
```

Running `vpm` with no arguments now opens install-focused help (`install` mode by default).

If you use the repo launcher directly, keep it pointed at
`vpm` so it can find the project root.

Login from CLI (token is saved for that registry):

```bash
vpm login your_user your_password \
  --registry http://127.0.0.1:4090
vpm whoami --registry http://127.0.0.1:4090
```

Manual token fetch still works:

```bash
TOKEN=$(curl -sS -X POST http://127.0.0.1:4090/api/login \
  -H 'content-type: application/json' \
  -d '{"username":"your_user","password":"your_password"}' \
  | sed -E 's/.*"token":"([^"]+)".*/\1/')
```

Publish JSON payload (manifest fields):

```bash
vpm publish \
  --registry http://127.0.0.1:4090
```

Publish with file upload:

```bash
vpm publish \
  --registry http://127.0.0.1:4090 \
  --file ./my-package.tgz
```

Override GitHub repo at publish time:

```bash
vpm publish \
  --registry http://127.0.0.1:4090 \
  --github https://github.com/owner/repo
```

Logout:

```bash
vpm logout --registry http://127.0.0.1:4090
```

Install and search stay public:

```bash
vpm search util --registry http://127.0.0.1:4090
vpm install some_pkg --registry http://127.0.0.1:4090
vpm install some_pkg --version 1.2.3 --registry http://127.0.0.1:4090
vpm info some_pkg --registry http://127.0.0.1:4090
vpm info some_pkg --version 1.2.3 --readme --registry http://127.0.0.1:4090
vpm list
vpm remove some_pkg
```

Convert npm package -> Void/VPM-ready package:

```bash
vpm npm-import discord.js
# optional:
# vpm npm-import @discordjs/builders --version 1.9.0 --as discord_builders
# install into void_modules (opt-in):
# vpm npm-import discord.js --as discord_js --install --registry http://127.0.0.1:4090 --token "$TOKEN"
```

This command:

- downloads package from npm registry
- caches converted output by npm package + version and reuses it on later imports
- reuses the already-converted version from `package.json` when run again without `--version`
- when `--install` is used, reads current package versions from your website registry API
- when `--install` imports a new version, it publishes to website registry
- if `--token`/`VPM_TOKEN` is present, publish is attributed to your account
- if no token is provided, npm imports publish through the guest npm-import endpoint (`author: npm_ghost`)
- converts into `vpm-imports/<void_name>` by default
- extracts source into `<converted_dir>/npm/package` and converts JS/TS files to `.void`
- does not install into `void_modules` unless `--install` is used
- does not keep `.js` runtime bridge files
- generates a Void wrapper (`index.void`) plus `void.json` and `voidpkg.toml`
- updates `void.lock` only when `--install` is used

## API

- `GET /`: registry website
- `GET /packages/:name`: package detail page (README + version history + install commands)
- `GET /packages/:name/:version`: package detail page for a specific version
- `GET /uploads/:file`: uploaded package files
- `POST /register`: create account
- `POST /login`: login (session cookie)
- `POST /logout`: logout
- `POST /publish`: publish from website form (requires session)
- `POST /api/login`: API login, returns bearer token
- `POST /api/publish`: authenticated JSON publish
- `POST /api/publish/npm-import`: guest npm-import JSON publish (no auth; restricted to npm-derived package names)
- `POST /api/publish/upload`: authenticated multipart publish (file upload)
- `GET /api/packages`: list latest versions
- `GET /api/packages/:name/:version`: fetch one package version (used for exact-version install)
- `GET /api/packages/:name`: list all versions of package
- `GET /api/search?q=...`: search packages

## Environment variables

- `VOID_BUILD_PROFILE`: launcher build profile for `bin/void-registry` and `bin/vpm` (`dev` default, set to `release` for optimized builds)
- `VPM_TOKEN`: optional default API token used by `vpm npm-import --install` when `--token` is omitted
- `VPM_AUTH_FILE`: optional path for saved CLI login sessions (default `$HOME/.vpm/auth.json`)
- `VPM_CACHE_DIR`: base cache directory for npm imports (default `$XDG_CACHE_HOME/vpm` or `$HOME/.cache/vpm`)
- `VOID_REGISTRY_ADDR`: bind address (default `127.0.0.1:4090`)
- `VOID_REGISTRY_PUBLIC_URL`: base URL used for uploaded file links (default derived from addr)
- `VOID_REGISTRY_DB`: sqlite DB path (default `registry.db`)
- `VOID_REGISTRY_UPLOADS`: upload storage directory (default `uploads`)
