"use client";

import { FormEvent, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { ApiError, api, type PublicUser } from "@/lib/api";
import { getToken, setSession } from "@/lib/auth";

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
    <main className="login-stage">
      <div className="inner">
        <h1 className="brand">
          Jar<span>vis</span>
        </h1>
        <p className="lede">Dostęp tylko na zaproszenie administratora.</p>
        <form className="panel" onSubmit={onSubmit}>
          <div className="field">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              type="email"
              autoComplete="username"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </div>
          <div className="field">
            <label htmlFor="password">Hasło</label>
            <input
              id="password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>
          {error ? <p className="error">{error}</p> : null}
          <button className="btn" type="submit" disabled={loading}>
            {loading ? "Logowanie…" : "Zaloguj"}
          </button>
        </form>
      </div>
    </main>
  );
}
