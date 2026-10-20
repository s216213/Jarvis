import type { JarvisVersion as JarvisVersionInfo } from "@/lib/api";
import { CLIENT_VERSION } from "@/lib/future";

export function jarvisSystem(info: JarvisVersionInfo | null) {
  if (!info) return "";
  return [info.system_name, info.system_version].filter(Boolean).join(" ");
}

export function JarvisVersionLines({ info }: { info: JarvisVersionInfo | null }) {
  const system = jarvisSystem(info);
  return (
    <dl className="fx-version">
      <div>
        <dt>UI</dt>
        <dd>{CLIENT_VERSION}</dd>
      </div>
      <div>
        <dt>Brain</dt>
        <dd>{info?.brain_version || "—"}</dd>
      </div>
      <div>
        <dt>System</dt>
        <dd>{system || "—"}</dd>
      </div>
    </dl>
  );
}
