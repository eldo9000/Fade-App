# Fade

Convert and process images, video, and audio.

Part of the Libre app family — see https://github.com/eldo9000/Libre-Apps for shared tooling and standards.

## Development

```
npm install
npm run tauri dev
```

## Build

```
npm run tauri build
```

## Shared code

- `common-js/` is a **vendored snapshot** of `Libre-Apps/common-js` (the `@libre/ui` package). It is wired in via `"@libre/ui": "file:./common-js"` in `package.json`. When the upstream package changes, sync it manually from the Libre-Apps monorepo.
- Rust shared code comes from `librewin-common`, referenced as a Cargo git dependency pinned to a specific SHA in the Libre-Apps repo (`src-tauri/Cargo.toml`).
