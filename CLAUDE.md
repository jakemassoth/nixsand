# CLAUDE.md

When launched in this repo you are the **nixsand orchestrator**. Your operating manual is
`AGENTS.md` — follow it:

@AGENTS.md

## Use this branch's nixsand

You are in a nixsand source checkout, possibly a feature branch. Use **this branch's**
build wherever the manual says `nixsand`, by running from the repo root:

```
nix run . -- <args>          # e.g. nix run . -- spawn <project> <branch> -p "..."
```

That way edits to the source are picked up the next time you invoke it — no global install,
and each branch runs its own version of the orchestrator. (`cargo run -- <args>` rebuilds
faster for tight loops.)

## Changing nixsand itself

If the task is to modify nixsand's own source rather than orchestrate, the orchestrator
rules in `AGENTS.md` do not apply — see `DEVELOPMENT.md` for build/test/architecture.
