"use client";

import { useEffect, useState } from "react";
import { Meter } from "@/components/future/Meter";
import { api, type HostSnapshot } from "@/lib/api";
import { getToken } from "@/lib/auth";
import { formatBytes, formatUptime, formatWhen, usagePercent } from "@/lib/format";

export default function ResourcesPage() {
  const [host, setHost] = useState<HostSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let stop = false;
    const token = getToken();
    if (!token) return;

    async function tick() {
      try {
        const snap = await api<HostSnapshot>("/admin/host/metrics", { token });
        if (!stop) {
          setHost(snap);
          setError(null);
        }
      } catch (err) {
        if (!stop) setError(err instanceof Error ? err.message : "Brak metryk.");
      }
    }

    tick();
    const timer = window.setInterval(tick, 5000);
    return () => {
      stop = true;
      window.clearInterval(timer);
    };
  }, []);

  const ram = host ? usagePercent(host.memory_used_bytes, host.memory_total_bytes) : 0;

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">OCI VM</p>
          <h2>Zasoby</h2>
          <p>
            Odczyt co 5 s jako migawka, nie jako strumień. Na maszynie OCI CPU i RAM pochodzą z
            procesu API (w Dockerze zwykle widok hosta przez /proc). Dysk oznaczony „OCI host” to
            katalog VM zamontowany w <span className="fx-mono">/host/root</span>.
          </p>
        </div>
        {host ? (
          <span className="fx-pill">
            <span className="fx-dot" aria-hidden />
            {formatWhen(host.collected_at)}
          </span>
        ) : null}
      </header>
      {error ? <p className="fx-alert">{error}</p> : null}
      {!host ? <p className="fx-note">Odczyt migawki…</p> : null}
      {host ? (
        <>
          <section className="fx-grid fx-grid-3">
            <Meter
              label="Procesor"
              value={host.cpu_usage_percent}
              detail={`${host.cpu_count} rdzeni · load ${host.load_1.toFixed(2)} / ${host.load_5.toFixed(2)} / ${host.load_15.toFixed(2)}`}
            />
            <Meter
              label="Pamięć RAM"
              value={ram}
              detail={`${formatBytes(host.memory_used_bytes)} z ${formatBytes(host.memory_total_bytes)} · wolne ${formatBytes(host.memory_available_bytes)}`}
            />
            <article className="fx-card">
              <h3>{host.hostname}</h3>
              <p>
                {host.os}
                <br />
                {host.kernel} · {host.arch}
                <br />
                Uptime {formatUptime(host.uptime_sec)} · procesy {host.process_count}
              </p>
              {host.swap_total_bytes > 0 ? (
                <p>
                  Swap {formatBytes(host.swap_used_bytes)} / {formatBytes(host.swap_total_bytes)}
                </p>
              ) : null}
            </article>
          </section>

          <section className="fx-card">
            <h3>Rdzenie</h3>
            <div className="fx-cores" style={{ marginTop: "0.8rem" }}>
              {host.cores.map((core) => (
                <div key={core.name} className="fx-core">
                  <span>
                    {core.name} · {Math.round(core.usage_percent)}%
                  </span>
                  <div className="fx-bar" aria-hidden>
                    <span style={{ width: `${Math.min(100, core.usage_percent)}%` }} />
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="fx-card">
            <h3>Dyski</h3>
            <div className="fx-table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Montowanie</th>
                    <th>Zakres</th>
                    <th>Użycie</th>
                    <th>Zajęte</th>
                  </tr>
                </thead>
                <tbody>
                  {host.disks.map((disk) => {
                    const percent = usagePercent(disk.used_bytes, disk.total_bytes);
                    return (
                      <tr key={`${disk.mount}-${disk.name}`}>
                        <td>
                          <div className="fx-mono">{disk.mount}</div>
                          <div className="fx-note">{disk.name || "wolumen"}</div>
                        </td>
                        <td>{disk.scope === "host" ? "OCI host" : "Proces API"}</td>
                        <td style={{ minWidth: 140 }}>
                          <div className="fx-bar" aria-hidden>
                            <span style={{ width: `${percent}%` }} />
                          </div>
                        </td>
                        <td>
                          {formatBytes(disk.used_bytes)} / {formatBytes(disk.total_bytes)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}
