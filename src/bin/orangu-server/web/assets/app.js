(() => {
  "use strict";

  const transcript = document.getElementById("transcript");
  const input = document.getElementById("input");
  const composer = document.getElementById("composer");
  const sendBtn = document.getElementById("send-btn");
  const reloadBtn = document.getElementById("reload-btn");
  const newChatBtn = document.getElementById("new-chat-btn");
  const historyBtn = document.getElementById("history-btn");
  const historyPanel = document.getElementById("history-panel");
  const historyList = document.getElementById("history-list");
  const themeToggleBtn = document.getElementById("theme-toggle-btn");

  const state = { sessionId: null, busy: false };

  const THEME_KEY = "orangu-theme";

  function effectiveTheme() {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved === "light" || saved === "dark") return saved;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function renderThemeToggle() {
    const dark = effectiveTheme() === "dark";
    const label = dark ? "Switch to light mode" : "Switch to dark mode";
    themeToggleBtn.textContent = dark ? "☀️" : "🌙";
    themeToggleBtn.setAttribute("aria-label", label);
    themeToggleBtn.setAttribute("title", label);
  }

  themeToggleBtn.addEventListener("click", () => {
    localStorage.setItem(THEME_KEY, effectiveTheme() === "dark" ? "light" : "dark");
    document.documentElement.setAttribute("data-theme", effectiveTheme());
    renderThemeToggle();
  });

  renderThemeToggle();

  function escapeHtml(text) {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function addMessage(role, text) {
    const el = document.createElement("div");
    el.className = `message ${role}`;
    el.textContent = text;
    transcript.appendChild(el);
    transcript.scrollTop = transcript.scrollHeight;
    return el;
  }

  function addRenderedMessage(role, html) {
    const el = document.createElement("div");
    el.className = `message ${role}`;
    el.innerHTML = html;
    transcript.appendChild(el);
    transcript.scrollTop = transcript.scrollHeight;
    return el;
  }

  // While a code block is still filling up during streaming, keep it
  // scrolled to its latest line (like `tail -f`) instead of leaving it
  // pinned to the top — the horizontal/vertical scrollbars (`.message
  // pre` in app.css) stay available throughout for manual scrolling.
  function pinCodeBlocksToLatest(el) {
    for (const pre of el.querySelectorAll("pre")) {
      pre.scrollTop = pre.scrollHeight;
    }
  }

  function setBusy(busy) {
    state.busy = busy;
    input.disabled = busy;
    sendBtn.disabled = busy;
  }

  async function createSession() {
    const res = await fetch("/api/sessions", { method: "POST" });
    if (!res.ok) throw new Error(`failed to create session (${res.status})`);
    return res.json();
  }

  async function newChat() {
    const session = await createSession();
    state.sessionId = session.id;
    localStorage.setItem("orangu-session-id", session.id);
    transcript.innerHTML = "";
    hideHistory();
  }

  async function loadSession(id) {
    const res = await fetch(`/api/sessions/${encodeURIComponent(id)}`);
    if (!res.ok) throw new Error(`failed to load session (${res.status})`);
    const session = await res.json();
    state.sessionId = session.id;
    localStorage.setItem("orangu-session-id", session.id);
    transcript.innerHTML = "";
    for (const message of session.messages) {
      if (message.role === "assistant") {
        addRenderedMessage("assistant", message.html || escapeHtml(message.content));
      } else {
        addMessage(message.role, message.content);
      }
    }
    hideHistory();
  }

  function formatDate(unixSeconds) {
    return new Date(unixSeconds * 1000).toLocaleString();
  }

  async function refreshHistory() {
    const res = await fetch("/api/sessions");
    if (!res.ok) return;
    const sessions = await res.json();
    historyList.innerHTML = "";
    if (sessions.length === 0) {
      const empty = document.createElement("div");
      empty.className = "history-empty";
      empty.textContent = "No previous chats yet.";
      historyList.appendChild(empty);
      return;
    }
    for (const session of sessions) {
      const item = document.createElement("div");
      item.className = "history-item";
      const title = document.createElement("div");
      title.className = "history-title";
      title.textContent = session.title || "New chat";
      const date = document.createElement("div");
      date.className = "history-date";
      date.textContent = formatDate(session.updated_at);
      item.appendChild(title);
      item.appendChild(date);
      item.addEventListener("click", () => {
        loadSession(session.id).catch((err) => console.error(err));
      });
      historyList.appendChild(item);
    }
  }

  function showHistory() {
    refreshHistory().catch((err) => console.error(err));
    historyPanel.hidden = false;
    historyBtn.setAttribute("aria-expanded", "true");
  }

  function hideHistory() {
    historyPanel.hidden = true;
    historyBtn.setAttribute("aria-expanded", "false");
  }

  // Shown in the chat on any failure — the real detail always goes to the
  // browser console (console.error) instead, for whoever's actually
  // debugging it; a chat bubble full of a stack trace or a template-
  // rendering error isn't useful to someone just trying to send a message.
  const FAILURE_MESSAGE = "🦧⚙️";

  function showFailure(assistantEl, consoleLabel, detail) {
    console.error(consoleLabel, detail);
    assistantEl.className = "message error";
    assistantEl.textContent = FAILURE_MESSAGE;
  }

  async function sendMessage(text) {
    if (!state.sessionId) {
      await newChat();
    }
    addMessage("user", text);
    const assistantEl = addMessage("assistant", "🤖");
    assistantEl.classList.add("pending");
    setBusy(true);

    try {
      const res = await fetch(`/api/sessions/${encodeURIComponent(state.sessionId)}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ content: text }),
      });
      if (!res.ok || !res.body) {
        const detail = await res.text().catch(() => "");
        throw new Error(`request failed (${res.status})${detail ? `: ${detail}` : ""}`);
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let sseBuffer = "";
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        sseBuffer += decoder.decode(value, { stream: true });
        const events = sseBuffer.split("\n\n");
        sseBuffer = events.pop() ?? "";
        for (const raw of events) {
          const line = raw.split("\n").find((l) => l.startsWith("data: "));
          if (!line) continue;
          const payload = JSON.parse(line.slice("data: ".length));
          assistantEl.classList.remove("pending");
          if (payload.type === "token" || payload.type === "done") {
            assistantEl.innerHTML = payload.html;
            pinCodeBlocksToLatest(assistantEl);
            if (payload.type === "done" && payload.truncated) {
              const notice = document.createElement("p");
              notice.className = "truncated-notice";
              notice.textContent = "⚠️ Response was cut off at the token limit.";
              assistantEl.appendChild(notice);
            }
            transcript.scrollTop = transcript.scrollHeight;
          } else if (payload.type === "error") {
            showFailure(assistantEl, "orangu-server generation error:", payload.message);
          }
        }
      }
    } catch (err) {
      showFailure(assistantEl, "orangu-server request failed:", err);
    } finally {
      setBusy(false);
    }
  }

  composer.addEventListener("submit", (event) => {
    event.preventDefault();
    if (state.busy) return;
    const text = input.value.trim();
    if (!text) return;
    input.value = "";
    sendMessage(text).catch((err) => console.error(err));
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      composer.requestSubmit();
    }
  });

  newChatBtn.addEventListener("click", () => {
    newChat().catch((err) => console.error(err));
  });

  reloadBtn.addEventListener("click", () => {
    window.location.reload();
  });

  // The Reload button stays hidden (see index.html) until the running
  // server's assets no longer match what this page was loaded with —
  // otherwise there's nothing for it to fix.
  const ASSET_VERSION = window.__ORANGU_ASSET_VERSION__;
  const UPDATE_CHECK_INTERVAL_MS = 60000;

  async function checkForUpdate() {
    if (!reloadBtn.hidden) return;
    try {
      const res = await fetch("/api/asset-version", { cache: "no-store" });
      if (!res.ok) return;
      const { version } = await res.json();
      if (version && version !== ASSET_VERSION) {
        reloadBtn.hidden = false;
      }
    } catch {
      // Server unreachable right now — nothing to report.
    }
  }

  setInterval(() => checkForUpdate().catch((err) => console.error(err)), UPDATE_CHECK_INTERVAL_MS);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") checkForUpdate().catch((err) => console.error(err));
  });
  checkForUpdate().catch((err) => console.error(err));

  historyBtn.addEventListener("click", () => {
    if (historyPanel.hidden) {
      showHistory();
    } else {
      hideHistory();
    }
  });

  document.addEventListener("click", (event) => {
    if (
      !historyPanel.hidden &&
      !historyPanel.contains(event.target) &&
      event.target !== historyBtn
    ) {
      hideHistory();
    }
  });

  (async function init() {
    const savedId = localStorage.getItem("orangu-session-id");
    if (savedId) {
      try {
        await loadSession(savedId);
        return;
      } catch {
        // Stale/missing session — fall through to creating a new one.
      }
    }
    await newChat();
  })().catch((err) => console.error(err));
})();
