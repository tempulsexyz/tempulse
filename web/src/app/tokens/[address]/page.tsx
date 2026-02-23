import { HoldersTable } from "@/components/holders-table";
import { ActivityFeed } from "@/components/activity-feed";
import { StatCard } from "@/components/stat-card";
import { TokenVolumeChart } from "@/components/token-volume-chart";
import { fetchTokenDetailData } from "@/lib/backend";
import { formatTokenAmount, truncateAddress } from "@/lib/format";

interface TokenDetailPageProps {
    params: Promise<{ address: string }>;
}

export default async function TokenDetailPage({ params }: TokenDetailPageProps) {
    const { address } = await params;
    const data = await fetchTokenDetailData(address);

    if (!data) {
        return (
            <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div className="flex items-center gap-2 text-sm text-muted mb-3">
                    <a href="/" className="hover:text-foreground transition-colors">Dashboard</a>
                    <span>/</span>
                    <a href="/tokens" className="hover:text-foreground transition-colors">Tokens</a>
                    <span>/</span>
                    <span className="text-foreground font-mono">{truncateAddress(address)}</span>
                </div>
                <div className="rounded-xl border border-negative/30 bg-negative/5 p-6 text-center">
                    <p className="text-negative font-medium mb-1">
                        Unable to load token details
                    </p>
                    <p className="text-muted text-sm">
                        Token not found or the API server is unavailable.
                    </p>
                </div>
            </div>
        );
    }

    const { token, holders, transfers, dailyVolume } = data;

    // Calculate payment velocity: volume / supply ratio
    const totalSupplyNum = Number(token.total_supply) || 1;
    const transferVolume = transfers.reduce(
        (sum, t) => sum + Number(t.amount),
        0
    );
    const velocity = transferVolume / totalSupplyNum;

    // Count unique active addresses from recent transfers
    const activeAddresses = new Set<string>();
    transfers.forEach((tx) => {
        if (tx.from_address !== "0x0000000000000000000000000000000000000000") {
            activeAddresses.add(tx.from_address);
        }
        if (tx.to_address !== "0x0000000000000000000000000000000000000000") {
            activeAddresses.add(tx.to_address);
        }
    });

    return (
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            {/* Token Header */}
            <div className="mb-8 animate-fade-in">
                <div className="flex items-center gap-2 text-sm text-muted mb-3">
                    <a href="/" className="hover:text-foreground transition-colors">Dashboard</a>
                    <span>/</span>
                    <a href="/tokens" className="hover:text-foreground transition-colors">Tokens</a>
                    <span>/</span>
                    <span className="text-foreground">{token.symbol || "Token"}</span>
                </div>

                <div className="flex items-center gap-4">
                    <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-accent to-accent-secondary flex items-center justify-center shadow-lg shadow-accent/20">
                        <span className="text-2xl font-bold text-white">
                            {token.symbol ? token.symbol[0] : "?"}
                        </span>
                    </div>
                    <div>
                        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-3">
                            {token.name || "Unknown Token"}
                            <span className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-accent/10 text-accent-secondary border border-accent/20">
                                {token.symbol}
                            </span>
                        </h1>
                        <div className="flex items-center gap-3 mt-1 text-sm text-muted">
                            <span className="font-mono">{truncateAddress(token.address, 10)}</span>
                            <span>•</span>
                            <span>Currency: {token.currency || "—"}</span>
                            <span>•</span>
                            <span>Created at block {token.created_at_block.toLocaleString()}</span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Stats — payment-focused */}
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
                <StatCard
                    label="Transfer Volume"
                    value={formatTokenAmount(transferVolume.toString(), token.decimals)}
                    subtitle="From recent transactions"
                    delay={1}
                    icon={
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M7 17l9.2-9.2M17 17V7H7" />
                        </svg>
                    }
                />
                <StatCard
                    label="Total Payments"
                    value={transfers.length.toString()}
                    subtitle="Recent transactions"
                    delay={2}
                    icon={
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M12 2v20M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
                        </svg>
                    }
                />
                <StatCard
                    label="Active Addresses"
                    value={activeAddresses.size.toString()}
                    subtitle="Unique senders & receivers"
                    delay={3}
                    icon={
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
                            <circle cx="9" cy="7" r="4" />
                        </svg>
                    }
                />
                <StatCard
                    label="Payment Velocity"
                    value={`${velocity.toFixed(2)}x`}
                    subtitle="Volume / Supply ratio"
                    delay={4}
                    icon={
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
                        </svg>
                    }
                />
            </div>

            {/* Volume + Activity Charts — full width */}
            <div className="mb-8">
                <TokenVolumeChart
                    data={dailyVolume}
                    symbol={token.symbol || "Token"}
                    decimals={token.decimals}
                />
            </div>

            {/* Holders + Transfers */}
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                <div className="lg:col-span-2">
                    <HoldersTable
                        holders={holders}
                        totalSupply={token.total_supply}
                        decimals={token.decimals}
                    />
                </div>
                <div>
                    <ActivityFeed transfers={transfers} showToken={false} />
                </div>
            </div>
        </div>
    );
}
