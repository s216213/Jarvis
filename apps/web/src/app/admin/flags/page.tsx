"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { api, type FeatureFlag } from "@/lib/api";
import { getToken } from "@/lib/auth";
import { formatWhen } from "@/lib/format";

export default function FlagsPage() {
  const [flags, setFlags] = useState<FeatureFlag[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [keyName, setKeyName] = useState("");
  const [description, setDescription] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    const token = getToken();
    if (!token) return;
    const rows = await api<FeatureFlag[]>("/admin/flags", { token });
    setFlags(rows);
    setDrafts(Object.fromEntries(rows.map((flag) => [flag.key, flag.description])));
  }, []);

  useEffect(() => {
    let stop = false;
    (async () => {
      try {
        const token = getToken();
        if (!token) return;
        const rows = await api<FeatureFlag[]>("/admin/flags", { token });
        if (stop) return;
        setFlags(rows);
        setDrafts(Object.fromEntries(rows.map((flag) => [flag.key, flag.description])));
      } catch (err) {
        if (!stop) {
          setError(err instanceof Error ? err.message : "Nie udało się wczytać flag.");
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
    setError(null);
    try {
      await api<FeatureFlag>("/admin/flags", {
        method: "POST",
        token,
        body: JSON.stringify({
          key: keyName.trim(),
          description,
          enabled,
        }),
      });
      setKeyName("");
      setDescription("");
      setEnabled(true);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się dodać flagi.");
    }
  }

  async function toggle(flag: FeatureFlag) {
    const token = getToken();
    if (!token) return;
    setFlags((prev) =>
      prev.map((item) => (item.key === flag.key ? { ...item, enabled: !item.enabled } : item)),
    );
    try {
      const updated = await api<FeatureFlag>(`/admin/flags/${flag.key}`, {
        method: "PATCH",
        token,
        body: JSON.stringify({ enabled: !flag.enabled }),
      });
      setFlags((prev) => prev.map((item) => (item.key === updated.key ? updated : item)));
    } catch (err) {
      setFlags((prev) => prev.map((item) => (item.key === flag.key ? flag : item)));
      setError(err instanceof Error ? err.message : "Nie udało się przełączyć flagi.");
    }
  }

  async function saveDescription(flag: FeatureFlag) {
    const token = getToken();
    if (!token) return;
    setError(null);
    try {
      const updated = await api<FeatureFlag>(`/admin/flags/${flag.key}`, {
        method: "PATCH",
        token,
        body: JSON.stringify({ description: drafts[flag.key] ?? "" }),
      });
      setFlags((prev) => prev.map((item) => (item.key === updated.key ? updated : item)));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się zapisać opisu.");
    }
  }

  async function remove(flag: FeatureFlag) {
    const token = getToken();
    if (!token) return;
    if (!window.confirm(`Usunąć flagę ${flag.key}?`)) return;
    setError(null);
    try {
      await api(`/admin/flags/${flag.key}`, { method: "DELETE", token });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się usunąć flagi.");
    }
  }

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">Produkt</p>
          <h2>Feature flags</h2>
          <p>
            <code>chat_enabled</code> blokuje nowe wiadomości. <code>device_registration</code>{" "}
            blokuje tylko nowe urządzenia. Klienci czytają stan z <code>GET /flags</code>.
          </p>
        </div>
      </header>
      {error ? <p className="fx-alert">{error}</p> : null}

      <section className="fx-card">
        <h3>Nowa flaga</h3>
        <form onSubmit={onCreate} className="fx-form" style={{ marginTop: "0.9rem" }}>
          <label className="fx-field">
            <span>Klucz</span>
            <input
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              pattern="[a-z0-9_]{1,64}"
              placeholder="np. voice_replies"
              required
            />
          </label>
          <label className="fx-field">
            <span>Opis</span>
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              maxLength={400}
            />
          </label>
          <label className="fx-field" style={{ gridTemplateColumns: "auto 1fr", alignItems: "center" }}>
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              style={{ width: "auto" }}
            />
            <span style={{ textTransform: "none", letterSpacing: 0 }}>Włączona od razu</span>
          </label>
          <button className="fx-btn" type="submit">
            Dodaj
          </button>
        </form>
      </section>

      <div className="fx-stack">
        {flags.map((flag) => {
          const dirty = (drafts[flag.key] ?? "") !== flag.description;
          return (
            <article key={flag.key} className="fx-card fx-flag">
              <button
                type="button"
                className="fx-switch"
                role="switch"
                aria-checked={flag.enabled}
                aria-label={flag.enabled ? `Wyłącz ${flag.key}` : `Włącz ${flag.key}`}
                onClick={() => toggle(flag)}
              >
                <i />
              </button>
              <div>
                <code>{flag.key}</code>
                <label className="fx-field" style={{ marginTop: "0.55rem" }}>
                  <span>Opis</span>
                  <input
                    value={drafts[flag.key] ?? ""}
                    onChange={(e) =>
                      setDrafts((prev) => ({ ...prev, [flag.key]: e.target.value }))
                    }
                    maxLength={400}
                  />
                </label>
                <p>Aktualizacja {formatWhen(flag.updated_at)}</p>
              </div>
              <div className="fx-actions">
                <button
                  type="button"
                  className="fx-btn fx-btn-ghost"
                  disabled={!dirty}
                  onClick={() => saveDescription(flag)}
                >
                  Zapisz opis
                </button>
                <button type="button" className="fx-btn fx-btn-danger" onClick={() => remove(flag)}>
                  Usuń
                </button>
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
