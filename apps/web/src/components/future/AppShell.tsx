"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState, useSyncExternalStore, type ReactNode, type SVGProps } from "react";
import { api, type JarvisVersion } from "@/lib/api";
import { clearSession, getToken, getUser, isAdmin, subscribeSession } from "@/lib/auth";
import { JarvisVersionLines } from "@/components/future/JarvisVersion";
import { browserName, deviceFingerprint, deviceSystem } from "@/lib/device";
import { CLIENT_VERSION, FUTURE_UI } from "@/lib/future";

type IconName =
  | "chat"
  | "grid"
  | "cpu"
  | "term"
  | "users"
  | "devices"
  | "flag"
  | "out";

const NAV: Array<{
  href: string;
  label: string;
  icon: IconName;
  exact?: boolean;
  admin?: boolean;
}> = [
  { href: "/chat", label: "Czat", icon: "chat", exact: true },
  { href: "/admin", label: "Przegląd", icon: "grid", exact: true, admin: true },
  { href: "/admin/resources", label: "Zasoby OCI", icon: "cpu", admin: true },
  { href: "/admin/terminal", label: "Terminal", icon: "term", admin: true },
  { href: "/admin/accounts", label: "Konta", icon: "users", admin: true },
  { href: "/admin/devices", label: "Urządzenia", icon: "devices", admin: true },
  { href: "/admin/flags", label: "Feature flags", icon: "flag", admin: true },
];

export function AppShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const user = useSyncExternalStore(subscribeSession, getUser, () => null);
  const [clock, setClock] = useState("");
  const [greeting, setGreeting] = useState("Witaj");
  const [jarvis, setJarvis] = useState<JarvisVersion | null>(null);

  useEffect(() => {
    const token = getToken();
    if (!token) {
      router.replace("/login");
      return;
    }

    const tick = () => {
      const now = new Date();
      setGreeting(now.getHours() < 18 ? "Dzień dobry" : "Dobry wieczór");
      setClock(
        new Intl.DateTimeFormat("pl-PL", {
          hour: "2-digit",
          minute: "2-digit",
          weekday: "short",
          day: "numeric",
          month: "short",
        }).format(now),
      );
    };
    const start = window.setTimeout(tick, 0);
    const timer = window.setInterval(tick, 30_000);

    void api<JarvisVersion>("/version", { token }).then(setJarvis).catch(() => {});

    const fingerprint = deviceFingerprint();
    const name = `${browserName()} · web`;
    void deviceSystem().then((system) => {
      api("/me/devices", {
        method: "POST",
        token,
        body: JSON.stringify({
          name,
          platform: "web",
          ui_version: FUTURE_UI,
          client_version: CLIENT_VERSION,
          system_name: system.name,
          system_version: system.version,
          fingerprint,
        }),
      })
        .then(() => {
          window.dispatchEvent(new Event("jarvis-device"));
        })
        .catch(() => {});
    });

    return () => {
      window.clearTimeout(start);
      window.clearInterval(timer);
    };
  }, [router]);

  function logout() {
    clearSession();
    router.replace("/login");
  }

  const links = NAV.filter((item) => !item.admin || isAdmin(user));

  return (
    <div className="fx-app">
      <aside className="fx-side">
        <Link href="/chat" className="fx-brand">
          <span className="fx-mark" aria-hidden />
          <span className="fx-brand-copy">
            <strong>Jarvis</strong>
            {jarvis?.brain_version ? <small>{jarvis.brain_version}</small> : null}
          </span>
        </Link>
        <nav className="fx-nav" aria-label="Główne">
          {links.map((item) => {
            const active = item.exact
              ? pathname === item.href
              : pathname === item.href || pathname?.startsWith(`${item.href}/`);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={active ? "active" : undefined}
                aria-current={active ? "page" : undefined}
              >
                <Icon name={item.icon} />
                <span>{item.label}</span>
              </Link>
            );
          })}
        </nav>
        <div className="fx-side-foot">
          <span className="fx-user">{user?.display_name ?? "…"}</span>
          <button type="button" className="fx-btn fx-btn-ghost" onClick={logout}>
            <Icon name="out" />
            Wyloguj
          </button>
        </div>
      </aside>
      <div className="fx-main">
        <header className="fx-top">
          <div>
            <p className="fx-kicker">{FUTURE_UI}</p>
            <h1>
              {greeting}
              {user ? `, ${user.display_name}` : ""}
            </h1>
          </div>
          <div className="fx-top-meta">
            <JarvisVersionLines info={jarvis} />
            <span className="fx-pill">
              <span className="fx-dot" aria-hidden />
              online
            </span>
            <time suppressHydrationWarning>{clock}</time>
          </div>
        </header>
        <div className="fx-content">{children}</div>
      </div>
    </div>
  );
}

function Icon({ name, ...props }: { name: IconName } & SVGProps<SVGSVGElement>) {
  const common = {
    width: 18,
    height: 18,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.7,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
    ...props,
  };
  switch (name) {
    case "chat":
      return (
        <svg {...common}>
          <path d="M6 6.5h12v8H9l-3 3v-11z" />
        </svg>
      );
    case "grid":
      return (
        <svg {...common}>
          <rect x="4" y="4" width="6.5" height="6.5" rx="1.4" />
          <rect x="13.5" y="4" width="6.5" height="6.5" rx="1.4" />
          <rect x="4" y="13.5" width="6.5" height="6.5" rx="1.4" />
          <rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.4" />
        </svg>
      );
    case "cpu":
      return (
        <svg {...common}>
          <rect x="6" y="6" width="12" height="12" rx="2" />
          <path d="M9 3.5v2.5M12 3.5v2.5M15 3.5v2.5M9 18v2.5M12 18v2.5M15 18v2.5M3.5 9H6M3.5 12H6M3.5 15H6M18 9h2.5M18 12h2.5M18 15h2.5" />
        </svg>
      );
    case "term":
      return (
        <svg {...common}>
          <path d="M5 7l5 5-5 5M12 17h7" />
        </svg>
      );
    case "users":
      return (
        <svg {...common}>
          <circle cx="9" cy="9" r="3" />
          <path d="M4.5 18.5c.6-2.4 2.4-3.5 4.5-3.5s3.9 1.1 4.5 3.5" />
          <circle cx="16.5" cy="9.5" r="2.2" />
          <path d="M16 15c1.8.2 3.2 1.2 3.8 3.2" />
        </svg>
      );
    case "devices":
      return (
        <svg {...common}>
          <rect x="7" y="3.5" width="10" height="17" rx="2" />
          <path d="M11 18.5h2" />
        </svg>
      );
    case "flag":
      return (
        <svg {...common}>
          <path d="M6 4.5v15M6 5.5h10l-2 3.2 2 3.3H6" />
        </svg>
      );
    case "out":
      return (
        <svg {...common}>
          <path d="M10 7V5.5A1.5 1.5 0 0 1 11.5 4h7A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 10 18.5V17" />
          <path d="M4 12h10M11 9l3 3-3 3" />
        </svg>
      );
  }
}
