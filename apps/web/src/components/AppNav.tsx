"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { clearSession, getUser, isAdmin } from "@/lib/auth";

export function AppNav() {
  const pathname = usePathname();
  const router = useRouter();
  const user = getUser();

  function logout() {
    clearSession();
    router.replace("/login");
  }

  return (
    <header>
      <h1 className="brand">
        Jar<span>vis</span>
      </h1>
      <p className="lede">Asystent tekstowy — zalogowani użytkownicy.</p>
      <nav className="nav">
        <Link className={pathname === "/chat" ? "active" : ""} href="/chat">
          Czat
        </Link>
        {isAdmin(user) ? (
          <Link
            className={pathname?.startsWith("/admin") ? "active" : ""}
            href="/admin"
          >
            Admin
          </Link>
        ) : null}
        <span className="hint">{user?.display_name}</span>
        <button type="button" className="linkish" onClick={logout}>
          Wyloguj
        </button>
      </nav>
    </header>
  );
}
