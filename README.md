# veridian

![build](https://github.com/vivekmalneedi/veridian/workflows/CI/badge.svg)

A SystemVerilog Language Server

## Installation

- veridian supports these optional external tools
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

### [vscode](https://github.com/vivekmalneedi/veridian/tree/master/extensions/vscode)
- download veridian.vsix from the latest release
- install the extension using one of the two following methods
  - In the extensions tab, click on the 3 dots, then click `Install from VSIX` and choose `veridian.vsix`
  - Run `code --install-extension veridian.vsix`


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
# list of directories with header files
include_dirs:
  - inc1
  - inc2
# list of directories to recursively search for SystemVerilog/Verilog sources
source_dirs:
  - src
  - src2
```

There are other config options as well

```
# if true, recursively search the working directory for files to run diagnostics on
# default: true
auto_search_workdir: true|false,
# enable formatting with verible-verilog-format
# default: true
format: true|false,
# enable linting with Cadence HAL
# default: true
hal: true|false,
# path to verible-verilog-format binary, defaults to verible-verilog-format
# default: verible-verilog-format
verible_format_path: <path>,
# path to hal binary, defaults to hal
# default: hal
hal_path: <path>,
# set log level
# default: Info
log_level: Error|Warn|Info|Debug|Trace
```

## LSP Support

See the [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/specification-current/) for more details

- diagnostics (using [slang](https://github.com/MikePopoloski/slang) or hal)
- completion
  - identifier completion
  - dot completion
  - keywords & snippets
  - system task/function and compiler directives
- hover (documentation)
- definition
- documentSymbol
- formatting (using [verible](https://github.com/google/verible))
- rangeFormatting (using [verible](https://github.com/google/verible))
