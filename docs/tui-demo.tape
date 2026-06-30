# Proof recording: launch the yeschef TUI, let it render, then quit.
#
# Headless render:  vhs docs/tui-demo.tape   (inside `nix develop`)
# Produces docs/tui-demo.gif — embedded in the PR as proof VHS works.

Output docs/tui-demo.gif

Set FontSize 14
Set Width 1000
Set Height 600
Set Padding 10

# Build is already done, so --quiet keeps the recording free of cargo chatter.
Type "cargo run --quiet -- tui"
Enter
Sleep 4s

# Quit cleanly with the TUI's own quit key (see src/commands/tui.rs).
Type "q"
Sleep 1s
