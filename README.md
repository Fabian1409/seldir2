# seldir
A directory selection tui written in Rust

## shell integration
To change the current directory of the shell you need a wrapper around seldir

### fish
```fish
function sd2f
    seldir $argv
    cd (cat /tmp/seldir)
end
```
