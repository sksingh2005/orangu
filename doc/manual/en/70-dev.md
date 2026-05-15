\newpage

# Developer information

## Main components

- `src/bin/orangu.rs` - terminal loop, command handling, history, connection state, and waiting state
- `src/config.rs` - INI parsing and normalization
- `src/llm/openai.rs` - OpenAI-compatible client for llama.cpp-style backends
- `src/session.rs` - tool-calling conversation flow
- `src/tools.rs` - workspace-scoped local tool execution
- `src/tui.rs` - header, prompt frame, and status rendering

## Development workflow

```sh
cargo fmt
cargo test
```

## Documentation workflow

```sh
./doc/build_manual.sh
```

## Notes

- The default config lookup path is `~/.orangu/orangu.conf`
- Command history is stored in `~/.orangu/orangu.history`
- Tool-calling prompts can be slow on local models, so the default timeout is 30 minutes
