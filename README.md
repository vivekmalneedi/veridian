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

## LSP Support
See the [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/specification-current/) for more details
- [x] diagnostics (using [slang](https://github.com/MikePopoloski/slang))
- [x] completion
  * identifier completion
  * dot completion (partially implemented)
  * keywords & keyword snippets
  * system task/function and compiler directives
- [ ] completion resolve
- [x] hover (documentation)
- [ ] signatureHelp
- [ ] declaration
- [x] definition
- [ ] typeDefinition
- [ ] implementation
- [ ] references
- [ ] documentHighlight
- [ ] documentSymbol
- [ ] codeAction
- [ ] codeLens
- [ ] codeLens resolve
- [ ] documentLink
- [ ] documentLink resolve
- [ ] documentColor
- [ ] colorPresentation
- [ ] formatting
- [ ] rangeFormatting
- [ ] onTypeFormatting
- [ ] rename
- [ ] prepareRename
- [ ] foldingRange
- [ ] selectionRange
