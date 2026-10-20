"use client";

import { FormEvent, useEffect, useRef, useState } from "react";
import { AppShell } from "@/components/future/AppShell";
import { JarvisMark } from "@/components/future/JarvisMark";
import { ApiError, api, type Message, type UsageSnapshot } from "@/lib/api";
import { getToken } from "@/lib/auth";

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [usage, setUsage] = useState<UsageSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let stop = false;
    (async () => {
      try {
        const token = getToken();
        if (!token) return;
        const [msgs, snap] = await Promise.all([
          api<Message[]>("/chat/messages", { token }),
          api<UsageSnapshot>("/me/usage", { token }),
        ]);
        if (stop) return;
        setMessages(msgs);
        setUsage(snap);
      } catch (err) {
        if (!stop) {
          setError(err instanceof Error ? err.message : "Nie udało się wczytać czatu.");
        }
      }
    })();
    return () => {
      stop = true;
    };
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [messages]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const token = getToken();
    if (!token || !content.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await api<{
        user_message: Message;
        assistant_message: Message;
      }>("/chat/messages", {
        method: "POST",
        token,
        body: JSON.stringify({ content }),
      });
      setMessages((prev) => [...prev, res.user_message, res.assistant_message]);
      setContent("");
      const snap = await api<UsageSnapshot>("/me/usage", { token });
      setUsage(snap);
    } catch (err) {
      if (err instanceof ApiError && err.isRateLimited) {
        const scope = String(err.body.scope ?? "limit");
        setError(`Limit AI (${scope}). Spróbuj ponownie za ${err.retryAfterSec || 60}s.`);
      } else if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("Wysyłanie nieudane.");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <AppShell>
      <section className="fx-card">
        <div className="fx-page-head">
          <div>
            <p className="fx-kicker">Rozmowa</p>
            <h2>Czat</h2>
          </div>
          {usage ? (
            <p className="fx-note">
              {usage.user_rpm_used}/{usage.user_rpm_limit} RPM · {usage.user_daily_used}/
              {usage.user_daily_limit} dziennie
            </p>
          ) : null}
        </div>
        <div className="fx-messages" aria-live="polite">
          {messages.length === 0 ? (
            <p className="fx-note">Brak wiadomości — napisz do Jarvisa.</p>
          ) : (
            messages.map((message) => (
              <article
                key={message.id}
                className={`fx-bubble ${message.role === "assistant" ? "assistant" : ""}`}
              >
                <div className="who">
                  {message.role === "assistant" ? <JarvisMark size={16} glow={false} /> : null}
                  {message.role === "assistant" ? "Jarvis" : "Ty"}
                </div>
                <div className="text">{message.content}</div>
              </article>
            ))
          )}
          <div ref={endRef} />
        </div>
        <form className="fx-stack" onSubmit={onSubmit}>
          <label className="fx-field">
            <span>Wiadomość</span>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Wiadomość do Jarvisa…"
              required
            />
          </label>
          {error ? <p className="fx-alert">{error}</p> : null}
          <button className="fx-btn" type="submit" disabled={loading}>
            {loading ? "Myślę…" : "Wyślij"}
          </button>
        </form>
      </section>
    </AppShell>
  );
}
