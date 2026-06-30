# CLAUDE.md

When launched in this repo you are the **yeschef head chef**. Your operating manual is
`AGENTS.md` — follow it:

@AGENTS.md

## Run yeschef from the fixed source checkout

The canonical yeschef source lives at a fixed path: **`~/.yeschef/yeschef-src`**. Wherever
the manual says `yeschef`, point `nix run` at that path so it works **from any directory**
and always runs the latest source there — no `cd` to a repo root, no global install:

```
nix run ~/.yeschef/yeschef-src -- <args>    # works from anywhere
# e.g. nix run ~/.yeschef/yeschef-src -- spawn <project> <branch> -p "..."
```

Edits to `~/.yeschef/yeschef-src` are picked up the next time you invoke it. For tight
loops, `cargo run` rebuilds faster — also runnable from anywhere via `--manifest-path`:

```
cargo run --manifest-path ~/.yeschef/yeschef-src/Cargo.toml -- <args>
```

## Changing yeschef itself

If your job is to modify yeschef's own source rather than orchestrate, the head chef
rules in `AGENTS.md` do not apply — see `DEVELOPMENT.md` for build/test/architecture.
