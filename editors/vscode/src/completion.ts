export interface CompletionSettings {
  serverUrl: string;
  model?: string;
  maxTokens: number;
  temperature: number;
  contextCharacters: number;
}

export interface CompletionResponse {
  choices?: Array<{ text?: string }>;
  error?: string | { message?: string };
}

export type FetchLike = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export function normalizeApiBase(serverUrl: string): string {
  const trimmed = serverUrl.trim().replace(/\/+$/, "");
  if (!trimmed) {
    throw new Error("Orangu server URL is empty");
  }
  return trimmed.endsWith("/v1") ? trimmed : `${trimmed}/v1`;
}

export function healthUrl(serverUrl: string): string {
  const base = normalizeApiBase(serverUrl);
  return `${base.slice(0, -3)}/health`;
}

export function contextBeforeCursor(
  documentText: string,
  cursorOffset: number,
  maxCharacters: number,
): string {
  const safeOffset = Math.max(0, Math.min(cursorOffset, documentText.length));
  return documentText.slice(Math.max(0, safeOffset - maxCharacters), safeOffset);
}

export function cleanCompletion(text: string): string {
  const withoutNulls = text.replaceAll("\0", "");
  const fenced = withoutNulls.match(/^\s*```[^\n]*\n([\s\S]*?)\n```\s*$/);
  return fenced?.[1] ?? withoutNulls;
}

function errorMessage(body: CompletionResponse | undefined, status: number): string {
  if (typeof body?.error === "string") {
    return body.error;
  }
  if (body?.error && typeof body.error.message === "string") {
    return body.error.message;
  }
  return `HTTP ${status}`;
}

export async function requestCompletion(
  prompt: string,
  settings: CompletionSettings,
  apiKey: string | undefined,
  signal: AbortSignal,
  fetcher: FetchLike = fetch,
): Promise<string> {
  const body: Record<string, unknown> = {
    prompt,
    max_tokens: settings.maxTokens,
    temperature: settings.temperature,
    stream: false,
    cache_prompt: true,
  };
  if (settings.model?.trim()) {
    body.model = settings.model.trim();
  }

  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (apiKey) {
    headers.Authorization = `Bearer ${apiKey}`;
  }

  const response = await fetcher(`${normalizeApiBase(settings.serverUrl)}/completions`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal,
  });

  let parsed: CompletionResponse | undefined;
  try {
    parsed = (await response.json()) as CompletionResponse;
  } catch {
    // Keep the status-based error below when a proxy returns HTML or plain text.
  }
  if (!response.ok) {
    throw new Error(`Orangu completion failed: ${errorMessage(parsed, response.status)}`);
  }
  return cleanCompletion(parsed?.choices?.[0]?.text ?? "");
}
