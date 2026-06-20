#!/usr/bin/zsh

if (( ${+commands[rg]} )); then
    rg -i "todo" --glob "**/*.rs"
else
    grep -i "todo" **/*.rs
fi


