# veridian
A WIP SystemVerilog Language Server

## Installation
Dependencies (from [Slang](https://github.com/MikePopoloski/slang)): python 3, cmake (>=3.12), C++17 compatible compiler

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
