import { StatCard } from "@/components/stat-card";
import { TokenTable } from "@/components/token-table";
import { ActivityFeed } from "@/components/activity-feed";
import { VolumeChart } from "@/components/volume-chart";
import { DominancePieChart } from "@/components/dominance-chart";
import { fetchDashboardData } from "@/lib/backend";
import { formatCompact } from "@/lib/format";

export const dynamic = "force-dynamic";

export default async function DashboardPage() {
  const data = await fetchDashboardData();

  if (!data) {
    return (
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="mb-8">
          <h1 className="text-3xl font-bold tracking-tight mb-2">
            Tempo Payment Analytics
          </h1>
        </div>
        <div className="rounded-xl border border-negative/30 bg-negative/5 p-6 text-center">
          <p className="text-negative font-medium mb-1">
            Unable to connect to the API
          </p>
          <p className="text-muted text-sm">
            Make sure the Tempulse API server is running and try again.
          </p>
        </div>
      </div>
    );
  }

  const { tokens, overview, activity, daily, monthly, volume } = data;
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
