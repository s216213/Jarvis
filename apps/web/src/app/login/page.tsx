"use client";

import { FormEvent, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { ApiError, api, type PublicUser } from "@/lib/api";
import { getToken, setSession } from "@/lib/auth";
import { FUTURE_UI } from "@/lib/future";

export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (getToken()) router.replace("/chat");
  }, [router]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const res = await api<{ access_token: string; user: PublicUser }>(
        "/auth/login",
        {
          method: "POST",
          body: JSON.stringify({ email, password }),
        },
      );
      setSession(res.access_token, res.user);
      router.replace("/chat");
    } catch (err) {
      if (err instanceof ApiError && err.isRateLimited) {
        setError(
          `Za dużo prób logowania. Spróbuj za ${err.retryAfterSec || 60}s.`,
        );
      } else if (err instanceof Error) {
        setError(err.message === "unauthorized" ? "Błędny email lub hasło." : err.message);
      } else {
        setError("Logowanie nieudane.");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="fx-login">
      <div className="fx-orb" aria-hidden />
      <section className="fx-card fx-login-card">
        <p className="fx-kicker">{FUTURE_UI}</p>
        <h1>
          Jar<span>vis</span>
        </h1>
        <p className="fx-note">Dostęp tylko na zaproszenie administratora.</p>
        <form onSubmit={onSubmit} className="fx-stack" style={{ marginTop: "1.2rem" }}>
          <label className="fx-field">
            <span>Email</span>
            <input
              type="email"
              autoComplete="username"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </label>
          <label className="fx-field">
            <span>Hasło</span>
            <input
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </label>
          {error ? <p className="fx-alert">{error}</p> : null}
          <button className="fx-btn" type="submit" disabled={loading}>
            {loading ? "Logowanie…" : "Zaloguj"}
          </button>
        </form>
      </section>
    </main>
  );
}
