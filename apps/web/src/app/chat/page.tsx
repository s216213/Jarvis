"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { AppNav } from "@/components/AppNav";
import {
  ApiError,
  api,
  type Message,
  type UsageSnapshot,
} from "@/lib/api";
import { getToken } from "@/lib/auth";

export default function ChatPage() {
  const router = useRouter();
  const [messages, setMessages] = useState<Message[]>([]);
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [usage, setUsage] = useState<UsageSnapshot | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    const token = getToken();
    if (!token) {
      router.replace("/login");
      return;
    }
    const [msgs, snap] = await Promise.all([
      api<Message[]>("/chat/messages", { token }),
      api<UsageSnapshot>("/me/usage", { token }),
    ]);
    setMessages(msgs);
    setUsage(snap);
  }, [router]);

  useEffect(() => {
    load().catch((err) => {
      if (err instanceof ApiError && err.status === 401) {
        router.replace("/login");
        return;
      }
      setError(err instanceof Error ? err.message : "Nie udało się wczytać czatu.");
    });
  }, [load, router]);

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
        model: string;
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
        setError(
          `Limit AI (${scope}). Spróbuj ponownie za ${err.retryAfterSec || 60}s.`,
        );
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
    <main className="shell">
      <AppNav />
      <section className="panel">
        {usage ? (
          <p className="hint">
            Zużycie: {usage.user_rpm_used}/{usage.user_rpm_limit} RPM ·{" "}
            {usage.user_daily_used}/{usage.user_daily_limit} dziennie
          </p>
        ) : null}
        <div className="messages">
          {messages.length === 0 ? (
            <p className="hint">Brak wiadomości — napisz do Jarvisa.</p>
          ) : (
            messages.map((m) => (
              <article
                key={m.id}
                className={`bubble ${m.role === "assistant" ? "assistant" : ""}`}
              >
                <div className="who">
                  {m.role === "assistant" ? "Jarvis" : "Ty"}
                </div>
                <div className="text">{m.content}</div>
              </article>
            ))
          )}
        </div>
        <form className="composer" onSubmit={onSubmit}>
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Wiadomość do Jarvisa…"
            required
          />
          {error ? <p className="error">{error}</p> : null}
          <button className="btn" type="submit" disabled={loading}>
            {loading ? "Myślę…" : "Wyślij"}
          </button>
        </form>
      </section>
    </main>
  );
}
