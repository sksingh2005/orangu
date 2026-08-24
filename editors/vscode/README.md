# Orangu for VS Code

Orangu for VS Code provides private inline code completions from an
`orangu-server` (or another OpenAI-compatible completion server). Source context
goes only to the endpoint you configure; the default is local loopback.

## Development install

1. Start a code-capable server, for example:

   ```sh
   orangu-server --code <model>
   ```

2. Build and package the extension:

   ```sh
   cd editors/vscode
   npm install
   npm test
   npm run package
   ```

3. In VS Code, run **Extensions: Install from VSIX...** and select the generated
   `.vsix` file.

The default endpoint is `http://127.0.0.1:8100/v1`. Change **Orangu: Server
Url** when the server or coordinator listens elsewhere. If it requires a key,
run **Orangu: Set API Key**; the key is stored in VS Code secret storage, not in
`settings.json`.

Inline suggestions appear after a short typing pause and are accepted with the
normal VS Code inline-suggestion key (`Tab` by default). Use `Ctrl+Alt+O`
(`Cmd+Alt+O` on macOS) to request one explicitly. The Orangu status-bar item
toggles automatic suggestions for the current workspace; **Orangu: Check Server
Connection** diagnoses endpoint problems.

## Settings

- `orangu.inlineCompletions.enabled` — workspace-level completion toggle.
- `orangu.serverUrl` — OpenAI API base or server root.
- `orangu.model` — optional model selector, useful with `orangu-coordinator`.
- `orangu.completion.maxTokens` — generation cap (default 64).
- `orangu.completion.temperature` — sampling temperature (default 0.1).
- `orangu.completion.contextCharacters` — bounded context before the cursor.
- `orangu.completion.debounceMilliseconds` — automatic request delay.
- `orangu.completion.disabledLanguages` — language IDs excluded from automatic completion.

The extension intentionally starts with one narrow feature. Orangu's terminal
client remains the place for agentic edits, Git operations, and `/review` or
`/auto_review`; the extension reuses the server boundary without duplicating
those safety-sensitive workflows inside the editor.
