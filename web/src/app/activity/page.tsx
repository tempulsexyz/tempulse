import { ActivityFeed } from "@/components/activity-feed";
import { fetchRecentActivity } from "@/lib/backend";

export const dynamic = "force-dynamic";

export default async function ActivityPage() {
    const activity = await fetchRecentActivity(100);

    return (
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div className="mb-8">
                <h1 className="text-3xl font-bold tracking-tight mb-2">Activity</h1>
                <p className="text-muted">
                    Recent stablecoin transfers, mints, and burns on Tempo
                </p>
            </div>
            {activity ? (
                <ActivityFeed transfers={activity} />
            ) : (
                <div className="rounded-xl border border-negative/30 bg-negative/5 p-6 text-center">
                    <p className="text-negative font-medium mb-1">
                        Unable to load activity
                    </p>
                    <p className="text-muted text-sm">
                        Make sure the Tempulse API server is running and try again.
                    </p>
                </div>
            )}
        </div>
    );
}
