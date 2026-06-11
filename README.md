# Lambda Finder — Tauri 2 edition

A Rust + Tauri 2 rewrite of the Electron `lambda-finder` app. Same UI, same
features, much smaller binary (~10–15 MB vs ~110 MB), uses the system WebView
instead of bundling Chromium.

Features (parity with the Electron version):

1. **Pull Latest Branches** — pick a branch, runs `git fetch / checkout / pull`
   across the configured repos with streaming output.
2. **Find Lambda / Logs URL** — searches local YAML templates, resolves the
   real Lambda + log group name (handles both `envStackname-` and
   `!Sub ${EnvironmentValue}` conventions), opens them in the default browser.

## Prerequisites

### One-time system deps (Ubuntu 24.04)

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev \
  build-essential curl wget file libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

### Rust toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### Tauri CLI (one of these)

```bash
# As an npm devDep (already wired into package.json):
npm install

# Or globally via cargo:
cargo install tauri-cli --version "^2"
```

## Run from source

```bash
cd lambda-finder
npm install
npm run dev          # hot-reload dev window
```

## Build a distributable

```bash
npm run build:linux   # produces .deb and .AppImage in src-tauri/target/release/bundle/
```

## In-app auto updates

The app shows an "Update available" toast at the bottom-right when a newer
version has been published. Clicking it downloads, signature-verifies,
replaces the running AppImage in-place, and restarts.

### One-time setup (per project)

1. **Generate the signing keypair** (private key stays on your machine):

   ```bash
   npm run updater:keygen
   ```

   This writes `~/.tauri/lambda-finder.key` (private) and
   `~/.tauri/lambda-finder.key.pub` (public).

2. **Wire the public key into `src-tauri/tauri.conf.json`**: replace
   `REPLACE_WITH_BASE64_PUBLIC_KEY` with the contents of the `.pub` file.

3. **Wire the GitHub repo into `src-tauri/tauri.conf.json`**: replace
   `REPLACE_OWNER/REPLACE_REPO` in the `endpoints` URL with your real
   `owner/repo`.

### Releasing a new version

1. Bump `version` in both `package.json` and `src-tauri/tauri.conf.json`.
2. Export the signing key for this shell:

   ```bash
   export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/lambda-finder.key)"
   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # or your password
   ```

3. Build + sign + assemble release manifest:

   ```bash
   npm run release -- "Notes for this release"
   ```

   Outputs three files in `bundle/release/`:
   - `Lambda Finder_<ver>_amd64.AppImage`
   - `Lambda Finder_<ver>_amd64.AppImage.sig`
   - `latest.json`

4. Tag, push, and publish a GitHub Release with all three files attached:

   ```bash
   git tag v<ver> && git push --tags
   gh release create v<ver> bundle/release/* \
       --title "v<ver>" --latest
   ```

5. Existing installs will see the toast on next launch.

> **Dev note:** in `npm run dev` the app version is identical to the latest
> published version (or the bundle is unsigned), so the toast never appears.
> The check fails silently — that's expected.

## File layout

- `src-tauri/` — Rust backend
  - `src/lib.rs` — Tauri command handlers (`get_config`, `set_config`, `find_lambda`, `start_pull`)
  - `src/lambda_finder.rs` — YAML grep + resolve logic (port of `lib/lambda-finder.js`)
  - `src/pull_branch.rs` — git pull workflow with streaming events (port of `lib/pull-branch.js`)
  - `src/config.rs` — persistent repo-root config
  - `tauri.conf.json` — window, identifier, bundle, icons
- `index.html`, `src/styles.css`, `src/main.js` — frontend (same UI as v1)

## Notes

- The "Find" feature shells out to `grep -i -r -l --include='*.yml' --include='*.yaml' ...`
  inside the configured repo root. `grep` must be on `PATH`.
- The "Pull" feature shells out to `git fetch / checkout / pull`. `git` must be on `PATH`.
- Region hard-coded to `ap-south-1` in `src-tauri/src/lambda_finder.rs`.
- `utec-microservices` is intentionally excluded from the pull list (matching `makepull.sh`).
