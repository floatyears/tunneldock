import React, { useState } from "react";
import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Download,
  RefreshCw,
  FolderOpen,
  ArrowUpRight,
  Sliders,
  Terminal as TerminalIcon,
} from "lucide-react";
import { EnvCheckItem, TunnelSettings } from "../types";
import { installComponent, openPathInExplorer, saveTunnelCredentials } from "../api";

interface EnvironmentViewProps {
  items: EnvCheckItem[];
  loading: boolean;
  onRefresh: () => void;
  onOpenTerminal: () => void;
  settings: TunnelSettings | null;
  onSaveSettings: () => void;
}

export const EnvironmentView: React.FC<EnvironmentViewProps> = ({
  items,
  loading,
  onRefresh,
  onOpenTerminal,
  settings,
  onSaveSettings,
}) => {
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [isAutoInstalling, setIsAutoInstalling] = useState(false);
  const [showConfigModal, setShowConfigModal] = useState(false);
  const [inputTunnelId, setInputTunnelId] = useState(settings?.tunnel_id || "");
  const [inputApiKey, setInputApiKey] = useState(settings?.api_key || "");
  const [configSaving, setConfigSaving] = useState(false);
  const [configError, setConfigError] = useState<string | null>(null);

  const readyCount = items.filter((i) => i.status === "ready").length;
  const missingItems = items.filter(
    (i) => i.status === "missing" || i.status === "outdated"
  );
  const configNeededItems = items.filter((i) => i.status === "config_needed");
  const isAllReady = readyCount === items.length && items.length > 0;

  const handleInstallOne = async (id: string) => {
    try {
      setInstallingId(id);
      onOpenTerminal();
      await installComponent(id);
      await onRefresh();
    } catch (e) {
      console.error(e);
    } finally {
      setInstallingId(null);
    }
  };

  const handleAutoInstallAll = async () => {
    setIsAutoInstalling(true);
    onOpenTerminal();
    for (const item of missingItems) {
      if (item.can_auto_install) {
        setInstallingId(item.id);
        try {
          await installComponent(item.id);
        } catch (e) {
          console.error(e);
        }
      }
    }
    setInstallingId(null);
    setIsAutoInstalling(false);
    await onRefresh();
  };

  const handleSaveCredentials = async () => {
    if (!inputTunnelId.trim() || !inputApiKey.trim()) {
      setConfigError("Tunnel ID 和 API Key 均不能为空");
      return;
    }
    try {
      setConfigSaving(true);
      setConfigError(null);
      await saveTunnelCredentials(inputTunnelId.trim(), inputApiKey.trim());
      setShowConfigModal(false);
      onSaveSettings();
      await onRefresh();
    } catch (err: any) {
      setConfigError(String(err));
    } finally {
      setConfigSaving(false);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case "ready":
        return (
          <span className="flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded bg-emerald-950/40 text-emerald-400 border border-emerald-800/60 shrink-0 whitespace-nowrap">
            <CheckCircle2 className="w-3 h-3 text-emerald-400 shrink-0" />
            已就绪
          </span>
        );
      case "outdated":
        return (
          <span className="flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded bg-amber-950/40 text-amber-400 border border-amber-800/60 shrink-0 whitespace-nowrap">
            <AlertTriangle className="w-3 h-3 text-amber-400 shrink-0" />
            版本过低
          </span>
        );
      case "warning":
        return (
          <span className="flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded bg-amber-950/40 text-amber-400 border border-amber-800/60 shrink-0 whitespace-nowrap">
            <AlertTriangle className="w-3 h-3 text-amber-400 shrink-0" />
            注意
          </span>
        );
      case "config_needed":
        return (
          <span className="flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded bg-amber-950/40 text-amber-400 border border-amber-800/60 shrink-0 whitespace-nowrap">
            <Sliders className="w-3 h-3 text-amber-400 shrink-0" />
            待配置
          </span>
        );
      default:
        return (
          <span className="flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded bg-rose-950/40 text-rose-400 border border-rose-800/60 shrink-0 whitespace-nowrap">
            <XCircle className="w-3 h-3 text-rose-400 shrink-0" />
            未安装
          </span>
        );
    }
  };

  const categories = [
    { key: "runtime", title: "核心运行时 (Runtime)" },
    { key: "tools", title: "开发与构建工具 (Tools)" },
    { key: "mcp", title: "OpenAI MCP & Pi 体系" },
    { key: "credentials", title: "隧道凭据与 Profile" },
  ];

  return (
    <div className="p-6 space-y-6 max-w-6xl mx-auto">
      {/* Top Banner */}
      <div className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg flex items-center justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold text-zinc-100">
              系统环境全链路自检
            </h2>
            <span className="text-xs font-mono text-zinc-500">
              ({readyCount}/{items.length} 就绪)
            </span>
          </div>
          <p className="text-xs text-zinc-400 max-w-xl">
            {isAllReady
              ? "本地开发链路环境完全就绪，无需手动敲击任何命令，可直接启动 Tunnel 与开启工作区。"
              : `检测到 ${
                  missingItems.length + configNeededItems.length
                } 项需处理。点击下方“一键全自动装配”或单独组件一键安装，即可实现全自动化配置。`}
          </p>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={onOpenTerminal}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors"
          >
            <TerminalIcon className="w-3.5 h-3.5" />
            <span>查看安装输出</span>
          </button>

          <button
            onClick={onRefresh}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors disabled:opacity-50"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${loading ? "animate-spin" : ""}`}
            />
            <span>重新自检</span>
          </button>

          {missingItems.length > 0 && (
            <button
              onClick={handleAutoInstallAll}
              disabled={isAutoInstalling}
              className="flex items-center gap-2 px-4 py-2 rounded text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-zinc-950 font-semibold shadow transition-all disabled:opacity-50"
            >
              <Download className="w-4 h-4" />
              <span>
                {isAutoInstalling ? "正在自动装配中..." : "一键全自动装配"}
              </span>
            </button>
          )}

          {configNeededItems.length > 0 && missingItems.length === 0 && (
            <button
              onClick={() => setShowConfigModal(true)}
              className="flex items-center gap-2 px-4 py-2 rounded text-xs font-medium bg-amber-500 hover:bg-amber-400 text-zinc-950 font-semibold shadow transition-all"
            >
              <Sliders className="w-4 h-4" />
              <span>快速配置凭据</span>
            </button>
          )}
        </div>
      </div>

      {/* Categories Grouping */}
      <div className="space-y-6">
        {categories.map((cat) => {
          const catItems = items.filter((i) => i.category === cat.key);
          if (catItems.length === 0) return null;

          return (
            <div key={cat.key} className="space-y-3">
              <h3 className="text-xs font-mono font-semibold uppercase tracking-wider text-zinc-400 flex items-center gap-2">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                {cat.title}
              </h3>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {catItems.map((item) => {
                  const isCurInstalling = installingId === item.id;

                  return (
                    <div
                      key={item.id}
                      className="p-4 rounded-lg bg-dark-card/90 border border-zinc-800 hover:border-zinc-700/80 transition-all flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-2">
                        {/* Title and Badge */}
                        <div className="flex items-start justify-between gap-3">
                          <div className="space-y-0.5 min-w-0 flex-1">
                            <div className="text-sm font-semibold text-zinc-100 flex items-center gap-2 flex-wrap">
                              <span className="truncate">{item.name}</span>
                              {item.version && (
                                <span
                                  className="font-mono text-xs text-zinc-400 truncate max-w-[180px] sm:max-w-[220px]"
                                  title={item.version}
                                >
                                  ({item.version})
                                </span>
                              )}
                            </div>
                            {item.required_version && (
                              <div className="text-[11px] font-mono text-zinc-500">
                                要求版本: {item.required_version}
                              </div>
                            )}
                          </div>
                          {getStatusBadge(item.status)}
                        </div>

                        {/* Description Message */}
                        <p className="text-xs text-zinc-400 leading-relaxed">
                          {item.message}
                        </p>

                        {/* Path Info */}
                        {item.path && (
                          <div className="flex items-center justify-between text-[11px] font-mono text-zinc-500 bg-zinc-950/70 px-2 py-1 rounded border border-zinc-800/80">
                            <span className="truncate pr-2">{item.path}</span>
                            <button
                              onClick={() => openPathInExplorer(item.path!)}
                              className="text-zinc-400 hover:text-zinc-200 shrink-0"
                              title="在资源管理器中查看"
                            >
                              <FolderOpen className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        )}
                      </div>

                      {/* Action Row */}
                      <div className="pt-2 border-t border-zinc-800/60 flex items-center justify-between">
                        <span className="text-[11px] font-mono text-zinc-500">
                          ID: {item.id}
                        </span>

                        <div className="flex items-center gap-2">
                          {item.status !== "ready" && item.can_auto_install && (
                            <button
                              onClick={() => handleInstallOne(item.id)}
                              disabled={isCurInstalling}
                              className="px-2.5 py-1 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                            >
                              <Download className="w-3 h-3 text-emerald-400" />
                              <span>
                                {isCurInstalling
                                  ? "安装中..."
                                  : item.status === "outdated"
                                  ? "一键升级"
                                  : "一键安装"}
                              </span>
                            </button>
                          )}

                          {item.category === "credentials" && (
                            <button
                              onClick={() => {
                                setInputTunnelId(settings?.tunnel_id || "");
                                setInputApiKey(settings?.api_key || "");
                                setShowConfigModal(true);
                              }}
                              className="px-2.5 py-1 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors flex items-center gap-1.5"
                            >
                              <Sliders className="w-3 h-3 text-amber-400" />
                              <span>配置凭据</span>
                            </button>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>

      {/* Configuration Modal */}
      {showConfigModal && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-dark-card border border-zinc-800 rounded-lg max-w-md w-full p-6 space-y-5 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="space-y-1">
              <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
                <Sliders className="w-4 h-4 text-amber-400" />
                配置 OpenAI Tunnel 凭据
              </h3>
              <p className="text-xs text-zinc-400">
                系统将自动生成密钥文件与 otunnel profile，无需手动输入终端命令。
              </p>
            </div>

            {configError && (
              <div className="p-3 rounded bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300">
                {configError}
              </div>
            )}

            <div className="space-y-4">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <label className="text-zinc-300 font-medium">Tunnel ID</label>
                  <a
                    href="https://platform.openai.com/settings/organization/tunnels"
                    target="_blank"
                    rel="noreferrer"
                    className="text-emerald-400 hover:underline flex items-center gap-1 text-[11px]"
                  >
                    <span>在平台创建 Tunnel</span>
                    <ArrowUpRight className="w-3 h-3" />
                  </a>
                </div>
                <input
                  type="text"
                  placeholder="例如: tunnel_6aa8f23637488191acd536bd857791d1"
                  value={inputTunnelId}
                  onChange={(e) => setInputTunnelId(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
                />
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <label className="text-zinc-300 font-medium">
                    OpenAI API Key (Restricted)
                  </label>
                  <a
                    href="https://platform.openai.com/settings/organization/api-keys"
                    target="_blank"
                    rel="noreferrer"
                    className="text-emerald-400 hover:underline flex items-center gap-1 text-[11px]"
                  >
                    <span>创建 Tunnel 密钥</span>
                    <ArrowUpRight className="w-3 h-3" />
                  </a>
                </div>
                <input
                  type="password"
                  placeholder="sk-..."
                  value={inputApiKey}
                  onChange={(e) => setInputApiKey(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
                />
                <p className="text-[11px] text-zinc-500">
                  权限建议只赋予 Tunnels: Read / Use，密钥将安全存放于 ~/.chappie/tunnelkey.txt。
                </p>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => setShowConfigModal(false)}
                className="px-3 py-1.5 rounded text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800"
              >
                取消
              </button>
              <button
                onClick={handleSaveCredentials}
                disabled={configSaving}
                className="px-4 py-1.5 rounded text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-zinc-950 font-semibold disabled:opacity-50"
              >
                {configSaving ? "正在保存..." : "保存并初始化 Profile"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
