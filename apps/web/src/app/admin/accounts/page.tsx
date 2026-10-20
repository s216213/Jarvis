"use client";

import { FormEvent, useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { api, type PublicUser } from "@/lib/api";
import { getToken, getUser, setSession, subscribeSession } from "@/lib/auth";
import { formatWhen } from "@/lib/format";

type Draft = {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_active: boolean;
  password: string;
};

export default function AccountsPage() {
  const [users, setUsers] = useState<PublicUser[]>([]);
  const meId = useSyncExternalStore(
    subscribeSession,
    () => getUser()?.id ?? null,
    () => null,
  );
  const [error, setError] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [role, setRole] = useState("user");
  const [saving, setSaving] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  const load = useCallback(async () => {
    const token = getToken();
    if (!token) return;
    const list = await api<PublicUser[]>("/admin/users", { token });
    setUsers(list);
  }, []);

  useEffect(() => {
    let stop = false;
    (async () => {
      try {
        const token = getToken();
        if (!token) return;
        const list = await api<PublicUser[]>("/admin/users", { token });
        if (!stop) setUsers(list);
      } catch (err) {
        if (!stop) {
          setError(err instanceof Error ? err.message : "Nie udało się wczytać kont.");
        }
      }
    })();
    return () => {
      stop = true;
    };
  }, []);

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
      setOk("Konto dodane.");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się dodać.");
    } finally {
      setSaving(false);
    }
  }

  function openEdit(user: PublicUser) {
    setDraft({
      id: user.id,
      email: user.email,
      display_name: user.display_name,
      role: user.role,
      is_active: user.is_active,
      password: "",
    });
    dialogRef.current?.showModal();
  }

  async function saveDraft(e: FormEvent) {
    e.preventDefault();
    const token = getToken();
    if (!token || !draft) return;
    setError(null);
    setOk(null);
    try {
      const updated = await api<PublicUser>(`/admin/users/${draft.id}`, {
        method: "PATCH",
        token,
        body: JSON.stringify({
          email: draft.email,
          display_name: draft.display_name,
          role: draft.role,
          is_active: draft.is_active,
          ...(draft.password ? { password: draft.password } : {}),
        }),
      });
      if (updated.id === meId) setSession(token, updated);
      setOk("Konto zapisane.");
      dialogRef.current?.close();
      setDraft(null);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się zapisać.");
    }
  }

  async function setActive(user: PublicUser, isActive: boolean) {
    const token = getToken();
    if (!token) return;
    setError(null);
    try {
      await api(`/admin/users/${user.id}`, {
        method: "PATCH",
        token,
        body: JSON.stringify({ is_active: isActive }),
      });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się zmienić statusu.");
    }
  }

  async function remove(user: PublicUser) {
    const token = getToken();
    if (!token) return;
    if (!window.confirm(`Usunąć konto ${user.email} na stałe?`)) return;
    setError(null);
    try {
      await api(`/admin/users/${user.id}`, { method: "DELETE", token });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się usunąć.");
    }
  }

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">Dostęp</p>
          <h2>Konta</h2>
          <p>Tworzenie, edycja, wyłączanie i usuwanie. Własnego konta nie da się wyłączyć ani usunąć.</p>
        </div>
      </header>

      <section className="fx-card">
        <h3>Nowe konto</h3>
        <form onSubmit={onCreate} className="fx-form" style={{ marginTop: "0.9rem" }}>
          <label className="fx-field">
            <span>Nazwa</span>
            <input value={displayName} onChange={(e) => setDisplayName(e.target.value)} required />
          </label>
          <label className="fx-field">
            <span>Email</span>
            <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
          </label>
          <label className="fx-field">
            <span>Hasło</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              minLength={8}
              required
            />
          </label>
          <label className="fx-field">
            <span>Rola</span>
            <select value={role} onChange={(e) => setRole(e.target.value)}>
              <option value="user">user</option>
              <option value="admin">admin</option>
            </select>
          </label>
          <button className="fx-btn" type="submit" disabled={saving}>
            {saving ? "Dodaję…" : "Dodaj dostęp"}
          </button>
        </form>
        {ok ? <p className="fx-alert ok">{ok}</p> : null}
        {error ? <p className="fx-alert">{error}</p> : null}
      </section>

      <section className="fx-card">
        <div className="fx-table-wrap">
          <table>
            <thead>
              <tr>
                <th>Nazwa</th>
                <th>Email</th>
                <th>Rola</th>
                <th>Status</th>
                <th>Utworzone</th>
                <th>Akcje</th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => {
                const self = user.id === meId;
                return (
                  <tr key={user.id}>
                    <td>{user.display_name}</td>
                    <td>{user.email}</td>
                    <td>{user.role}</td>
                    <td>
                      <span className={`fx-badge ${user.is_active ? "ok" : "danger"}`}>
                        {user.is_active ? "aktywne" : "wyłączone"}
                      </span>
                    </td>
                    <td>{formatWhen(user.created_at)}</td>
                    <td>
                      <div className="fx-actions">
                        <button type="button" className="fx-btn fx-btn-ghost" onClick={() => openEdit(user)}>
                          Edytuj
                        </button>
                        {self ? null : user.is_active ? (
                          <button
                            type="button"
                            className="fx-btn fx-btn-ghost"
                            onClick={() => setActive(user, false)}
                          >
                            Wyłącz
                          </button>
                        ) : (
                          <button
                            type="button"
                            className="fx-btn fx-btn-ghost"
                            onClick={() => setActive(user, true)}
                          >
                            Włącz
                          </button>
                        )}
                        {self ? null : (
                          <button type="button" className="fx-btn fx-btn-danger" onClick={() => remove(user)}>
                            Usuń
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>

      <dialog ref={dialogRef} className="fx-dialog" onClose={() => setDraft(null)}>
        {draft ? (
          <form onSubmit={saveDraft} className="fx-stack">
            <h3>Edycja konta</h3>
            <label className="fx-field">
              <span>Nazwa</span>
              <input
                value={draft.display_name}
                onChange={(e) => setDraft({ ...draft, display_name: e.target.value })}
                required
              />
            </label>
            <label className="fx-field">
              <span>Email</span>
              <input
                type="email"
                value={draft.email}
                onChange={(e) => setDraft({ ...draft, email: e.target.value })}
                required
              />
            </label>
            <label className="fx-field">
              <span>Rola</span>
              <select
                value={draft.role}
                disabled={draft.id === meId}
                onChange={(e) => setDraft({ ...draft, role: e.target.value })}
              >
                <option value="user">user</option>
                <option value="admin">admin</option>
              </select>
            </label>
            <label className="fx-field">
              <span>Nowe hasło</span>
              <input
                type="password"
                value={draft.password}
                minLength={8}
                placeholder="zostaw puste, aby nie zmieniać"
                onChange={(e) => setDraft({ ...draft, password: e.target.value })}
              />
            </label>
            <label className="fx-field" style={{ gridTemplateColumns: "auto 1fr", alignItems: "center" }}>
              <input
                type="checkbox"
                checked={draft.is_active}
                disabled={draft.id === meId}
                onChange={(e) => setDraft({ ...draft, is_active: e.target.checked })}
                style={{ width: "auto" }}
              />
              <span style={{ textTransform: "none", letterSpacing: 0 }}>Konto aktywne</span>
            </label>
            <div className="fx-actions">
              <button className="fx-btn" type="submit">
                Zapisz
              </button>
              <button
                className="fx-btn fx-btn-ghost"
                type="button"
                onClick={() => dialogRef.current?.close()}
              >
                Anuluj
              </button>
            </div>
          </form>
        ) : null}
      </dialog>
    </div>
  );
}
