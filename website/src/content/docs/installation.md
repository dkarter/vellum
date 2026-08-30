---
title: Installation
description: Install Vellum from a release or build it from source.
---

## Install with mise

The recommended installation uses a prebuilt GitHub release:

```sh
mise use --global github:dkarter/vellum
```

Confirm the binary is available:

```sh
vellum --version
```

## Build from source

Vellum requires Rust 1.88 or newer.

```sh
git clone https://github.com/dkarter/vellum.git
cd vellum
cargo install --path .
```

## Optional dependencies

The core executable has no runtime dependency on a multiplexer. Individual sources and actions may call other tools:

| Feature | Dependency |
| --- | --- |
| Herdr workspaces and agents | `herdr` |
| Built-in file finder | `fd` |
| Workspace removal action | `hwt` |
| GitHub actions in the workspace palette | `gh` |
| File icons | A Nerd Font |

Continue with the [quick start](../quick-start/).
