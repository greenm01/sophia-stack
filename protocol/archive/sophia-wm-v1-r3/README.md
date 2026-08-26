# Archived `sophia_wm_v1` Revision 3 Client

This directory is an immutable compatibility snapshot of the independent C99
client and codec at the revision-3 freeze candidate. It deliberately does not
include or link the current generated binding. The compatibility gate compiles
these archived sources directly, then runs the resulting client through the
current authenticated host, canonical behavior corpus, and two-process
reconnect/restart sequence.

Do not update these files when the live generator or binding changes. A future
wire change is compatible only if this archived client still passes unchanged.
`SHA256SUMS` makes accidental edits fail before compilation.
