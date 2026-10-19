"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { AppNav } from "@/components/AppNav";
import {
  ApiError,
  api,
  type LlmStatus,
  type PublicUser,
} from "@/lib/api";
import { getToken, getUser, isAdmin } from "@/lib/auth";

export default function AdminPage() {
  const router = useRouter();
  const [users, setUsers] = useState<PublicUser[]>([]);
  const [status, setStatus] = useState<LlmStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [role, setRole] = useState("user");
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    const token = getToken();
    const me = getUser();
    if (!token || !isAdmin(me)) {
      router.replace("/chat");
      return;
    }
    const [list, llm] = await Promise.all([
      api<PublicUser[]>("/admin/users", { token }),
      api<LlmStatus>("/admin/llm/status", { token }),
    ]);
    setUsers(list);
    setStatus(llm);
  }, [router]);

  useEffect(() => {
    load().catch((err) => {
      if (err instanceof ApiError && (err.status === 401 || err.status === 403)) {
        router.replace("/chat");
        return;
      }
      setError(err instanceof Error ? err.message : "Błąd admina.");
    });
  }, [load, router]);

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    const token = getToken();
    if (!token) return;
    setSaving(true);
    setError(null);
    setOk(null);
    try {
      await api<PublicUser>("/admin/users", {
        method: "POST",
        token,
        body: JSON.stringify({
          email,
          password,
          display_name: displayName,
          role,
        }),
      });
      setEmail("");
      setPassword("");
      setDisplayName("");
      setRole("user");
      setOk("Użytkownik dodany.");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się dodać.");
    } finally {
      setSaving(false);
    }
  }

  async function deactivate(id: string) {
    const token = getToken();
    if (!token) return;
    setError(null);
    try {
      await api(`/admin/users/${id}`, {
        method: "PATCH",
        token,
        body: JSON.stringify({ is_active: false }),
      });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się zaktualizować.");
    }
  }

  async function remove(id: string) {
    const token = getToken();
    if (!token) return;
    if (!confirm("Usunąć użytkownika na stałe?")) return;
    setError(null);
    try {
      await api(`/admin/users/${id}`, { method: "DELETE", token });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się usunąć.");
    }
  }

  return (
    <main className="shell">
      <AppNav />
      <section className="panel">
        <h2 className="brand" style={{ fontSize: "1.4rem" }}>
          Panel admina
        </h2>

        {status ? (
          <dl className="status-grid">
            <div>
              <dt>Provider</dt>
              <dd>{status.provider}</dd>
            </div>
            <div>
              <dt>Model</dt>
              <dd>{status.model}</dd>
            </div>
            <div>
              <dt>Źródło</dt>
              <dd>{status.source}</dd>
            </div>
            <div>
              <dt>Global RPM</dt>
              <dd>
                {status.usage.global_rpm_used}/{status.limits.global_rpm}
              </dd>
            </div>
            <div>
              <dt>Global dziennie</dt>
              <dd>
                {status.usage.global_daily_used}/{status.limits.global_daily}
              </dd>
            </div>
          </dl>
        ) : null}

        <h3 style={{ margin: "0 0 0.75rem", fontFamily: "var(--font-display)" }}>
          Dodaj użytkownika
        </h3>
        <form onSubmit={onCreate} className="form-row" style={{ marginBottom: "1.5rem" }}>
          <div className="field">
            <label htmlFor="dn">Nazwa</label>
            <input
              id="dn"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              required
            />
          </div>
          <div className="field">
            <label htmlFor="em">Email</label>
            <input
              id="em"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </div>
          <div className="field">
            <label htmlFor="pw">Hasło</label>
            <input
              id="pw"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              minLength={8}
              required
            />
          </div>
          <div className="field">
            <label htmlFor="role">Rola</label>
            <select
              id="role"
              value={role}
              onChange={(e) => setRole(e.target.value)}
            >
              <option value="user">user</option>
              <option value="admin">admin</option>
            </select>
          </div>
          <button className="btn" type="submit" disabled={saving}>
            {saving ? "Dodaję…" : "Dodaj dostęp"}
          </button>
        </form>

        {ok ? <p className="hint">{ok}</p> : null}
        {error ? <p className="error">{error}</p> : null}

        <table className="table">
          <thead>
            <tr>
              <th>Nazwa</th>
              <th>Email</th>
              <th>Rola</th>
              <th>Status</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id}>
                <td>{u.display_name}</td>
                <td>{u.email}</td>
                <td>{u.role}</td>
                <td>{u.is_active ? "aktywny" : "wyłączony"}</td>
                <td style={{ display: "flex", gap: "0.5rem" }}>
                  {u.is_active ? (
                    <button
                      type="button"
                      className="btn btn-ghost"
                      onClick={() => deactivate(u.id)}
                    >
                      Wyłącz
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => remove(u.id)}
                  >
                    Usuń
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </main>
  );
}
