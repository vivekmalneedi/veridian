# veridian

A WIP SystemVerilog Language Server

## Installation

Dependencies (from [slang](https://github.com/MikePopoloski/slang)): python 3, cmake (>=3.12), C++17 compatible compiler

```
# clone the repo
git clone --recurse-submodules https://github.com/vivekmalneedi/veridian.git
# enter the folder
cd veridian
# build veridian
cargo build --release
# put veridian on your path
sudo cp target/release/veridian /usr/local/bin
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
