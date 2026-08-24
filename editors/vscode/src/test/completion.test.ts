import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  cleanCompletion,
  contextBeforeCursor,
  healthUrl,
  normalizeApiBase,
  requestCompletion,
} from "../completion";

describe("endpoint normalization", () => {
  it("accepts server roots and OpenAI API bases", () => {
    assert.equal(normalizeApiBase("http://127.0.0.1:8100"), "http://127.0.0.1:8100/v1");
    assert.equal(normalizeApiBase("http://127.0.0.1:8100/v1/"), "http://127.0.0.1:8100/v1");
    assert.equal(healthUrl("http://127.0.0.1:8100/v1"), "http://127.0.0.1:8100/health");
  });
});

describe("completion context", () => {
  it("sends only the bounded text before the cursor", () => {
    assert.equal(contextBeforeCursor("0123456789", 8, 4), "4567");
    assert.equal(contextBeforeCursor("hello", 99, 100), "hello");
  });
});

describe("completion cleanup", () => {
  it("unwraps an accidental markdown fence and strips nulls", () => {
    assert.equal(cleanCompletion("```rust\nlet answer = 42;\n```"), "let answer = 42;");
    assert.equal(cleanCompletion("ok\0  "), "ok  ");
  });
});

describe("completion request", () => {
  it("uses the OpenAI endpoint, model, and bearer secret", async () => {
    let input: string | URL | Request | undefined;
    let init: RequestInit | undefined;
    const fetcher = async (nextInput: string | URL | Request, nextInit?: RequestInit) => {
      input = nextInput;
      init = nextInit;
      return new Response(JSON.stringify({ choices: [{ text: "world" }] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    const result = await requestCompletion(
      "hello ",
      {
        serverUrl: "http://localhost:8100",
        model: "code-model",
        maxTokens: 32,
        temperature: 0,
        contextCharacters: 1000,
      },
      "secret",
      new AbortController().signal,
      fetcher,
    );

    assert.equal(result, "world");
    assert.equal(input, "http://localhost:8100/v1/completions");
    assert.equal((init?.headers as Record<string, string>).Authorization, "Bearer secret");
    assert.deepEqual(JSON.parse(String(init?.body)), {
      prompt: "hello ",
      max_tokens: 32,
      temperature: 0,
      stream: false,
      cache_prompt: true,
      model: "code-model",
    });
  });

  it("surfaces a structured server error", async () => {
    const fetcher = async () =>
      new Response(JSON.stringify({ error: { message: "model is busy" } }), {
        status: 503,
        headers: { "Content-Type": "application/json" },
      });
    await assert.rejects(
      requestCompletion(
        "hello",
        {
          serverUrl: "http://localhost:8100/v1",
          maxTokens: 16,
          temperature: 0,
          contextCharacters: 100,
        },
        undefined,
        new AbortController().signal,
        fetcher,
      ),
      /model is busy/,
    );
  });
});
