export function JarvisMark({ size = 36, glow = true }: { size?: number; glow?: boolean }) {
  return (
    <img
      className={glow ? "fx-mark" : "fx-mark fx-mark-quiet"}
      src="/logo.svg"
      width={size}
      height={size}
      alt=""
      draggable={false}
    />
  );
}
