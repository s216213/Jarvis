"use client";

import { useEffect, useSyncExternalStore, type ReactNode } from "react";
import { useRouter } from "next/navigation";
import { AppShell } from "@/components/future/AppShell";
import { getToken, getUser, isAdmin, subscribeSession } from "@/lib/auth";

function adminAllowed() {
  return Boolean(getToken() && isAdmin(getUser()));
}

export default function AdminLayout({ children }: { children: ReactNode }) {
  const router = useRouter();
  const allowed = useSyncExternalStore(subscribeSession, adminAllowed, () => false);

  useEffect(() => {
    if (!getToken()) router.replace("/login");
    else if (!isAdmin(getUser())) router.replace("/chat");
  }, [router, allowed]);

  return <AppShell>{children}</AppShell>;
}
