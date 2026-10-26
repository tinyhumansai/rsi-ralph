# Installing RSI Ralph

The [README](README.md) has a prompt that has a coding agent install and set up
RSI Ralph. This page is the manual route: the one-line install for each
platform, pinning a version, choosing a folder, the container image, building
from source and upgrading.

## Prebuilt binaries

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex
```

The installer picks the build for your machine from the latest
[release](https://github.com/tinyhumansai/rsi-ralph/releases), checks its
SHA-256, and puts `rsi-ralph` in `~/.local/bin` (on Windows,
`%LOCALAPPDATA%\Programs\rsi-ralph\bin`, which it adds to your user `PATH`).
It does not use `sudo`.

Prebuilt binaries exist for Linux (x86_64 and arm64, glibc 2.35+), macOS (Intel
and Apple silicon) and Windows (x86_64, which also runs on Windows on Arm).

### Pin a version or choose the folder

Pass flags through the pipe:

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh \
  | sh -s -- --version v0.4.0 --dir /usr/local/bin
```

### Upgrade

Run the installer again.

## Container image

If you would rather not install anything, use the container image:

```sh
docker run --rm -it -v "$PWD:/work" -w /work -v rsi-ralph-home:/data \
  -e OPENROUTER_API_KEY ghcr.io/tinyhumansai/rsi-ralph doctor
```

Runs are kept in the `/data` volume. The UI only listens on loopback, so add
`--network host` to reach it from the host, or pass `--no-serve` to `run`.

## Build from source

You need Rust 1.96 or newer and the submodules:

```sh
git clone --recurse-submodules https://github.com/tinyhumansai/rsi-ralph
cd rsi-ralph && cargo install --path crates/rsi-ralph-cli
```

## Shell completions

`rsi-ralph completions <shell>` prints a script for bash, zsh, fish,
PowerShell or elvish. For zsh, add `source <(rsi-ralph completions zsh)` to
`~/.zshrc`.

## Where runs are stored

Runs live in `~/.tinyrsi/runs/<loop>/<run-id>/`. Set `TINYRSI_HOME` or pass
`--home` to keep them somewhere else.

## Check the install

```sh
rsi-ralph doctor    # git, diff, python3 and your API key
```
