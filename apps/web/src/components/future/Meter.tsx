import { tone } from "@/lib/format";

export function Meter({
  label,
  value,
  detail,
}: {
  label: string;
  value: number;
  detail: string;
}) {
  const percent = Math.max(0, Math.min(100, value));
  return (
    <article className={`fx-card fx-meter fx-tone-${tone(percent)}`}>
      <div
        className="fx-ring"
        style={{ ["--p" as string]: String(percent) }}
        role="img"
        aria-label={`${label}: ${Math.round(percent)} procent`}
      >
        <span>{Math.round(percent)}%</span>
      </div>
      <div>
        <h3>{label}</h3>
        <p>{detail}</p>
      </div>
    </article>
  );
}
