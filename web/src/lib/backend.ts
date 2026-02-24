"use server";

import {
  type Token,
  type Transfer,
  type Account,
  type VolumeResponse,
  type OverviewResponse,
  type TimeSeriesEntry,
} from "./api";

const API_BASE = process.env.API_URL || "http://localhost:3001";

interface ApiResponse<T> {
  success: boolean;
  data: T;
}

async function apiFetch<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    cache: "no-store",
  });
  if (!res.ok) {
    throw new Error(`API error: ${res.status} ${res.statusText}`);
  }
  const json: ApiResponse<T> = await res.json();
  if (!json.success) {
    throw new Error("API returned unsuccessful response");
  }
  return json.data;
}

// ─── Server-side data fetchers ──────────────────────────────────────────────
// Each returns null on failure instead of throwing, so pages can show error UI.

export async function fetchTokens(): Promise<Token[] | null> {
  try {
    return await apiFetch<Token[]>("/api/v1/tokens");
  } catch (e) {
    console.error("Failed to fetch tokens:", e);
    return null;
  }
}

export async function fetchToken(address: string): Promise<Token | null> {
  try {
    return await apiFetch<Token>(`/api/v1/tokens/${address}`);
  } catch (e) {
    console.error(`Failed to fetch token ${address}:`, e);
    return null;
  }
}

export async function fetchTokenHolders(
  address: string,
  limit = 50
): Promise<Account[] | null> {
  try {
    return await apiFetch<Account[]>(
      `/api/v1/tokens/${address}/holders?limit=${limit}`
    );
  } catch (e) {
    console.error(`Failed to fetch holders for ${address}:`, e);
    return null;
  }
}

export async function fetchTokenTransfers(
  address: string,
  limit = 50
): Promise<Transfer[] | null> {
  try {
    return await apiFetch<Transfer[]>(
      `/api/v1/tokens/${address}/transfers?limit=${limit}`
    );
  } catch (e) {
    console.error(`Failed to fetch transfers for ${address}:`, e);
    return null;
  }
}

export async function fetchVolume(): Promise<VolumeResponse | null> {
  try {
    return await apiFetch<VolumeResponse>("/api/v1/stats/volume");
  } catch (e) {
    console.error("Failed to fetch volume:", e);
    return null;
  }
}

export async function fetchOverview(): Promise<OverviewResponse | null> {
  try {
    return await apiFetch<OverviewResponse>("/api/v1/stats/overview");
  } catch (e) {
    console.error("Failed to fetch overview:", e);
    return null;
  }
}

export async function fetchRecentActivity(
  limit = 50
): Promise<Transfer[] | null> {
  try {
    return await apiFetch<Transfer[]>(`/api/v1/activity/recent?limit=${limit}`);
  } catch (e) {
    console.error("Failed to fetch recent activity:", e);
    return null;
  }
}

export async function fetchDailyVolume(
  limit = 90
): Promise<TimeSeriesEntry[] | null> {
  try {
    return await apiFetch<TimeSeriesEntry[]>(
      `/api/v1/stats/daily?limit=${limit}`
    );
  } catch (e) {
    console.error("Failed to fetch daily volume:", e);
    return null;
  }
}

export async function fetchMonthlyVolume(
  limit = 24
): Promise<TimeSeriesEntry[] | null> {
  try {
    return await apiFetch<TimeSeriesEntry[]>(
      `/api/v1/stats/monthly?limit=${limit}`
    );
  } catch (e) {
    console.error("Failed to fetch monthly volume:", e);
    return null;
  }
}

export async function fetchTokenDailyVolume(
  address: string,
  limit = 90
): Promise<TimeSeriesEntry[] | null> {
  try {
    return await apiFetch<TimeSeriesEntry[]>(
      `/api/v1/tokens/${address}/volume/daily?limit=${limit}`
    );
  } catch (e) {
    console.error(`Failed to fetch daily volume for ${address}:`, e);
    return null;
  }
}

// ─── Composite fetchers ─────────────────────────────────────────────────────

export interface DashboardData {
  tokens: Token[];
  overview: OverviewResponse;
  activity: Transfer[];
  daily: TimeSeriesEntry[];
  monthly: TimeSeriesEntry[];
  volume: VolumeResponse;
}

export async function fetchDashboardData(): Promise<DashboardData | null> {
  const [tokens, overview, activity, daily, monthly, volume] =
    await Promise.all([
      fetchTokens(),
      fetchOverview(),
      fetchRecentActivity(20),
      fetchDailyVolume(30),
      fetchMonthlyVolume(12),
      fetchVolume(),
    ]);

  // If any critical data is missing, return null
  if (!tokens || !overview || !activity || !daily || !monthly || !volume) {
    return null;
  }

  return { tokens, overview, activity, daily, monthly, volume };
}

export interface TokenDetailData {
  token: Token;
  holders: Account[];
  transfers: Transfer[];
  dailyVolume: TimeSeriesEntry[];
}

export async function fetchTokenDetailData(
  address: string
): Promise<TokenDetailData | null> {
  const [token, holders, transfers, dailyVolume] = await Promise.all([
    fetchToken(address),
    fetchTokenHolders(address, 20),
    fetchTokenTransfers(address, 20),
    fetchTokenDailyVolume(address, 30),
  ]);

  if (!token || !holders || !transfers || !dailyVolume) {
    return null;
  }

  return { token, holders, transfers, dailyVolume };
}
