# veridian

![build](https://github.com/vivekmalneedi/veridian/workflows/CI/badge.svg)

A WIP SystemVerilog Language Server

## Installation

- For formatting support install `verible-verilog-format` from [verible](https://github.com/google/verible)
- Install dependencies: Rust toolchain, C++17 compatible compiler

```
# clone the repo
git clone https://github.com/vivekmalneedi/veridian.git
# install using cargo
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

- diagnostics (using [slang](https://github.com/MikePopoloski/slang))
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
