import { StatCard } from "@/components/stat-card";
import { TokenTable } from "@/components/token-table";
import { ActivityFeed } from "@/components/activity-feed";
import { VolumeChart } from "@/components/volume-chart";
import { DominancePieChart } from "@/components/dominance-chart";
import {
  getTokens,
  getOverview,
  getRecentActivity,
  getDailyVolume,
  getMonthlyVolume,
  getVolume,
} from "@/lib/api";
import { formatCompact } from "@/lib/format";

// Mock tokens
const MOCK_TOKENS = [
  {
    address: "0x20c0000000000000000000000000000000000001",
    name: "USD Coin",
    symbol: "USDC",
    decimals: 6,
    currency: "USD",
    total_supply: "1500000000000",
    created_at_block: 100,
    created_at_tx: "0x123",
  },
  {
    address: "0x20c0000000000000000000000000000000000002",
    name: "Tether USD",
    symbol: "USDT",
    decimals: 6,
    currency: "USD",
    total_supply: "800000000000",
    created_at_block: 200,
    created_at_tx: "0x456",
  },
  {
    address: "0x20c0000000000000000000000000000000000003",
    name: "Euro Coin",
    symbol: "EURC",
    decimals: 6,
    currency: "EUR",
    total_supply: "350000000000",
    created_at_block: 300,
    created_at_tx: "0x789",
  },
];

const MOCK_TRANSFERS = [
  {
    id: 1, token_address: "0x20c0000000000000000000000000000000000001",
    from_address: "0x0000000000000000000000000000000000000000",
    to_address: "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    amount: "5000000000", memo: null, event_type: "mint",
    transaction_hash: "0xabc123", block_number: 1000500, log_index: 0,
    created_at: new Date().toISOString(),
  },
  {
    id: 2, token_address: "0x20c0000000000000000000000000000000000001",
    from_address: "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    to_address: "0x742d35cc6634c0532925a3b844bc9e7595f2bd38",
    amount: "1250000000", memo: null, event_type: "transfer",
    transaction_hash: "0xdef456", block_number: 1000498, log_index: 1,
    created_at: new Date().toISOString(),
  },
  {
    id: 3, token_address: "0x20c0000000000000000000000000000000000002",
    from_address: "0x742d35cc6634c0532925a3b844bc9e7595f2bd38",
    to_address: "0x0000000000000000000000000000000000000000",
    amount: "200000000", memo: null, event_type: "burn",
    transaction_hash: "0xghi789", block_number: 1000495, log_index: 0,
    created_at: new Date().toISOString(),
  },
  {
    id: 4, token_address: "0x20c0000000000000000000000000000000000003",
    from_address: "0xabc4567890abcdef1234567890abcdef12345678",
    to_address: "0xdef4567890abcdef1234567890abcdef12345678",
    amount: "75000000000", memo: null, event_type: "transfer",
    transaction_hash: "0xjkl012", block_number: 1000490, log_index: 2,
    created_at: new Date().toISOString(),
  },
];

const MOCK_OVERVIEW = {
  total_value_transferred: "81450000000",
  total_transactions: 4,
  active_addresses: 5,
  tracked_tokens: 3,
};

// Generate realistic mock daily data (last 30 days)
function generateMockDaily() {
  const data = [];
  const now = new Date();
  for (let i = 29; i >= 0; i--) {
    const d = new Date(now);
    d.setDate(d.getDate() - i);
    const base = 2_000_000_000 + Math.floor(Math.random() * 800_000_000);
    const trend = Math.sin(i / 5) * 500_000_000;
    data.push({
      date: d.toISOString().split("T")[0],
      volume: Math.floor(base + trend).toString(),
      transfer_count: 15 + Math.floor(Math.random() * 25),
    });
  }
  return data;
}

// Generate realistic mock monthly data (last 12 months)
function generateMockMonthly() {
  const data = [];
  const now = new Date();
  for (let i = 11; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth() - i, 1);
    const month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
    const base = 60_000_000_000 + Math.floor(Math.random() * 20_000_000_000);
    const growth = (12 - i) * 3_000_000_000;
    data.push({
      date: month,
      volume: Math.floor(base + growth).toString(),
      transfer_count: 400 + Math.floor(Math.random() * 300) + (12 - i) * 30,
    });
  }
  return data;
}

const MOCK_VOLUME = {
  tokens: [
    { token_address: "0x20c0000000000000000000000000000000000001", symbol: "USDC", total_volume: "45000000000", transfer_count: 1200 },
    { token_address: "0x20c0000000000000000000000000000000000002", symbol: "USDT", total_volume: "28000000000", transfer_count: 800 },
    { token_address: "0x20c0000000000000000000000000000000000003", symbol: "EURC", total_volume: "8450000000", transfer_count: 350 },
  ],
};

async function fetchData() {
  try {
    const [tokens, overview, activity, daily, monthly, volume] = await Promise.all([
      getTokens(),
      getOverview(),
      getRecentActivity(20),
      getDailyVolume(30),
      getMonthlyVolume(12),
      getVolume(),
    ]);
    return { tokens, overview, activity, daily, monthly, volume, live: true };
  } catch {
    return {
      tokens: MOCK_TOKENS,
      overview: MOCK_OVERVIEW,
      activity: MOCK_TRANSFERS,
      daily: generateMockDaily(),
      monthly: generateMockMonthly(),
      volume: MOCK_VOLUME,
      live: false,
    };
  }
}

export default async function DashboardPage() {
  const { tokens, overview, activity, daily, monthly, volume, live } =
    await fetchData();

  const totalVolume = Number(overview.total_value_transferred) / 1e6;

  // Compute dominance data — top 10 by supply
  const supplyDominance = [...tokens]
    .sort((a, b) => Number(b.total_supply) - Number(a.total_supply))
    .slice(0, 10)
    .map((t) => ({
      name: t.symbol || t.name || "Unknown",
      value: Number(t.total_supply) / Math.pow(10, t.decimals),
      address: t.address,
    }));

  // Top 10 by transfer volume
  const volumeDominance = [...volume.tokens]
    .sort((a, b) => Number(b.total_volume) - Number(a.total_volume))
    .slice(0, 10)
    .map((t) => ({
      name: t.symbol || "Unknown",
      value: Number(t.total_volume) / 1e6,
      address: t.token_address,
    }));

  return (
    <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
      {/* Hero Section */}
      <div className="mb-8">
        <h1 className="text-3xl font-bold tracking-tight mb-2">
          Tempo Payment Analytics
        </h1>
        <p className="text-muted">
          Real-time payment insights for TIP-20 stablecoins on the Tempo
          blockchain
          {!live && (
            <span className="ml-2 inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-accent/10 text-accent">
              Demo Mode
            </span>
          )}
        </p>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <StatCard
          label="Total Value Transferred"
          value={`$${formatCompact(totalVolume)}`}
          subtitle="All-time payment volume"
          delay={1}
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M7 17l9.2-9.2M17 17V7H7" />
            </svg>
          }
        />
        <StatCard
          label="Total Transactions"
          value={overview.total_transactions.toLocaleString()}
          subtitle="Payments processed"
          delay={2}
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 2v20M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
            </svg>
          }
        />
        <StatCard
          label="Active Addresses"
          value={overview.active_addresses.toLocaleString()}
          subtitle="Unique senders & receivers"
          delay={3}
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
              <circle cx="9" cy="7" r="4" />
              <path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
            </svg>
          }
        />
        <StatCard
          label="Tracked Stablecoins"
          value={overview.tracked_tokens.toString()}
          subtitle="TIP-20 payment tokens"
          delay={4}
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" />
              <path d="M12 6v12M6 12h12" />
            </svg>
          }
        />
      </div>

      {/* Volume Charts — full width */}
      <div className="mb-8">
        <VolumeChart dailyData={daily} monthlyData={monthly} />
      </div>

      {/* Dominance Pie Charts */}
      <div className="mb-8">
        <DominancePieChart
          supplyData={supplyDominance}
          volumeData={volumeDominance}
        />
      </div>

      {/* Token Table + Activity Feed */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <TokenTable tokens={tokens} />
        </div>
        <div>
          <ActivityFeed transfers={activity.slice(0, 10)} />
        </div>
      </div>
    </div>
  );
}
