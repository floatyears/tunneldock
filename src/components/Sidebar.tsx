import React from "react";
import {
  Cpu,
  Layers,
  Activity,
  History,
  Settings,
  ExternalLink,
  Inbox,
  ShieldCheck,
} from "lucide-react";
import { McpMode, TunnelSettings } from "../types";
import { APP_VERSION } from "../version";
import { useTranslation } from "../i18n";

export type NavTab =
  | "env"
  | "workspaces"
  | "patches"
  | "health"
  | "history"
  | "settings";

interface SidebarProps {
  currentTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  missingEnvCount: number;
  activeWorkspacesCount: number;
  doctorPassed: boolean;
  historyCount: number;
  settings: TunnelSettings | null;
  mcpMode: McpMode;
  modeSwitchBusy: boolean;
  onSwitchMode: (mode: McpMode) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentTab,
  onSelectTab,
  missingEnvCount,
  activeWorkspacesCount,
  doctorPassed,
  historyCount,
  settings,
  mcpMode,
  modeSwitchBusy,
  onSwitchMode,
}) => {
  const { t } = useTranslation();

  const navItems = [
    {
      id: "env" as NavTab,
      label: t("sidebar.nav_env"),
      icon: Cpu,
      badge:
        missingEnvCount > 0
          ? t("sidebar.env_badge_issues", { count: missingEnvCount })
          : t("sidebar.env_badge_ok"),
      badgeColor:
        missingEnvCount > 0
          ? "bg-amber-500/10 text-amber-400 border-amber-500/30"
          : "bg-emerald-500/10 text-emerald-400 border-emerald-500/30",
    },
    {
      id: "workspaces" as NavTab,
      label: t(mcpMode === "readonly" ? "sidebar.nav_workspaces_readonly" : "sidebar.nav_workspaces"),
      icon: Layers,
      badge: t(mcpMode === "readonly" ? "sidebar.ws_badge_accessible" : "sidebar.ws_badge_running", { count: activeWorkspacesCount }),
      badgeColor:
        activeWorkspacesCount > 0
          ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/30"
          : "bg-zinc-800 text-zinc-400 border-zinc-700",
    },
    {
      id: "patches" as NavTab,
      label: t("sidebar.nav_patches"),
      icon: Inbox,
      badge: null,
      badgeColor: "",
    },
    {
      id: "health" as NavTab,
      label: t("sidebar.nav_health"),
      icon: Activity,
      badge: doctorPassed
        ? t("sidebar.health_badge_ready")
        : t("sidebar.health_badge_pending"),
      badgeColor: doctorPassed
        ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/30"
        : "bg-amber-500/10 text-amber-400 border-amber-500/30",
    },
    {
      id: "history" as NavTab,
      label: t("sidebar.nav_history"),
      icon: History,
      badge: t("sidebar.history_badge_count", { count: historyCount }),
      badgeColor: "bg-zinc-800 text-zinc-400 border-zinc-700",
    },
    {
      id: "settings" as NavTab,
      label: t("sidebar.nav_settings"),
      icon: Settings,
      badge: null,
      badgeColor: "",
    },
  ];

  return (
    <aside className="w-64 border-r border-zinc-800/80 bg-dark-bg/95 flex flex-col justify-between select-none">
      {/* Navigation Links */}
      <div className="p-3 space-y-1">
        <div className="mb-3 rounded-lg border border-zinc-800 bg-zinc-950/70 p-2.5">
          <div className="mb-2 flex items-center gap-1.5 px-1 text-[10px] font-mono uppercase tracking-wider text-zinc-500">
            <ShieldCheck className="h-3 w-3" />
            <span>{t("sidebar.run_mode")}</span>
          </div>
          <div className="grid grid-cols-2 gap-1 rounded-md bg-zinc-900 p-1">
            {(["readonly", "full"] as McpMode[]).map((mode) => {
              const targetId = mode === "readonly" ? settings?.safe_tunnel_id : settings?.full_tunnel_id;
              const otherId = mode === "readonly" ? settings?.full_tunnel_id : settings?.safe_tunnel_id;
              const normalizedTargetId = targetId?.trim() ?? "";
              const normalizedOtherId = otherId?.trim() ?? "";
              const canSwitch = Boolean(
                settings?.api_key.trim() &&
                  normalizedTargetId &&
                  normalizedTargetId !== normalizedOtherId
              );
              const active = mcpMode === mode;
              return (
                <button
                  key={mode}
                  type="button"
                  aria-pressed={active}
                  disabled={modeSwitchBusy || active}
                  onClick={() => {
                    if (!canSwitch) {
                      onSelectTab("settings");
                      return;
                    }
                    onSwitchMode(mode);
                  }}
                  title={
                    !canSwitch
                      ? t("settings_view.mode_switch_setup_hint")
                      : t(mode === "readonly" ? "sidebar.mode_safe_tooltip" : "sidebar.mode_full_tooltip")
                  }
                  className={active
                    ? "rounded px-2 py-2 text-[11px] font-semibold bg-emerald-950/60 text-emerald-300 border border-emerald-700/60"
                    : "rounded px-2 py-2 text-[11px] font-medium bg-zinc-900 text-zinc-400 border border-transparent hover:border-zinc-700 hover:text-zinc-200 disabled:opacity-40"}
                >
                  {t(mode === "readonly" ? "sidebar.mode_safe_label" : "sidebar.mode_full_label")}
                </button>
              );
            })}
          </div>
        </div>
        <div className="px-3 py-2 text-[11px] font-mono uppercase tracking-wider text-zinc-500 font-semibold">
          {t("sidebar.core_nav")}
        </div>
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = currentTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onSelectTab(item.id)}
              className={`w-full flex items-center justify-between px-3 py-2.5 rounded text-xs font-medium transition-all ${
                isActive
                  ? "bg-zinc-800/90 text-zinc-100 border border-zinc-700/80 shadow-sm"
                  : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900/60 border border-transparent"
              }`}
            >
              <div className="flex items-center gap-2.5">
                <Icon
                  className={`w-4 h-4 ${
                    isActive ? "text-zinc-100" : "text-zinc-400"
                  }`}
                />
                <span>{item.label}</span>
              </div>
              {item.badge && (
                <span
                  className={`text-[10px] font-mono px-1.5 py-0.5 rounded border ${item.badgeColor}`}
                >
                  {item.badge}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Bottom Section */}
      <div>
        {/* Bottom Profile Summary Card */}
        <div className="p-3 m-3 rounded bg-zinc-900/70 border border-zinc-800/80 space-y-2.5">
          <div className="flex items-center justify-between text-[11px] text-zinc-400 font-mono">
            <span>{t("sidebar.tunnel_profile")}</span>
            <span className="text-zinc-500">
              {settings?.profile_name || "chappie"}
            </span>
          </div>

          <div className="space-y-1">
            <div className="text-[10px] text-zinc-500 font-mono">Tunnel ID:</div>
            <div className="text-[11px] font-mono text-zinc-300 truncate bg-zinc-950 px-2 py-1 rounded border border-zinc-800/80">
              {settings?.tunnel_id
                ? settings.tunnel_id
                : t("sidebar.unconfigured_tunnel_id")}
            </div>
          </div>

          <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between text-[11px]">
            <a
              href="https://platform.openai.com/settings/organization/tunnels"
              target="_blank"
              rel="noreferrer"
              className="text-zinc-400 hover:text-zinc-200 flex items-center gap-1 transition-colors"
            >
              <span>{t("sidebar.openai_tunnels_link")}</span>
              <ExternalLink className="w-3 h-3 text-zinc-500" />
            </a>
            <span className="font-mono text-zinc-500 text-[10px]">
              {settings?.health_port
                ? t("sidebar.port_fixed", { port: settings.health_port })
                : t("sidebar.port_auto")}
            </span>
          </div>
        </div>

        {/* App Version Footer */}
        <div className="px-4 py-2.5 border-t border-zinc-800/80 flex items-center justify-between text-[11px] font-mono text-zinc-500">
          <span>{t("sidebar.client_version")}</span>
          <span className="px-1.5 py-0.5 rounded bg-zinc-900 border border-zinc-800 text-zinc-400">
            v{APP_VERSION}
          </span>
        </div>
      </div>
    </aside>
  );
};
