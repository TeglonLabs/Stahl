# Stahl language server

## Installation

From the directory of this crate, run:

```
cargo install --path .
```

This will install the `stahl-language-server` to your path.

Configuration in helix can be done by adding the following to your `languages.toml` file:

```toml
[[language]]
name = "scheme"
language-servers = [ "stahl-language-server" ]

[language-server.stahl-language-server]
command = "stahl-language-server"
args = []
```

Configuration in neovim can be done by adding the following to your configuration

```lua
configs.stahl = {
    default_config = {
        cmd = { "stahl-language-server" },
        filetypes = { "scheme" },
        root_dir = function(_)
            -- We can run on single file, so root dir doesn't really matter.
            require("lspconfig.util").root_pattern "*"
        end,
        single_file_support = true,
        docs = {
            description = [[
Stahl language server.

Install from: https://github.com/TeglonLabs/Stahl/tree/master/crates/stahl-language-server

Check the repo for more configuration.
]],
            default_config = {}
        }
    }
}
```

Now you can call

```lua
require'lspconfig'.stahl.setup {
  -- Your usual configuration (handlers, capabilities, etc.) goes here.
}
```

VSCode, and emacs installation instructions coming soon.

## Configuration

If you're embedding stahl within a host application, it is possible the language server does not have knowledge
of the globals given by the host application. To make these available to the language server, create a configuration file
for the language server under the path specified by the env var `STAHL_LSP_HOME`.

For example, I have set my `STAHL_LSP_HOME` to `~/.config/stahl-lsp/` and added the file `globals.scm`:

```scheme

(define keymaps (#%module "helix/core/keymaps"))

(define (register-values module values)
  (map (lambda (ident) (#%module-add module (symbol->string ident) void)) values))

(register-values keymaps
                 '(helix-current-keymap *buffer-or-extension-keybindings*
                                        *reverse-buffer-map*
                                        helix-merge-keybindings
                                        helix-string->keymap
                                        *global-keybinding-map*
                                        helix-deep-copy-keymap))

(define typable-commands (#%module "helix/core/typable"))
(define static-commands (#%module "helix/core/static"))
(define editor (#%module "helix/core/editor"))

(register-values typable-commands '())

(#%ignore-unused-identifier "_")

keymaps

```

The following rules apply: Any global value that is defined as a builtin module, will be interpreted by the LSP and used, or evaluated as a top level env var. So either defining `keymaps` or evaluating like so on the last line of the example will get the module to show up.

Any symbol added with `#%ignore-unused-identifier` will escape that symbol from being shown as an unused identifier.

Any global registered with `#%register-global` will be ignored as a free identifier as well.

