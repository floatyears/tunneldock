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
  Trash2,
} from "lucide-react";
import { EnvCheckItem, TunnelSettings } from "../types";
import {
  installComponent,
  uninstallComponent,
  openPathInExplorer,
  saveTunnelCredentials,
} from "../api";

interface EnvironmentViewProps {
  items: EnvCheckItem[];
  loading: boolean;
  onRefresh: () => void | Promise<void>;
  onOpenTerminal: () => void;
  settings: TunnelSettings | null;
  onSaveSettings: () => void | Promise<void>;
}

function formatError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error);
  } catch {
    return "发生未知错误";
  }
}

function uninstallImpact(item: EnvCheckItem): string {
  switch (item.id) {
    case "node":
    case "npm":
      return "Node.js 与 npm 属于同一运行时。卸载后 Pi、Chappie 及依赖 Node.js 的工作区将无法启动；TunnelDock 会先停止其管理的 Pi 工作区进程。";
    case "git":
      return "卸载后本机将无法通过该 Git 安装执行版本控制操作。系统自带或非 TunnelDock 可安全识别的 Git 不会被强制删除。";
    case "cargo":
      return "卸载 Rust/Cargo 可能同时移除同一 Cargo Home 中的 cargo-binstall、otunnel 等 Cargo 二进制。TunnelDock 会先停止其管理的 otunnel 进程。";
    case "cargo_binstall":
      return "仅移除 cargo-binstall 安装器；已经安装的 otunnel 不会因此被主动删除。";
    case "otunnel":
      return "TunnelDock 会先停止其管理的 otunnel 进程，然后移除 otunnel。Tunnel Key 与 Profile 配置会保留。";
    case "pi":
      return "TunnelDock 会先停止全部由其管理的 Pi 工作区进程。Chappie 扩展文件可能仍保留，但在 Pi 重新安装前无法运行。";
    case "chappie":
      return "TunnelDock 会先停止由其管理的 Pi 工作区进程，然后仅移除 @zetaloop/chappie 扩展，不卸载 Pi。";
    case "tunnel_key":
      return "将删除本机 ~/.chappie/tunnelkey.txt 中的 Tunnel API Key，并同步清空 TunnelDock 保存的密钥。远端 OpenAI API Key 本身不会被撤销。";
    case "tunnel_config":
      return "将删除 TunnelDock/otunnel 同步维护的所有 chappie.yaml 副本，并清空本地 Tunnel ID。Tunnel API Key 文件会保留。";
    default:
      return "将移除该组件，并在操作后重新检测实际系统状态。";
  }
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
  const [uninstallingId, setUninstallingId] = useState<string | null>(null);
  const [pendingUninstall, setPendingUninstall] = useState<EnvCheckItem | null>(null);
  const [isAutoInstalling, setIsAutoInstalling] = useState(false);
  const [showConfigModal, setShowConfigModal] = useState(false);
  const [inputTunnelId, setInputTunnelId] = useState(settings?.tunnel_id || "");
  const [inputApiKey, setInputApiKey] = useState(settings?.api_key || "");
  const [configSaving, setConfigSaving] = useState(false);
  const [configError, setConfigError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const readyCount = items.filter((i) => i.status === "ready").length;
  const missingItems = items.filter(
    (i) => i.status === "missing" || i.status === "outdated"
  );
  const configNeededItems = items.filter((i) => i.status === "config_needed");
  const isAllReady = readyCount === items.length && items.length > 0;
  const operationBusy =
    isAutoInstalling || installingId !== null || uninstallingId !== null;

  const handleInstallOne = async (id: string) => {
    if (operationBusy) return;
    try {
      setActionError(null);
      setInstallingId(id);
      onOpenTerminal();
      await installComponent(id);
      await onRefresh();
    } catch (error) {
      setActionError(`安装失败：${formatError(error)}`);
      console.error(error);
    } finally {
      setInstallingId(null);
    }
  };

  const handleAutoInstallAll = async () => {
    if (operationBusy) return;
    setActionError(null);
    setIsAutoInstalling(true);
    onOpenTerminal();
    const failures: string[] = [];

    for (const item of missingItems) {
      if (!item.can_auto_install) continue;
      setInstallingId(item.id);
      try {
        await installComponent(item.id);
      } catch (error) {
        failures.push(`${item.name}: ${formatError(error)}`);
        console.error(error);
      }
    }

    setInstallingId(null);
    setIsAutoInstalling(false);
    await onRefresh();
    if (failures.length > 0) {
      setActionError(`部分组件自动装配失败：${failures.join("；")}`);
    }
  };

  const handleConfirmUninstall = async () => {
    if (!pendingUninstall || operationBusy) return;
    const item = pendingUninstall;

    try {
      setActionError(null);
      setUninstallingId(item.id);
      onOpenTerminal();
      await uninstallComponent(item.id);
      setPendingUninstall(null);
      await onRefresh();
      await onSaveSettings();
    } catch (error) {
      setActionError(`卸载失败：${formatError(error)}`);
      console.error(error);
    } finally {
      setUninstallingId(null);
    }
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
      await onSaveSettings();
      await onRefresh();
    } catch (error) {
      setConfigError(formatError(error));
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

  const destructiveHighImpact = pendingUninstall
    ? ["node", "npm", "cargo", "pi"].includes(pendingUninstall.id)
    : false;

  return (
    <div className="p-6 space-y-6 max-w-6xl mx-auto">
      {/* Top Banner */}
      <div className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg flex items-center justify-between gap-4">
        <div className="space-y-1 min-w-0">
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
              ? "本地开发链路环境完全就绪。每个已安装组件均可独立执行受控卸载或配置清理。"
              : `检测到 ${
                  missingItems.length + configNeededItems.length
                } 项需处理。可自动装配缺失组件，也可对已安装组件执行受控卸载。`}
          </p>
        </div>

        <div className="flex items-center gap-3 shrink-0">
          <button
            onClick={onOpenTerminal}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors"
          >
            <TerminalIcon className="w-3.5 h-3.5" />
            <span>查看操作输出</span>
          </button>

          <button
            onClick={() => void onRefresh()}
            disabled={loading || operationBusy}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors disabled:opacity-50"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${loading ? "animate-spin" : ""}`}
            />
            <span>重新自检</span>
          </button>

          {missingItems.length > 0 && (
            <button
              onClick={() => void handleAutoInstallAll()}
              disabled={operationBusy}
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
              disabled={operationBusy}
              className="flex items-center gap-2 px-4 py-2 rounded text-xs font-medium bg-amber-500 hover:bg-amber-400 text-zinc-950 font-semibold shadow transition-all disabled:opacity-50"
            >
              <Sliders className="w-4 h-4" />
              <span>快速配置凭据</span>
            </button>
          )}
        </div>
      </div>

      {actionError && (
        <div className="p-3 rounded-lg bg-rose-950/30 border border-rose-800/60 text-xs text-rose-300 flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0">
            <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
            <span className="leading-relaxed break-words">{actionError}</span>
          </div>
          <button
            onClick={() => setActionError(null)}
            className="text-rose-300/70 hover:text-rose-200 shrink-0"
            aria-label="关闭错误提示"
          >
            <XCircle className="w-4 h-4" />
          </button>
        </div>
      )}

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
                  const isCurUninstalling = uninstallingId === item.id;

                  return (
                    <div
                      key={item.id}
                      className="p-4 rounded-lg bg-dark-card/90 border border-zinc-800 hover:border-zinc-700/80 transition-all flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-2">
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

                        <p className="text-xs text-zinc-400 leading-relaxed">
                          {item.message}
                        </p>

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

                      <div className="pt-2 border-t border-zinc-800/60 flex items-center justify-between gap-3">
                        <span className="text-[11px] font-mono text-zinc-500 truncate">
                          ID: {item.id}
                        </span>

                        <div className="flex items-center gap-2 shrink-0">
                          {item.status !== "ready" && item.can_auto_install && (
                            <button
                              onClick={() => void handleInstallOne(item.id)}
                              disabled={operationBusy}
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
                              disabled={operationBusy}
                              className="px-2.5 py-1 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                            >
                              <Sliders className="w-3 h-3 text-amber-400" />
                              <span>配置凭据</span>
                            </button>
                          )}

                          {item.installed && (
                            <button
                              onClick={() => {
                                setActionError(null);
                                setPendingUninstall(item);
                              }}
                              disabled={operationBusy}
                              className="px-2.5 py-1 rounded text-xs font-medium bg-rose-950/30 hover:bg-rose-950/60 text-rose-300 border border-rose-900/70 hover:border-rose-800 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                              title={item.category === "credentials" ? "清除本地配置" : "卸载组件"}
                            >
                              <Trash2 className="w-3 h-3" />
                              <span>
                                {isCurUninstalling
                                  ? "处理中..."
                                  : item.category === "credentials"
                                  ? "清除"
                                  : "卸载"}
                              </span>
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

      {/* Uninstall confirmation modal */}
      {pendingUninstall && (
        <div className="fixed inset-0 bg-black/75 backdrop-blur-sm flex items-center justify-center z-[60] p-4">
          <div className="bg-dark-card border border-rose-900/60 rounded-lg max-w-lg w-full p-6 space-y-5 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="space-y-2">
              <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 text-rose-400" />
                {pendingUninstall.category === "credentials" ? "确认清除" : "确认卸载"}
                <span className="text-rose-300">{pendingUninstall.name}</span>
              </h3>
              <p className="text-xs text-zinc-400 leading-relaxed">
                {uninstallImpact(pendingUninstall)}
              </p>
            </div>

            {destructiveHighImpact && (
              <div className="p-3 rounded bg-amber-950/30 border border-amber-800/60 text-xs text-amber-300 leading-relaxed">
                这是高影响操作。相关运行中的 TunnelDock 子进程会先被安全停止，操作完成后系统会自动重新自检。外部程序或终端中自行启动的进程不由 TunnelDock 强制管理。
              </div>
            )}

            <div className="p-3 rounded bg-zinc-950/70 border border-zinc-800 text-[11px] font-mono text-zinc-400">
              组件 ID: {pendingUninstall.id}
              {pendingUninstall.path && (
                <div className="mt-1 break-all">当前路径: {pendingUninstall.path}</div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-1">
              <button
                onClick={() => setPendingUninstall(null)}
                disabled={uninstallingId !== null}
                className="px-3 py-1.5 rounded text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 disabled:opacity-50"
              >
                取消
              </button>
              <button
                onClick={() => void handleConfirmUninstall()}
                disabled={uninstallingId !== null}
                className="px-4 py-1.5 rounded text-xs font-semibold bg-rose-600 hover:bg-rose-500 text-white disabled:opacity-50 flex items-center gap-1.5"
              >
                <Trash2 className="w-3.5 h-3.5" />
                {uninstallingId === pendingUninstall.id
                  ? "正在处理..."
                  : pendingUninstall.category === "credentials"
                  ? "确认清除"
                  : "确认卸载"}
              </button>
            </div>
          </div>
        </div>
      )}

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
                disabled={configSaving}
                className="px-3 py-1.5 rounded text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 disabled:opacity-50"
              >
                取消
              </button>
              <button
                onClick={() => void handleSaveCredentials()}
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
