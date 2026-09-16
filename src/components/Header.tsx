import React from "react";
import { RefreshCw, Power, Radio } from "lucide-react";
import { OtunnelDaemonStatus } from "../types";

interface HeaderProps {
  otunnelStatus: OtunnelDaemonStatus | null;
  activeSessionsCount: number;
  onToggleOtunnel: () => void;
  isTogglingOtunnel: boolean;
  onRefresh: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  otunnelStatus,
  activeSessionsCount,
  onToggleOtunnel,
  isTogglingOtunnel,
  onRefresh,
}) => {
  const isOnline = otunnelStatus?.running && otunnelStatus?.healthz_ok;

  return (
    <header className="h-14 border-b border-zinc-800/80 bg-dark-card/90 backdrop-blur px-6 flex items-center justify-between select-none">
      {/* Brand */}
      <div className="flex items-center gap-3">
        <img
          src="/app-icon.svg"
          alt="Chappie Studio"
          className="w-8 h-8 rounded-lg shadow-md border border-zinc-800/80"
        />
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-sm font-semibold tracking-wide text-zinc-100">
              CHAPPIE STUDIO
            </h1>
            <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-zinc-800/80 text-zinc-400 border border-zinc-700/50">
              v1.0 Pro
            </span>
          </div>
          <p className="text-[11px] text-zinc-500 font-mono">
            OpenAI Secure MCP Tunnel + Pi Runtime
          </p>
        </div>
      </div>

      {/* Center Status Indicators */}
      <div className="flex items-center gap-4">
        {/* Otunnel Status Badge */}
        <div className="flex items-center gap-2 px-3 py-1 rounded bg-zinc-900/90 border border-zinc-800 text-xs font-mono">
          <span
            className={`w-2 h-2 rounded-full ${
              isOnline
                ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)] animate-pulse"
                : "bg-zinc-600"
            }`}
          />
          <span className="text-zinc-400">Tunnel:</span>
          <span
            className={
              isOnline ? "text-emerald-400 font-medium" : "text-zinc-500"
            }
          >
            {isOnline ? "已连接" : "已断开"}
          </span>
          {isOnline && otunnelStatus?.latency_ms !== null && (
            <span className="text-zinc-500 text-[11px]">
              ({otunnelStatus.latency_ms}ms)
            </span>
          )}
        </div>

        {/* Workspace Sessions Count */}
        <div className="flex items-center gap-2 px-3 py-1 rounded bg-zinc-900/90 border border-zinc-800 text-xs font-mono">
          <Radio className="w-3.5 h-3.5 text-zinc-400" />
          <span className="text-zinc-400">工作区 Session:</span>
          <span
            className={
              activeSessionsCount > 0
                ? "text-emerald-400 font-medium"
                : "text-zinc-500"
            }
          >
            {activeSessionsCount} 在线
          </span>
        </div>
      </div>

      {/* Right Controls */}
      <div className="flex items-center gap-2">
        <button
          onClick={onRefresh}
          className="p-1.5 rounded hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors border border-transparent hover:border-zinc-700"
          title="全局刷新状态"
        >
          <RefreshCw className="w-4 h-4" />
        </button>

        <button
          onClick={onToggleOtunnel}
          disabled={isTogglingOtunnel}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium border transition-all ${
            isOnline
              ? "bg-rose-950/30 text-rose-300 border-rose-800/60 hover:bg-rose-900/40"
              : "bg-emerald-950/30 text-emerald-300 border-emerald-800/60 hover:bg-emerald-900/40"
          } disabled:opacity-50`}
        >
          <Power className="w-3.5 h-3.5" />
          <span>{isOnline ? "停止 Tunnel" : "启动 Tunnel"}</span>
        </button>
      </div>
    </header>
  );
};
