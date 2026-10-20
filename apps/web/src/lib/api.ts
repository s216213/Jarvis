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

export type LlmPoolSlot = {
  provider: string;
  model: string;
  source: string;
};

export type LlmModelInfo = {
  id: string;
  display_name: string;
  supports_chat: boolean;
  provider: string;
};

export type LlmStatus = {
  provider: string;
  model: string;
  source: string;
  route_provider: string;
  route_model: string;
  route_note: string;
  providers: string[];
  pool?: LlmPoolSlot[];
  models: LlmModelInfo[];
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

export type HostCore = {
  name: string;
  usage_percent: number;
};

export type HostDisk = {
  name: string;
  mount: string;
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  scope: string;
};

export type HostSnapshot = {
  mode: string;
  recommended_poll_sec: number;
  exec_mode: string;
  hostname: string;
  os: string;
  kernel: string;
  arch: string;
  uptime_sec: number;
  load_1: number;
  load_5: number;
  load_15: number;
  cpu_count: number;
  cpu_usage_percent: number;
  cores: HostCore[];
  memory_total_bytes: number;
  memory_used_bytes: number;
  memory_available_bytes: number;
  swap_total_bytes: number;
  swap_used_bytes: number;
  disks: HostDisk[];
  process_count: number;
  collected_at: string;
};

export type ExecResult = {
  stdout: string;
  stderr: string;
  exit_code: number | null;
  cwd: string;
  truncated: boolean;
  timed_out: boolean;
  exec_mode: string;
};

export type JarvisVersion = {
  brain_version: string;
  system_name: string;
  system_version: string;
};

export type AdminDevice = {
  id: string;
  user_id: string;
  user_email: string;
  user_display_name: string;
  name: string;
  platform: string;
  ui_version: string;
  client_version: string;
  system_name: string;
  system_version: string;
  fingerprint: string;
  last_seen_at: string;
  is_revoked: boolean;
  created_at: string;
};

export type FeatureFlag = {
  key: string;
  enabled: boolean;
  description: string;
  updated_at: string;
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
