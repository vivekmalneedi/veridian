# veridian

![build](https://github.com/vivekmalneedi/veridian/workflows/CI/badge.svg)
![GitHub](https://img.shields.io/github/license/vivekmalneedi/veridian)

A SystemVerilog Language Server\
<a href="https://asciinema.org/a/374859" target="_blank"><img src="https://asciinema.org/a/374859.svg" width="500"/></a>

## Installation

### Pre-Installation

- It is recommended to install the [verible](https://github.com/google/verible) tools for
  - formatting support with `verible-verilog-format`
  - syntax checking support with `verible-verilog-syntax`
- It is recommended to install [verilator](https://www.veripool.org/verilator/) for additional linting

### Install from Release

- Download the latest release for your OS from the [releases page](https://github.com/vivekmalneedi/veridian/releases)
  - The nightly release contains the last successful build, and is not guaranteed to be stable
  - The ubuntu build also includes [slang](https://github.com/MikePopoloski/slang) for linting

### Install from Source

- Build dependencies: Rust toolchain (Install through system package manager or through [rustup](https://rustup.rs/]))
- optional: C++17 compatible compiler (for linting with slang)

```bash
# install with slang feature, if C++17 compiler is available
cargo install --git https://github.com/vivekmalneedi/veridian.git --all-features
# install if C++17 compiler is not available
cargo install --git https://github.com/vivekmalneedi/veridian.git
```

## Usage

### [neovim]
```lua
local lspconfutil = require 'lspconfig/util'
local root_pattern = lspconfutil.root_pattern("veridian.yml", "veridian.yaml", ".git")
require('lspconfig').veridian.setup {
    cmd = { 'veridian' },
    root_dir = function(fname)
        local filename = lspconfutil.path.is_absolute(fname) and fname
        or lspconfutil.path.join(vim.loop.cwd(), fname)
        return root_pattern(filename) or lspconfutil.path.dirname(filename)
    end;
}
````

### [vscode](https://github.com/vivekmalneedi/veridian/tree/master/extensions/vscode)

- download veridian.vsix from the latest release
- install the extension using one of the two following methods
  - In the extensions tab, click on the 3 dots, then click `Install from VSIX` and choose `veridian.vsix`
  - Run `code --install-extension veridian.vsix`

### [coc.nvim](https://github.com/neoclide/coc.nvim)

In `coc-settings.json`:

```json
{
  "languageserver": {
    "veridian": {
      "command": "veridian",
      "filetypes": ["systemverilog", "verilog"]
    }
}

```

### Emacs

- Install the [`verilog-ext`](https://github.com/gmlarumbe/verilog-ext/) package
- Copy the following snippet into your init file:

```elisp
(require 'verilog-ext)
(verilog-ext-mode-setup)
(verilog-ext-eglot-set-server 've-veridian) ;`eglot' config
(verilog-ext-lsp-set-server 've-veridian)   ; `lsp' config
```

The [full list](https://github.com/vivekmalneedi/veridian/wiki/Usage-Instructions-for-various-LSP-Clients) is on the wiki

## Configuration

- Specify source directories and include directories using a yaml project config
- All settings have defaults so your config file should only specify custom values

In `veridian.yml`:

```yaml
# list of directories with header files
include_dirs:
  - inc1
  - inc2
# list of directories to recursively search for SystemVerilog/Verilog sources
source_dirs:
  - src
  - src2
# if true, recursively search the working directory for files to run diagnostics on
# default: true
auto_search_workdir: true|false,
# verible tool configuration
verible:
  # verible-verilog-syntax configuration
  syntax:
    # default: true if in path
    enabled: true|false,
    path: "verible-verilog-syntax"
    # default: none
    args:
      - arg1
      - arg2
  # verible-verilog-format configuration
  format:
    # default: true if in path
    enabled: true|false,
    path: "verible-verilog-format"
    # default: none
    args:
      - arg1
      - arg2
verilator:
  # verible-verilog-syntax configuration
  syntax:
    # default: true if in path
    enabled: true|false,
    path: "verilator"
    # default: specified below
    args:
      - --lint-only
      - --sv
      - --timing
      - -Wall
# set log level
# default: Info
log_level: Error|Warn|Info|Debug|Trace
```

## LSP Support

See the [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/specification-current/) for more details

- diagnostics (using [slang](https://github.com/MikePopoloski/slang) or [verible](https://github.com/google/verible))
- completion
  - identifier completion
  - dot completion
  - keywords & snippets
  - system task/function and compiler directives
- hover (documentation)
- definition
- documentSymbol
- documentHighlight
- formatting (using [verible](https://github.com/google/verible))
- rangeFormatting (using [verible](https://github.com/google/verible))

## Alternatives
The Verible project is working on a language server for SystemVerilog, check it out [here](https://github.com/chipsalliance/verible/tree/master/verilog/tools/ls)
