#!/usr/bin/zsh

if (( ${+commands[rg]} )); then
    rg -i "todo"
else
    grep -i "todo" **/*.rs
fi


