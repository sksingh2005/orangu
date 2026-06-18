\newpage

# Workspaces

A *workspace* is the project directory orangu is open on. It is the directory the model's tools read from and write to, the directory whose Git branch drives the prompt and the Git and review commands, and the directory a session belongs to. By default the workspace is the directory you launched orangu in; pass `--workspace <path>` (or `-w <path>`) at startup to begin somewhere else.

The `/workspace` command reports the active workspace and switches between workspaces without leaving the client. Each workspace is its own session, so switching to a workspace resumes the session you last had open there, or starts a fresh one when there is none. See the Sessions section of the Terminal interface chapter for how sessions are matched to a workspace and branch.

Where the workspace tabs are drawn is set by the `workspaces` key in the `[orangu]` configuration section; see the Tab placement section below.

\newpage

## /workspace

With no argument, `/workspace` reports the active workspace directory.

A bare number selects a workspace by its tab number; anything else is treated as a directory path. Switching to a directory opens it as a workspace and resumes the matching session there, or starts a fresh one if there is none.

Tab completion offers the workspaces seen in earlier sessions first, then completes filesystem directories, so a directory that has never been opened can still be navigated to a segment at a time:

```text
/workspace ~/pro⇥/ora⇥
```

It is handled locally and works regardless of server or model state.

### Examples

```text
/workspace
/workspace 1
/workspace ~/projects/orangu
```

Natural-language forms:

```text
workspace
switch workspace
workspace 1
switch workspace ~/projects/orangu
```

\newpage

## Tab placement

Where the workspace tabs are shown is set by the `workspaces` key in the `[orangu]` section of the configuration file:

```ini
[orangu]
workspaces = top
```

The options are `top` (the default), `bottom`, `left`, and `right`, matched case-insensitively. The interactive `--init` wizard prompts for it with Tab completion. Leaving the key out uses `top`; an unrecognised value is rejected when the configuration is loaded. The full list of `[orangu]` keys is in the Configuration chapter.
