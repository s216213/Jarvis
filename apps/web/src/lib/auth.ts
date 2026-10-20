import type { PublicUser } from "./api";

const TOKEN_KEY = "jarvis_token";
const USER_KEY = "jarvis_user";

const listeners = new Set<() => void>();
let cachedRaw: string | null | undefined;
let cachedUser: PublicUser | null = null;

function emitSession() {
  cachedRaw = undefined;
  listeners.forEach((listener) => listener());
}

export function subscribeSession(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(TOKEN_KEY);
}

export function getUser(): PublicUser | null {
  if (typeof window === "undefined") return null;
  const raw = localStorage.getItem(USER_KEY);
  if (raw === cachedRaw) return cachedUser;
  cachedRaw = raw;
  if (!raw) {
    cachedUser = null;
    return null;
  }
  try {
    cachedUser = JSON.parse(raw) as PublicUser;
  } catch {
    cachedUser = null;
  }
  return cachedUser;
}

export function setSession(token: string, user: PublicUser) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(USER_KEY, JSON.stringify(user));
  emitSession();
}

export function clearSession() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(USER_KEY);
  emitSession();
}

export function isAdmin(user: PublicUser | null) {
  return user?.role === "admin";
}
