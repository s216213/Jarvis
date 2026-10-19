export type PublicUser = {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_active: boolean;
  created_at: string;
};

export type Message = {
  id: string;
  user_id: string;
  role: string;
  content: string;
  created_at: string;
};

export type UsageSnapshot = {
  user_rpm_used: number;
  user_rpm_limit: number;
  user_daily_used: number;
  user_daily_limit: number;
  global_rpm_used: number;
  global_rpm_limit: number;
  global_daily_used: number;
  global_daily_limit: number;
};

export type LlmStatus = {
  provider: string;
  model: string;
  source: string;
  usage: UsageSnapshot;
  limits: {
    user_rpm: number;
    user_daily: number;
    global_rpm: number;
    global_daily: number;
    max_input_chars: number;
    context_messages: number;
    max_inflight: number;
  };
};

export class ApiError extends Error {
  status: number;
  body: Record<string, unknown>;

  constructor(status: number, body: Record<string, unknown>) {
    super(String(body.message ?? body.error ?? `HTTP ${status}`));
    this.status = status;
    this.body = body;
  }

  get isRateLimited() {
    return this.status === 429 || this.body.error === "rate_limited";
  }

  get retryAfterSec() {
    return Number(this.body.retry_after_sec ?? 0);
  }
}

function apiBase() {
  const base = process.env.NEXT_PUBLIC_API_URL;
  if (!base) {
    throw new Error("NEXT_PUBLIC_API_URL is not set");
  }
  return base.replace(/\/$/, "");
}

export async function api<T>(
  path: string,
  options: RequestInit & { token?: string | null } = {},
): Promise<T> {
  const { token, headers, ...rest } = options;
  const res = await fetch(`${apiBase()}${path}`, {
    ...rest,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...headers,
    },
  });

  const text = await res.text();
  const body = text ? (JSON.parse(text) as Record<string, unknown>) : {};

  if (!res.ok) {
    throw new ApiError(res.status, body);
  }

  return body as T;
}
