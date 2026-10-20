"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { JarvisMark } from "@/components/future/JarvisMark";
import { getToken } from "@/lib/auth";

export default function HomePage() {
  const router = useRouter();

  useEffect(() => {
    router.replace(getToken() ? "/chat" : "/login");
  }, [router]);

  return (
    <div className="fx-boot">
      <JarvisMark size={88} />
      <span>Jarvis</span>
    </div>
  );
}
