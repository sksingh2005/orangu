import * as vscode from "vscode";
import {
  CompletionSettings,
  contextBeforeCursor,
  healthUrl,
  normalizeApiBase,
  requestCompletion,
} from "./completion";

const SECRET_KEY = "orangu.apiKey";
const output = vscode.window.createOutputChannel("Orangu", { log: true });

function configuration(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("orangu");
}

function settings(): CompletionSettings {
  const config = configuration();
  return {
    serverUrl: config.get("serverUrl", "http://127.0.0.1:8100/v1"),
    model: config.get("model", ""),
    maxTokens: config.get("completion.maxTokens", 64),
    temperature: config.get("completion.temperature", 0.1),
    contextCharacters: config.get("completion.contextCharacters", 12000),
  };
}

function isEnabledFor(document: vscode.TextDocument): boolean {
  const config = configuration();
  const disabled = config.get<string[]>("completion.disabledLanguages", []);
  return config.get("inlineCompletions.enabled", true) && !disabled.includes(document.languageId);
}

function delay(milliseconds: number, token: vscode.CancellationToken): Promise<boolean> {
  return new Promise((resolve) => {
    if (token.isCancellationRequested) {
      resolve(false);
      return;
    }
    let settled = false;
    const finish = (elapsed: boolean) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      cancellation.dispose();
      resolve(elapsed);
    };
    const timer = setTimeout(() => finish(true), milliseconds);
    const cancellation = token.onCancellationRequested(() => {
      finish(false);
    });
  });
}

class OranguInlineCompletionProvider implements vscode.InlineCompletionItemProvider {
  private requestGeneration = 0;

  public constructor(
    private readonly secrets: vscode.SecretStorage,
    private readonly setStatus: (state: "ready" | "working" | "error") => void,
  ) {}

  public async provideInlineCompletionItems(
    document: vscode.TextDocument,
    position: vscode.Position,
    context: vscode.InlineCompletionContext,
    token: vscode.CancellationToken,
  ): Promise<vscode.InlineCompletionList | undefined> {
    if (!isEnabledFor(document)) {
      return undefined;
    }

    const lineBeforeCursor = document.lineAt(position.line).text.slice(0, position.character);
    if (context.triggerKind === vscode.InlineCompletionTriggerKind.Automatic && !lineBeforeCursor.trim()) {
      return undefined;
    }

    const debounce = configuration().get("completion.debounceMilliseconds", 250);
    if (context.triggerKind === vscode.InlineCompletionTriggerKind.Automatic) {
      const elapsed = await delay(debounce, token);
      if (!elapsed) {
        return undefined;
      }
    }

    const abort = new AbortController();
    const cancellation = token.onCancellationRequested(() => abort.abort());
    const requestGeneration = ++this.requestGeneration;
    this.setStatus("working");
    try {
      const currentSettings = settings();
      const prompt = contextBeforeCursor(
        document.getText(),
        document.offsetAt(position),
        currentSettings.contextCharacters,
      );
      const apiKey = await this.secrets.get(SECRET_KEY);
      const text = await requestCompletion(prompt, currentSettings, apiKey, abort.signal);
      if (requestGeneration === this.requestGeneration) {
        this.setStatus("ready");
      }
      if (requestGeneration !== this.requestGeneration || !text || token.isCancellationRequested) {
        return undefined;
      }
      return new vscode.InlineCompletionList([
        new vscode.InlineCompletionItem(text, new vscode.Range(position, position)),
      ]);
    } catch (error) {
      if (abort.signal.aborted) {
        if (requestGeneration === this.requestGeneration) {
          this.setStatus("ready");
        }
        return undefined;
      }
      if (requestGeneration === this.requestGeneration) {
        this.setStatus("error");
        output.error(error instanceof Error ? error.message : String(error));
      }
      return undefined;
    } finally {
      cancellation.dispose();
    }
  }
}

export function activate(context: vscode.ExtensionContext): void {
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 50);
  status.command = "orangu.toggleInlineCompletions";
  const setStatus = (state: "ready" | "working" | "error") => {
    const enabled = configuration().get("inlineCompletions.enabled", true);
    if (!enabled) {
      status.text = "$(circle-slash) Orangu";
      status.tooltip = "Orangu inline completions are disabled";
    } else if (state === "working") {
      status.text = "$(loading~spin) Orangu";
      status.tooltip = "Orangu is generating a completion";
    } else if (state === "error") {
      status.text = "$(warning) Orangu";
      status.tooltip = "Orangu could not reach the completion server; open the Orangu output for details";
    } else {
      status.text = "$(sparkle) Orangu";
      status.tooltip = "Orangu inline completions are enabled";
    }
    status.show();
  };
  setStatus("ready");

  const provider = new OranguInlineCompletionProvider(context.secrets, setStatus);
  context.subscriptions.push(
    output,
    status,
    vscode.languages.registerInlineCompletionItemProvider({ pattern: "**" }, provider),
    vscode.commands.registerCommand("orangu.triggerInlineCompletion", () =>
      vscode.commands.executeCommand("editor.action.inlineSuggest.trigger"),
    ),
    vscode.commands.registerCommand("orangu.toggleInlineCompletions", async () => {
      const config = configuration();
      const enabled = !config.get("inlineCompletions.enabled", true);
      await config.update(
        "inlineCompletions.enabled",
        enabled,
        vscode.ConfigurationTarget.Workspace,
      );
      setStatus("ready");
      void vscode.window.showInformationMessage(
        `Orangu inline completions ${enabled ? "enabled" : "disabled"} for this workspace.`,
      );
    }),
    vscode.commands.registerCommand("orangu.setApiKey", async () => {
      const apiKey = await vscode.window.showInputBox({
        title: "Set Orangu API key",
        prompt: "Stored securely in VS Code secret storage",
        password: true,
        ignoreFocusOut: true,
      });
      if (apiKey !== undefined) {
        await context.secrets.store(SECRET_KEY, apiKey);
        void vscode.window.showInformationMessage("Orangu API key saved.");
      }
    }),
    vscode.commands.registerCommand("orangu.clearApiKey", async () => {
      await context.secrets.delete(SECRET_KEY);
      void vscode.window.showInformationMessage("Orangu API key cleared.");
    }),
    vscode.commands.registerCommand("orangu.checkConnection", async () => {
      try {
        const current = settings();
        const apiKey = await context.secrets.get(SECRET_KEY);
        const headers: Record<string, string> = {};
        if (apiKey) {
          headers.Authorization = `Bearer ${apiKey}`;
        }
        const response = await fetch(healthUrl(current.serverUrl), { headers });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        setStatus("ready");
        void vscode.window.showInformationMessage(
          `Orangu server is reachable at ${normalizeApiBase(current.serverUrl)}.`,
        );
      } catch (error) {
        setStatus("error");
        const message = error instanceof Error ? error.message : String(error);
        output.error(`Connection check failed: ${message}`);
        void vscode.window.showErrorMessage(`Orangu server is unavailable: ${message}`);
      }
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("orangu.inlineCompletions.enabled")) {
        setStatus("ready");
      }
    }),
  );
}

export function deactivate(): void {}
