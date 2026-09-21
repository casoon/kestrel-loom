---
title: Installation
description: kestrel-loom is not published to crates.io or npm. You build the WebAssembly package from the repository, or depend on the Rust core through Git.
order: 1
---

## Requirements

- Rust 1.87 or newer with the `wasm32-unknown-unknown` target
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Build the WebAssembly package

```sh
git clone https://github.com/casoon/kestrel-loom
cd kestrel-loom
./build-wasm.sh
```

`build-wasm.sh` runs `wasm-pack build --target web` in `crates/wasm` and writes the package to
`pkg/`. What you embed in a page is two things side by side:

```
js/kestrel-loom.js            the JavaScript wrapper
pkg/kestrel_loom_wasm.js      the wasm-bindgen glue
pkg/kestrel_loom_wasm_bg.wasm the compiled chart
```

The wrapper imports `../pkg/kestrel_loom_wasm.js`, so keep that relative layout when you copy
the files into your project.

## Look at the demo locally

```sh
python3 -m http.server 8777     # then open http://localhost:8777/demo/
```

Any static file server works; the page needs to be served over HTTP, not opened as a file,
because the browser loads the `.wasm` file with `fetch`.

## Use the Rust core

The core crate `kestrel-loom` has no browser dependency and can be used on its own, for
example to produce and test render command streams. Depend on it through Git and pin a commit:

```toml
[dependencies]
kestrel-loom = { git = "https://github.com/casoon/kestrel-loom", rev = "<commit>" }
```

It pulls in kestrel-chartkit, pinned to a release tag in the workspace manifest.

## Next steps

Continue with the [Quickstart](../quickstart/) to put a chart on a page.
