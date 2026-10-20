"use client";

import { FormEvent, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api, type ExecResult, type HostSnapshot } from "@/lib/api";
import { getToken } from "@/lib/auth";

type Row = { id: number; kind: "cmd" | "out" | "err" | "meta"; text: string };

export default function TerminalPage() {
  const [rows, setRows] = useState<Row[]>([
    {
      id: 0,
      kind: "meta",
      text: "Terminal administratora. Jedna linia to jedno polecenie, katalog roboczy zostaje między poleceniami.",
    },
  ]);
  const [cwd, setCwd] = useState("~");
  const [line, setLine] = useState("");
  const [busy, setBusy] = useState(false);
  const [execMode, setExecMode] = useState<string>("");
  const [modeNote, setModeNote] = useState("Sprawdzam, gdzie polecenia będą wykonywane…");
  const history = useRef<string[]>([]);
  const histIndex = useRef(-1);
  const nextId = useRef(1);
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const token = getToken();
    if (!token) return;
    let stop = false;
    api<HostSnapshot>("/admin/host/metrics", { token })
      .then((snap) => {
        if (stop) return;
        setExecMode(snap.exec_mode);
        setModeNote(
          snap.exec_mode === "host"
            ? `Host OCI · ${snap.hostname}. Polecenia wchodzą w przestrzenie VM (nsenter).`
            : `Proces API · ${snap.hostname}. Na OCI ustaw JARVIS_HOST_EXEC=host, żeby wejść na VM.`,
        );
      })
      .catch(() => {
        if (!stop) {
          setModeNote("Nie udało się odczytać trybu hosta. Polecenie i tak zostanie wysłane.");
        }
      });
    return () => {
      stop = true;
    };
  }, []);

  useEffect(() => {
    const log = logRef.current;
    if (!log) return;
    const distance = log.scrollHeight - log.scrollTop - log.clientHeight;
    if (distance < 80) log.scrollTop = log.scrollHeight;
  }, [rows, modeNote]);

  function push(kind: Row["kind"], text: string) {
    const id = nextId.current++;
    setRows((prev) => [...prev, { id, kind, text }]);
  }

  async function run(command: string) {
    const token = getToken();
    if (!token) return;
    setBusy(true);
    push("cmd", `${cwd}$ ${command}`);
    try {
      const result = await api<ExecResult>("/admin/host/exec", {
        method: "POST",
        token,
        body: JSON.stringify({ command }),
      });
      setCwd(result.cwd || cwd);
      setExecMode(result.exec_mode);
      if (result.stdout) push("out", result.stdout);
      if (result.stderr) push("err", result.stderr);
      if (result.timed_out) push("err", "Przekroczono 20 s.");
      if (result.truncated) push("meta", "Wyjście obcięte do 64 KB.");
      if (result.exit_code !== null && result.exit_code !== 0) {
        push("meta", `kod ${result.exit_code}`);
      }
    } catch (err) {
      push("err", err instanceof Error ? err.message : "Polecenie nie doszło.");
    } finally {
      setBusy(false);
    }
  }

  function onSubmit(e: FormEvent) {
    e.preventDefault();
    const command = line.trim();
    if (!command || busy) return;
    history.current.push(command);
    histIndex.current = history.current.length;
    setLine("");
    void run(command);
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    const items = history.current;
    if (items.length === 0) return;
    const next =
      e.key === "ArrowUp"
        ? Math.max(0, histIndex.current - 1)
        : Math.min(items.length, histIndex.current + 1);
    histIndex.current = next;
    setLine(items[next] ?? "");
  }

  const prompt = cwd.length > 42 ? `…${cwd.slice(-40)}` : cwd;

  return (
    <div className="fx-stack">
      <header className="fx-page-head">
        <div>
          <p className="fx-kicker">OCI VM</p>
          <h2>Terminal</h2>
          <p>
            Polecenia wykonuje użytkownik procesu API i trafiają do dziennika audytu. Limit czasu
            to 20 sekund.
            {execMode === "host" ? " Cel: host OCI." : execMode ? " Cel: proces API." : ""}
          </p>
        </div>
      </header>
      <section className="fx-term" aria-label="Terminal OCI">
        <div className="fx-term-log" ref={logRef}>
          <div className="meta">{modeNote}</div>
          {rows.map((row) => (
            <div key={row.id} className={row.kind}>
              {row.text}
            </div>
          ))}
        </div>
        <form onSubmit={onSubmit}>
          <span>{prompt}$</span>
          <input
            value={line}
            onChange={(e) => setLine(e.target.value)}
            onKeyDown={onKeyDown}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            aria-label="Polecenie"
            style={{ flex: 1 }}
          />
          <button className="fx-btn" type="submit" disabled={busy || !line.trim()}>
            {busy ? "…" : "Wyślij"}
          </button>
        </form>
      </section>
    </div>
  );
}
