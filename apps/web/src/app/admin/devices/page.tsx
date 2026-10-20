"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { api, type AdminDevice } from "@/lib/api";
import { getToken } from "@/lib/auth";
import { formatAgo, formatWhen } from "@/lib/format";

export default function DevicesPage() {
  const [devices, setDevices] = useState<AdminDevice[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [name, setName] = useState("");

  const load = useCallback(async () => {
    const token = getToken();
    if (!token) return;
    setDevices(await api<AdminDevice[]>("/admin/devices", { token }));
  }, []);

  useEffect(() => {
    let stop = false;
    (async () => {
      try {
        const token = getToken();
        if (!token) return;
        const rows = await api<AdminDevice[]>("/admin/devices", { token });
        if (!stop) setDevices(rows);
      } catch (err) {
        if (!stop) {
          setError(err instanceof Error ? err.message : "Nie udało się wczytać urządzeń.");
        }
      }
    })();
    const onDevice = () => {
      void load().catch((err: unknown) => {
        if (!stop) {
          setError(err instanceof Error ? err.message : "Nie udało się wczytać urządzeń.");
        }
      });
    };
    window.addEventListener("jarvis-device", onDevice);
    return () => {
      stop = true;
      window.removeEventListener("jarvis-device", onDevice);
    };
  }, [load]);

  async function patch(id: string, body: { name?: string; is_revoked?: boolean }) {
    const token = getToken();
    if (!token) return;
    setError(null);
    try {
      await api(`/admin/devices/${id}`, {
        method: "PATCH",
        token,
        body: JSON.stringify(body),
      });
      setRenameId(null);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się zapisać urządzenia.");
    }
  }

  async function remove(device: AdminDevice) {
    const token = getToken();
    if (!token) return;
    if (!window.confirm(`Odłączyć ${device.name} na stałe?`)) return;
    setError(null);
    try {
      await api(`/admin/devices/${device.id}`, { method: "DELETE", token });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nie udało się usunąć urządzenia.");
    }
  }

  function onRename(e: FormEvent) {
    e.preventDefault();
    if (!renameId) return;
    void patch(renameId, { name });
  }

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">Ekosystem</p>
          <h2>Urządzenia</h2>
          <p>
            Klienci web, Android i desktop zgłaszają się po zalogowaniu. Cofnięcie dostępu blokuje
            kolejne zgłoszenia tego odcisku.
          </p>
        </div>
      </header>
      {error ? <p className="fx-alert">{error}</p> : null}
      <section className="fx-card">
        {devices.length === 0 ? (
          <p className="fx-note">
            Żadne urządzenie jeszcze się nie zgłosiło. Ta przeglądarka rejestruje się po wejściu do
            panelu.
          </p>
        ) : (
          <div className="fx-table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Nazwa</th>
                  <th>Platforma</th>
                  <th>Właściciel</th>
                  <th>Wersje</th>
                  <th>Ostatnio</th>
                  <th>Status</th>
                  <th>Akcje</th>
                </tr>
              </thead>
              <tbody>
                {devices.map((device) => (
                  <tr key={device.id}>
                    <td>
                      {renameId === device.id ? (
                        <form onSubmit={onRename} className="fx-actions">
                          <input
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            aria-label="Nowa nazwa urządzenia"
                            required
                            maxLength={80}
                          />
                          <button className="fx-btn" type="submit">
                            Zapisz
                          </button>
                        </form>
                      ) : (
                        device.name
                      )}
                    </td>
                    <td>
                      <span className="fx-badge">{device.platform}</span>
                    </td>
                    <td>
                      {device.user_display_name}
                      <div className="fx-note">{device.user_email}</div>
                    </td>
                    <td className="fx-mono">
                      <div>UI {device.ui_version || "—"}</div>
                      <div>Klient {device.client_version || "—"}</div>
                      <div>
                        {device.system_name || "—"}
                        {device.system_version ? ` ${device.system_version}` : ""}
                      </div>
                    </td>
                    <td title={formatWhen(device.last_seen_at)}>{formatAgo(device.last_seen_at)}</td>
                    <td>
                      <span className={`fx-badge ${device.is_revoked ? "danger" : "ok"}`}>
                        {device.is_revoked ? "odcięte" : "podpięte"}
                      </span>
                    </td>
                    <td>
                      <div className="fx-actions">
                        <button
                          type="button"
                          className="fx-btn fx-btn-ghost"
                          onClick={() => {
                            setRenameId(device.id);
                            setName(device.name);
                          }}
                        >
                          Nazwa
                        </button>
                        <button
                          type="button"
                          className="fx-btn fx-btn-ghost"
                          onClick={() => patch(device.id, { is_revoked: !device.is_revoked })}
                        >
                          {device.is_revoked ? "Przywróć" : "Odetnij"}
                        </button>
                        <button
                          type="button"
                          className="fx-btn fx-btn-danger"
                          onClick={() => remove(device)}
                        >
                          Usuń
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
