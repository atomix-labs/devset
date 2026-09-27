# The README's demo on a light page; parts/demo.tape is the session. From the repository's root:
# vhs docs/demo/demo-light.tape
Output docs/src/media/demo-light.gif

Set Shell "bash"
Set FontFamily "JetBrains Mono, JetBrainsMono Nerd Font, DejaVu Sans Mono"
Set FontSize 20
Set LineHeight 1.35
Set Columns 96
Set Rows 19
Set Padding 32
# The built-in GitHub theme's bright colours are too pale on white; GitHub's own Primer palette is not.
Set Theme { "name": "Primer Light", "background": "#ffffff", "foreground": "#1f2328", "cursor": "#1f2328", "selection": "#ddf4ff", "black": "#24292f", "red": "#cf222e", "green": "#116329", "yellow": "#4d2d00", "blue": "#0969da", "magenta": "#8250df", "cyan": "#1b7c83", "white": "#6e7781", "brightBlack": "#57606a", "brightRed": "#a40e26", "brightGreen": "#1a7f37", "brightYellow": "#633c01", "brightBlue": "#218bff", "brightMagenta": "#a475f9", "brightCyan": "#1b7c83", "brightWhite": "#8c959f" }
Set TypingSpeed 35ms
Set CursorBlink false

Source docs/demo/parts/demo.tape
