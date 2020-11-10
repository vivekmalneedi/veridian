# veridian

![build](https://github.com/vivekmalneedi/veridian/workflows/CI/badge.svg)

A WIP SystemVerilog Language Server

## Installation

- Veridian supports these optional external tools
  - For formatting support install `verible-verilog-format` from [verible](https://github.com/google/verible)
  - Cadence Incisive HAL linter

### Install from Release

- Download the latest release for your OS from the [releases page](https://github.com/vivekmalneedi/veridian/releases)
  - The nightly release contains the last successful build, and is not guaranteed to be stable
  - The ubuntu build also includes [slang](https://github.com/MikePopoloski/slang) for linting

### Install from Source

- Build dependencies: Rust toolchain
- optional: C++17 compatible compiler (for linting with slang)

```
# clone the repo
git clone https://github.com/vivekmalneedi/veridian.git
# install with slang feature, if C++17 compiler is available
cargo install --path veridian --all-features
# install if C++17 compiler is not available
cargo install --path veridian
```

## Usage

### [coc.nvim](https://github.com/neoclide/coc.nvim)

In `coc-settings.json`:

```
{
  "languageserver": {
    "veridian": {
      "command": "veridian",
      "filetypes": ["systemverilog", "verilog"]
    }
}

```

## Configuration

Specify source directories and include directories using a yaml project config

In `veridian.yml`:

```
include_dirs:
  - inc1
  - inc2
source_dirs:
  - src
  - src2
```

## LSP Support

See the [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/specification-current/) for more details

- diagnostics (using [slang](https://github.com/MikePopoloski/slang) or hal)
- completion
  - identifier completion
  - dot completion (partially implemented)
  - keywords & keyword snippets
  - system task/function and compiler directives
- hover (documentation)
- definition
- documentSymbol
- formatting (using [verible](https://github.com/google/verible))
- rangeFormatting (using [verible](https://github.com/google/verible))
