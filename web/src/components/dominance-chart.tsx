"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer, Legend } from "recharts";
import { useState } from "react";

interface DominanceEntry {
    name: string;
    value: number;
    address: string;
}

interface DominancePieChartProps {
    supplyData: DominanceEntry[];
    volumeData: DominanceEntry[];
}

type View = "supply" | "volume";

const COLORS = [
    "#6366f1", // indigo
    "#818cf8", // light indigo
    "#34d399", // emerald
    "#f59e0b", // amber
    "#f87171", // red
    "#a78bfa", // violet
    "#38bdf8", // sky
    "#fb923c", // orange
    "#4ade80", // green
    "#e879f9", // fuchsia
];

function formatValue(value: number): string {
    if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(2)}M`;
    if (value >= 1_000) return `$${(value / 1_000).toFixed(2)}K`;
    return `$${value.toFixed(2)}`;
}

interface CustomLabelProps {
    cx?: number;
    cy?: number;
    midAngle?: number;
    innerRadius?: number;
    outerRadius?: number;
    percent?: number;
    name?: string;
}

function renderCustomLabel({
    cx,
    cy,
    midAngle,
    innerRadius,
    outerRadius,
    percent,
}: CustomLabelProps) {
    if (!percent || percent < 0.05) return null;
    const mid = midAngle ?? 0;
    const inner = innerRadius ?? 0;
    const outer = outerRadius ?? 0;
    const RADIAN = Math.PI / 180;
    const radius = inner + (outer - inner) * 0.5;
    const x = (cx ?? 0) + radius * Math.cos(-mid * RADIAN);
    const y = (cy ?? 0) + radius * Math.sin(-mid * RADIAN);

    return (
        <text
            x={x}
            y={y}
            fill="#e4e6ef"
            textAnchor="middle"
            dominantBaseline="central"
            fontSize={12}
            fontWeight={600}
        >
            {`${(percent * 100).toFixed(0)}%`}
        </text>
    );
}

export function DominancePieChart({
    supplyData,
    volumeData,
}: DominancePieChartProps) {
    const [view, setView] = useState<View>("volume");

    const data = view === "supply" ? supplyData : volumeData;
    const total = data.reduce((sum, d) => sum + d.value, 0);

    return (
        <div className="bg-card border border-border rounded-2xl p-6 animate-fade-in">
            <div className="flex items-center justify-between mb-6">
                <div>
                    <h2 className="text-lg font-semibold">Stablecoin Dominance</h2>
                    <p className="text-sm text-muted mt-0.5">Top 10 by market share</p>
                </div>
                <div className="flex bg-background rounded-lg p-0.5 border border-border">
                    <button
                        onClick={() => setView("volume")}
                        className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${view === "volume"
                            ? "bg-accent text-white shadow"
                            : "text-muted hover:text-foreground"
                            }`}
                    >
                        By Volume
                    </button>
                    <button
                        onClick={() => setView("supply")}
                        className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${view === "supply"
                            ? "bg-accent text-white shadow"
                            : "text-muted hover:text-foreground"
                            }`}
                    >
                        By Supply
                    </button>
                </div>
            </div>

            {data.length > 0 ? (
                <div className="flex flex-col lg:flex-row items-center gap-6">
                    <div className="w-full lg:w-1/2" style={{ minHeight: 280 }}>
                        <ResponsiveContainer width="100%" height={280}>
                            <PieChart>
                                <Pie
                                    data={data}
                                    cx="50%"
                                    cy="50%"
                                    innerRadius={60}
                                    outerRadius={120}
                                    paddingAngle={2}
                                    dataKey="value"
                                    label={renderCustomLabel}
                                    labelLine={false}
                                    animationBegin={0}
                                    animationDuration={800}
                                >
                                    {data.map((_, index) => (
                                        <Cell
                                            key={`cell-${index}`}
                                            fill={COLORS[index % COLORS.length]}
                                            stroke="transparent"
                                        />
                                    ))}
                                </Pie>
                                <Tooltip
                                    contentStyle={{
                                        backgroundColor: "#12141f",
                                        border: "1px solid #1e2233",
                                        borderRadius: "12px",
                                        color: "#e4e6ef",
                                        fontSize: "13px",
                                    }}
                                    formatter={(value: number | undefined) => [
                                        formatValue(value ?? 0),
                                        view === "volume" ? "Transfer Volume" : "Supply",
                                    ]}
                                    labelStyle={{ color: "#6b7194" }}
                                />
                            </PieChart>
                        </ResponsiveContainer>
                    </div>

                    {/* Legend table */}
                    <div className="w-full lg:w-1/2">
                        <div className="space-y-2">
                            {data.map((entry, index) => {
                                const pct = total > 0 ? (entry.value / total) * 100 : 0;
                                return (
                                    <div
                                        key={entry.address}
                                        className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-card-hover transition-colors"
                                    >
                                        <div
                                            className="w-3 h-3 rounded-full shrink-0"
                                            style={{ backgroundColor: COLORS[index % COLORS.length] }}
                                        />
                                        <span className="text-sm font-medium flex-1 truncate">
                                            {entry.name}
                                        </span>
                                        <span className="text-sm text-muted font-mono">
                                            {formatValue(entry.value)}
                                        </span>
                                        <span className="text-xs text-accent-secondary font-semibold w-14 text-right">
                                            {pct.toFixed(1)}%
                                        </span>
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                </div>
            ) : (
                <div className="h-[280px] flex items-center justify-center text-muted">
                    <div className="text-center">
                        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="mx-auto mb-3 opacity-30">
                            <path d="M21.21 15.89A10 10 0 1 1 8 2.83" />
                            <path d="M22 12A10 10 0 0 0 12 2v10z" />
                        </svg>
                        <p>No token data available</p>
                    </div>
                </div>
            )}
        </div>
    );
}
