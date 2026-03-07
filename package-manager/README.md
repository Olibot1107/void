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
/Users/olie/Desktop/void/package-manager/bin/vpm init
```

Login and get a token for CLI publish:

```bash
TOKEN=$(curl -sS -X POST http://127.0.0.1:4090/api/login \
  -H 'content-type: application/json' \
  -d '{"username":"your_user","password":"your_password"}' \
  | sed -E 's/.*"token":"([^"]+)".*/\1/')
```

Publish JSON payload (manifest fields):

```bash
/Users/olie/Desktop/void/package-manager/bin/vpm publish \
  --registry http://127.0.0.1:4090 \
  --token "$TOKEN"
```

Publish with file upload:

```bash
/Users/olie/Desktop/void/package-manager/bin/vpm publish \
  --registry http://127.0.0.1:4090 \
  --token "$TOKEN" \
  --file ./my-package.tgz
```

Override GitHub repo at publish time:

```bash
/Users/olie/Desktop/void/package-manager/bin/vpm publish \
  --registry http://127.0.0.1:4090 \
  --token "$TOKEN" \
  --github https://github.com/owner/repo
```

Install and search stay public:

```bash
/Users/olie/Desktop/void/package-manager/bin/vpm search util --registry http://127.0.0.1:4090
/Users/olie/Desktop/void/package-manager/bin/vpm install some_pkg --registry http://127.0.0.1:4090
```

Convert npm package -> Void/VPM-ready package:

```bash
/Users/olie/Desktop/void/package-manager/bin/vpm npm-import discord.js
# optional:
# /Users/olie/Desktop/void/package-manager/bin/vpm npm-import @discordjs/builders --version 1.9.0 --as discord_builders
```

This command:

- downloads package from npm registry
- converts into `void_modules/<void_name>`
- extracts source into `void_modules/<void_name>/npm/package`
- runs `npm install --omit=dev --legacy-peer-deps` inside converted package (when npm is available)
- generates a Void wrapper (`index.void`) plus `void.json` and `voidpkg.toml`
- updates `void.lock`

## API

- `GET /`: registry website
- `GET /uploads/:file`: uploaded package files
- `POST /register`: create account
- `POST /login`: login (session cookie)
- `POST /logout`: logout
- `POST /publish`: publish from website form (requires session)
- `POST /api/login`: API login, returns bearer token
- `POST /api/publish`: authenticated JSON publish
- `POST /api/publish/upload`: authenticated multipart publish (file upload)
- `GET /api/packages`: list latest versions
- `GET /api/packages/:name`: list all versions of package
- `GET /api/search?q=...`: search packages

## Environment variables

- `VOID_REGISTRY_ADDR`: bind address (default `127.0.0.1:4090`)
- `VOID_REGISTRY_PUBLIC_URL`: base URL used for uploaded file links (default derived from addr)
- `VOID_REGISTRY_DB`: sqlite DB path (default `registry.db`)
- `VOID_REGISTRY_UPLOADS`: upload storage directory (default `uploads`)
