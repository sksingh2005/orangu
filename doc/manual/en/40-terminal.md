\newpage

# Terminal interface

`orangu` is an interactive terminal client with a persistent header and a prompt area anchored to the bottom of the terminal.

## Header

The top banner displays:

- current version
- workspace status
- server status
- model status
- `/help` reminder

## Prompt area

The prompt area stays at the bottom of the terminal window.

- Long input wraps upward
- The model name is right-aligned below the prompt frame
- Submitted input moves directly into the output area

## Waiting state

While the model is generating a response, the output area shows a blinking:

```text
Thinking
```

placeholder in the position where the reply will appear.

## History and navigation

Command history is stored in:

```text
~/.orangu/orangu.history
```

Use:

- `<ARROW_UP>` to move backward in history
- `<ARROW_DOWN>` to move forward in history

## Connection commands

`orangu` supports runtime server target control:

- `/connect` reconnects to the configured endpoint of the active model profile
- `/connect <url>` sets a specific current server target
- `/disconnect` disconnects from the current server target
- `/reload` restores the startup model and configured server target and clears the current conversation

## Comments and ignored input

- If the first non-whitespace character is `#`, the line is treated as a local comment, shown in the transcript, and not sent to the LLM
- If the first non-whitespace character is `\`, the line is ignored

## Editing keys

The prompt supports standard shell-style editing:

- `Ctrl+A`
- `Ctrl+E`
- `Ctrl+K`
- `Ctrl+U`
- `Ctrl+W`
- `Home`
- `End`
- `Left`
- `Right`
- `Tab` completion for slash commands and `/model`

Press `Ctrl+C` once to arm quit mode. Press it again within 2 seconds to exit.
