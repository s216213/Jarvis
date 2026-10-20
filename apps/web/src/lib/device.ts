const FINGERPRINT_KEY = "jarvis_device_fp";

export function deviceFingerprint() {
  const existing = localStorage.getItem(FINGERPRINT_KEY);
  if (existing) return existing;
  const created = crypto.randomUUID();
  localStorage.setItem(FINGERPRINT_KEY, created);
  return created;
}

export async function deviceSystem(): Promise<{ name: string; version: string }> {
  const nav = navigator as Navigator & {
    userAgentData?: {
      platform?: string;
      getHighEntropyValues?: (
        hints: string[],
      ) => Promise<{ platform?: string; platformVersion?: string }>;
    };
  };
  const data = nav.userAgentData;
  if (data?.getHighEntropyValues) {
    try {
      const hints = await Promise.race([
        data.getHighEntropyValues(["platform", "platformVersion"]),
        new Promise<null>((resolve) => {
          window.setTimeout(() => resolve(null), 800);
        }),
      ]);
      const name = (hints?.platform || data.platform || "").trim();
      if (hints && name) return { name, version: (hints.platformVersion || "").trim() };
    } catch {
      // The browser refused high-entropy hints. Fall back to the user agent.
    }
  }
  return systemFromUserAgent(navigator.userAgent);
}

function systemFromUserAgent(ua: string): { name: string; version: string } {
  const windows = ua.match(/Windows NT ([\d.]+)/);
  if (windows) return { name: "Windows", version: windows[1] };
  const mac = ua.match(/Mac OS X ([\d_]+)/);
  if (mac) return { name: "macOS", version: mac[1].replace(/_/g, ".") };
  const android = ua.match(/Android ([\d.]+)/);
  if (android) return { name: "Android", version: android[1] };
  const ios = ua.match(/(?:iPhone|iPad|iPod).*OS ([\d_]+)/);
  if (ios) return { name: "iOS", version: ios[1].replace(/_/g, ".") };
  if (ua.includes("Linux")) return { name: "Linux", version: "" };
  return { name: "unknown", version: "" };
}

export function browserName() {
  const ua = navigator.userAgent;
  if (ua.includes("Edg/")) return "Edge";
  if (ua.includes("Chrome/")) return "Chrome";
  if (ua.includes("Firefox/")) return "Firefox";
  if (ua.includes("Safari/")) return "Safari";
  return "Web";
}
