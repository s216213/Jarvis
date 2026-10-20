"use client";

import { useEffect, useState } from "react";
import { JarvisVersionLines } from "@/components/future/JarvisVersion";
import { Meter } from "@/components/future/Meter";
import {
  api,
  type FeatureFlag,
  type HostSnapshot,
  type JarvisVersion,
  type LlmStatus,
  type PublicUser,
  type AdminDevice,
} from "@/lib/api";
import { getToken } from "@/lib/auth";
import { formatBytes, usagePercent } from "@/lib/format";

export default function AdminOverviewPage() {
  const [users, setUsers] = useState<PublicUser[]>([]);
  const [flags, setFlags] = useState<FeatureFlag[]>([]);
  const [devices, setDevices] = useState<AdminDevice[]>([]);
  const [status, setStatus] = useState<LlmStatus | null>(null);
  const [host, setHost] = useState<HostSnapshot | null>(null);
  const [jarvis, setJarvis] = useState<JarvisVersion | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [routeProvider, setRouteProvider] = useState("auto");
  const [routeModel, setRouteModel] = useState("auto");
  const [routeBusy, setRouteBusy] = useState(false);
  const [routeError, setRouteError] = useState<string | null>(null);

  useEffect(() => {
    let stop = false;
    const token = getToken();
    if (!token) return;

    async function loadLists() {
      const [userRows, flagRows, deviceRows, llm] = await Promise.all([
        api<PublicUser[]>("/admin/users", { token }),
        api<FeatureFlag[]>("/admin/flags", { token }),
        api<AdminDevice[]>("/admin/devices", { token }),
        api<LlmStatus>("/admin/llm/status", { token }),
      ]);
      if (stop) return;
      setUsers(userRows);
      setFlags(flagRows);
      setDevices(deviceRows);
      setStatus(llm);
      setRouteProvider(llm.route_provider || "auto");
      setRouteModel(llm.route_model || "auto");
    }

    async function loadHost() {
      try {
        const snap = await api<HostSnapshot>("/admin/host/metrics", { token });
        if (!stop) setHost(snap);
      } catch (err) {
        if (!stop) setError(err instanceof Error ? err.message : "Brak metryk hosta.");
      }
    }

    loadLists().catch((err) => {
      if (!stop) setError(err instanceof Error ? err.message : "Błąd panelu.");
    });
    api<JarvisVersion>("/version", { token })
      .then((row) => {
        if (!stop) setJarvis(row);
      })
      .catch(() => {});
    loadHost();
    const timer = window.setInterval(loadHost, 5000);
    return () => {
      stop = true;
      window.clearInterval(timer);
    };
  }, []);

  async function saveRoute(provider: string, model: string) {
    const token = getToken();
    if (!token) return;
    setRouteBusy(true);
    setRouteError(null);
    try {
      const llm = await api<LlmStatus>("/admin/llm/route", {
        method: "PATCH",
        token,
        body: JSON.stringify({ provider, model }),
      });
      setStatus(llm);
      setRouteProvider(llm.route_provider || "auto");
      setRouteModel(llm.route_model || "auto");
    } catch (err) {
      setRouteError(err instanceof Error ? err.message : "Nie udało się zapisać trasy.");
    } finally {
      setRouteBusy(false);
    }
  }

  function onProvider(next: string) {
    setRouteProvider(next);
    const allowed = (status?.models ?? []).some(
      (item) => item.provider === next && item.id === routeModel,
    );
    if (next !== "auto" && routeModel !== "auto" && !allowed) setRouteModel("auto");
  }

  const modelOptions = (status?.models ?? []).filter(
    (item) => routeProvider === "auto" || item.provider === routeProvider,
  );
  const disk = host?.disks[0];
  const diskPercent = disk ? usagePercent(disk.used_bytes, disk.total_bytes) : 0;
  const ramPercent = host
    ? usagePercent(host.memory_used_bytes, host.memory_total_bytes)
    : 0;
  const activeDevices = devices.filter((device) => !device.is_revoked).length;
  const enabledFlags = flags.filter((flag) => flag.enabled).length;

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">Sterowanie</p>
          <h2>Centrum sterowania</h2>
          <p>
            Migawka zasobów OCI co 5 sekund. Strumień na żywo zostaje wyłączony — metryki VM
            zmieniają się wolniej niż interfejs, a polling nie trzyma stałego połączenia.
          </p>
        </div>
        {host ? <span className="fx-pill">{host.hostname}</span> : null}
      </header>

      {error ? <p className="fx-alert">{error}</p> : null}

      <article className="fx-card">
        <p className="fx-kicker">Wersja Jarvisa</p>
        <JarvisVersionLines info={jarvis} />
      </article>

      <section className="fx-grid fx-grid-4">
        <Meter
          label="Procesor"
          value={host?.cpu_usage_percent ?? 0}
          detail={host ? `${host.cpu_count} rdzeni` : "Odczyt…"}
        />
        <Meter
          label="Pamięć"
          value={ramPercent}
          detail={
            host
              ? `${formatBytes(host.memory_used_bytes)} / ${formatBytes(host.memory_total_bytes)}`
              : "Odczyt…"
          }
        />
        <Meter
          label="Dysk"
          value={diskPercent}
          detail={disk ? disk.mount : "Odczyt…"}
        />
        <article className="fx-card">
          <p className="fx-kicker">Trasa</p>
          <p className="fx-stat">
            {status ? `${status.route_provider} : ${status.route_model}` : "—"}
          </p>
          <p>
            {status
              ? `${status.provider} · ${status.model} · ${status.usage.global_daily_used}/${status.limits.global_daily} dziennie`
              : "Status LLM"}
          </p>
        </article>
      </section>

      <article className="fx-card">
        <p className="fx-kicker">Wymuszenie modelu</p>
        <h3>Provider i model</h3>
        <p>
          Zapisana trasa to{" "}
          <span className="fx-mono">
            {status ? `${status.route_provider} : ${status.route_model}` : "auto : auto"}
          </span>
          . auto : auto zostawia wybór routerowi na każdą wiadomość.
        </p>
        {status?.route_note ? <p className="fx-alert">{status.route_note}</p> : null}
        {routeError ? <p className="fx-alert">{routeError}</p> : null}
        <form
          className="fx-form"
          onSubmit={(event) => {
            event.preventDefault();
            void saveRoute(routeProvider, routeModel);
          }}
        >
          <label className="fx-field">
            <span>Provider</span>
            <select
              value={routeProvider}
              onChange={(event) => onProvider(event.target.value)}
              disabled={routeBusy || !status}
            >
              <option value="auto">auto</option>
              {(status?.providers ?? []).map((provider) => (
                <option key={provider} value={provider}>
                  {provider}
                </option>
              ))}
            </select>
          </label>
          <label className="fx-field">
            <span>Model</span>
            <select
              value={routeModel}
              onChange={(event) => setRouteModel(event.target.value)}
              disabled={routeBusy || !status}
            >
              <option value="auto">auto</option>
              {modelOptions.map((item) => (
                <option key={`${item.provider}:${item.id}`} value={item.id}>
                  {routeProvider === "auto" ? `${item.provider} · ${item.id}` : item.id}
                </option>
              ))}
            </select>
          </label>
          <div className="fx-actions">
            <button className="fx-btn" type="submit" disabled={routeBusy || !status}>
              {routeBusy ? "…" : "Zapisz"}
            </button>
            <button
              className="fx-btn fx-btn-ghost"
              type="button"
              disabled={routeBusy || !status}
              onClick={() => void saveRoute("auto", "auto")}
            >
              auto : auto
            </button>
          </div>
        </form>
      </article>

      <section className="fx-grid fx-grid-3">
        <article className="fx-card">
          <h3>Konta</h3>
          <p className="fx-stat">{users.length}</p>
          <p>{users.filter((user) => user.is_active).length} aktywnych</p>
        </article>
        <article className="fx-card">
          <h3>Urządzenia</h3>
          <p className="fx-stat">{activeDevices}</p>
          <p>{devices.length} zapisanych w ekosystemie</p>
        </article>
        <article className="fx-card">
          <h3>Flagi</h3>
          <p className="fx-stat">{enabledFlags}</p>
          <p>{flags.length} zdefiniowanych</p>
        </article>
      </section>
    </div>
  );
}
