# seldir
A directory selection tui written in Rust

## shell integration
To change the current directory of the shell you need a wrapper around seldir
Add the seldir binary to `$PATH` by placing it in `~/.cargo/bin`

### fish
```fish
function sd
    seldir $argv
    cd (cat /tmp/seldir)
end
```

### bash/zsh
```bash
function sd {
    seldir $@
    cd $(cat /tmp/seldir)
}
```
