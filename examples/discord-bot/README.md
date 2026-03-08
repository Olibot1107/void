# Discord Bot Example (Void-Only + VPM)

This example is void-only. No `.js` files are used.

It uses VPM conversion to install `discord.js` metadata and then sends a Discord message through webhook from Void.

## 1) Install package with VPM

Run from this folder:

```bash
cd /Users/olie/Desktop/void/examples/discord-bot
vpm npm-import discord.js --as discord_js --install
```

That creates:

- `./void_modules/discord_js/...`

## 2) Add your Discord webhook URL

```bash
export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/...."
```

## 3) Run with Void launcher

```bash
/Users/olie/Desktop/void/language/void ./bot.void
```

## Behavior

- Sends a message to your Discord channel using webhook
- Prints package metadata from converted `discord_js` module
