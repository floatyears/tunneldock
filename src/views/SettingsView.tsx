import React, { useState, useEffect } from "react";
import {
  Shield,
  FolderOpen,
  ArrowUpRight,
  Check,
  Save,
  Info,
  Server,
  Tag,
  RefreshCw,
  Download,
  LoaderCircle,
  Sparkles,
} from "lucide-react";
import { TunnelSettings } from "../types";
import { AppUpdateState } from "../hooks/useAppUpdater";
import { openPathInExplorer, saveTunnelCredentials } from "../api";
import { APP_VERSION } from "../version";

interface SettingsViewProps {
  settings: TunnelSettings | null;
  onRefreshSettings: () => void;
  updateState: AppUpdateState;
  onCheckUpdates: () => void;
  onOpenUpdater: () => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  settings,
  onRefreshSettings,
  updateState,
  onCheckUpdates,
  onOpenUpdater,
}) => {
  const [tunnelId, setTunnelId] = useState(settings?.tunnel_id || "");
  const [apiKey, setApiKey] = useState(settings?.api_key || "");
  const [healthPort, setHealthPort] = useState<number>(
    settings?.health_port || 8080
  );
  const [saving, setSaving] = useState(false);
  const [savedSuccess, setSavedSuccess] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (settings) {
      setTunnelId(settings.tunnel_id);
      setApiKey(settings.api_key);
      setHealthPort(settings.health_port);
    }
  }, [settings]);

  const handleSave = async () => {
    if (!tunnelId.trim() || !apiKey.trim()) {
      setErrorMessage("Tunnel ID 和 API Key 均为必填项");
      return;
    }

    try {
      setSaving(true);
      setErrorMessage(null);
      await saveTunnelCredentials(tunnelId.trim(), apiKey.trim(), healthPort);
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2500);
      onRefreshSettings();
    } catch (err: any) {
      setErrorMessage(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="p-6 space-y-6 max-w-4xl mx-auto">
      {/* Top Banner */}
      <div className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg flex items-center justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold text-zinc-100">
              OpenAI Tunnel 凭据与配置文件
            </h2>
            <span className="text-xs font-mono text-zinc-500">
              (Profile: {settings?.profile_name || "chappie"})
            </span>
          </div>
          <p className="text-xs text-zinc-400 max-w-xl">
            配置与 OpenAI Secure MCP Tunnel 相关的组织级隧道凭据与本地监听端口。保存后将自动同步至 ~/.chappie/tunnelkey.txt 与 chappie.yaml。
          </p>
        </div>

        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center gap-2 px-4 py-2 rounded text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-zinc-950 font-semibold shadow transition-all disabled:opacity-50"
        >
          {savedSuccess ? (
            <Check className="w-4 h-4 text-zinc-950" />
          ) : (
            <Save className="w-4 h-4" />
          )}
          <span>{saving ? "正在保存..." : savedSuccess ? "已成功保存" : "保存所有配置"}</span>
        </button>
      </div>

      {errorMessage && (
        <div className="p-3 rounded bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300">
          {errorMessage}
        </div>
      )}

      {/* Main Form Cards */}
      <div className="space-y-4">
        {/* Card 1: Credentials */}
        <div className="p-5 rounded-lg bg-dark-card border border-zinc-800 space-y-4">
          <h3 className="text-xs font-mono font-semibold uppercase tracking-wider text-zinc-300 flex items-center gap-2">
            <Shield className="w-4 h-4 text-emerald-400" />
            <span>OpenAI 平台凭据</span>
          </h3>

          <div className="space-y-4">
            {/* Tunnel ID */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between text-xs">
                <label className="text-zinc-300 font-medium">Tunnel ID</label>
                <a
                  href="https://platform.openai.com/settings/organization/tunnels"
                  target="_blank"
                  rel="noreferrer"
                  className="text-emerald-400 hover:underline flex items-center gap-1 text-[11px]"
                >
                  <span>前往 OpenAI Platform 创建 Tunnel</span>
                  <ArrowUpRight className="w-3 h-3" />
                </a>
              </div>
              <input
                type="text"
                value={tunnelId}
                onChange={(e) => setTunnelId(e.target.value)}
                placeholder="例如: tunnel_6aa8f23637488191acd536bd857791d1"
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
              />
              <p className="text-[11px] text-zinc-500">
                在 OpenAI 组织设置中的 Tunnels 页面创建，名称建议为 Chappie。
              </p>
            </div>

            {/* API Key */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between text-xs">
                <label className="text-zinc-300 font-medium">
                  Tunnel API Key (Restricted)
                </label>
                <a
                  href="https://platform.openai.com/settings/organization/api-keys"
                  target="_blank"
                  rel="noreferrer"
                  className="text-emerald-400 hover:underline flex items-center gap-1 text-[11px]"
                >
                  <span>创建专属 Restricted Key</span>
                  <ArrowUpRight className="w-3 h-3" />
                </a>
              </div>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
              />
              <p className="text-[11px] text-zinc-500">
                权限严格限制为: <span className="text-zinc-300 font-mono">Tunnels: Read, Use</span>。不要配置为全局 Admin 权限。
              </p>
            </div>
          </div>
        </div>

        {/* Card 2: Local Network & Daemon Settings */}
        <div className="p-5 rounded-lg bg-dark-card border border-zinc-800 space-y-4">
          <h3 className="text-xs font-mono font-semibold uppercase tracking-wider text-zinc-300 flex items-center gap-2">
            <Server className="w-4 h-4 text-emerald-400" />
            <span>本地监听与控制面参数</span>
          </h3>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <label className="text-xs text-zinc-300 font-medium">
                健康检查监听端口 (Health Port)
              </label>
              <input
                type="number"
                value={healthPort}
                onChange={(e) => setHealthPort(Number(e.target.value) || 8080)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
              />
              <p className="text-[11px] text-zinc-500">
                默认 8080。本地守护进程将在 127.0.0.1:8080 暴露 /healthz 探针。
              </p>
            </div>

            <div className="space-y-1.5">
              <label className="text-xs text-zinc-300 font-medium">
                OpenAI 控制面 Base URL
              </label>
              <input
                type="text"
                value="https://api.openai.com"
                disabled
                className="w-full bg-zinc-950/60 border border-zinc-800/80 rounded px-3 py-2 text-xs font-mono text-zinc-400 cursor-not-allowed"
              />
              <p className="text-[11px] text-zinc-500">
                OpenAI 官方安全控制面地址。
              </p>
            </div>
          </div>
        </div>

        {/* Card 3: Storage Paths */}
        <div className="p-5 rounded-lg bg-dark-card border border-zinc-800 space-y-3">
          <h3 className="text-xs font-mono font-semibold uppercase tracking-wider text-zinc-300 flex items-center gap-2">
            <FolderOpen className="w-4 h-4 text-amber-400" />
            <span>配置文件物理存储位置</span>
          </h3>

          <div className="space-y-2 text-xs font-mono">
            <div className="p-2.5 rounded bg-zinc-950 border border-zinc-800 flex items-center justify-between">
              <div className="space-y-0.5 truncate pr-2">
                <div className="text-zinc-500 text-[10px]">API Key 密钥文件:</div>
                <div className="text-zinc-300 truncate">
                  {settings?.key_file_path || "C:\\Users\\...\\.chappie\\tunnelkey.txt"}
                </div>
              </div>
              {settings?.key_file_path && (
                <button
                  onClick={() => openPathInExplorer(settings.key_file_path)}
                  className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-700 flex items-center gap-1 shrink-0"
                >
                  <FolderOpen className="w-3 h-3" />
                  <span>定位</span>
                </button>
              )}
            </div>

            <div className="p-2.5 rounded bg-zinc-950 border border-zinc-800 flex items-center justify-between">
              <div className="space-y-0.5 truncate pr-2">
                <div className="text-zinc-500 text-[10px]">Otunnel Profile 描述文件:</div>
                <div className="text-zinc-300 truncate">
                  %APPDATA%\tunnel-client\chappie.yaml
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Card 4: Best Practices Guide */}
        <div className="p-5 rounded-lg bg-dark-card border border-zinc-800 space-y-3">
          <div className="flex items-center gap-2 text-xs font-semibold text-zinc-200">
            <Info className="w-4 h-4 text-emerald-400" />
            <span>官方最佳实践安全原则</span>
          </div>

          <ul className="text-xs text-zinc-400 space-y-1.5 list-disc list-inside leading-relaxed">
            <li>
              <strong>不要以管理员 (Administrator) 身份运行</strong>：Pi 拥有当前系统用户的文件与命令执行权限。
            </li>
            <li>
              <strong>精准 Session 绑定</strong>：在 ChatGPT 中始终使用 <code className="text-zinc-300 font-mono">sessions → cwd → sessionId → init</code> 绑定具体工程，切勿跨项目串线。
            </li>
            <li>
              <strong>耗时任务后台化</strong>：编译大型工程、打包 Docker 镜像等超过 30 秒的命令，建议使用后台任务执行，避免 MCP 请求超时。
            </li>
          </ul>
        </div>

        {/* Card 5: Version & In-app Update Center */}
        <div className="p-5 rounded-lg bg-dark-card border border-zinc-800 space-y-4">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-xs font-mono font-semibold uppercase tracking-wider text-zinc-300 flex items-center gap-2">
                <Tag className="w-4 h-4 text-emerald-400" />
                <span>应用版本与软件更新</span>
              </h3>
              <p className="text-xs text-zinc-500 leading-relaxed">
                启动后自动检查 <span className="text-zinc-300 font-mono">t59688/hola</span> 的 GitHub Release。更新包在应用内部下载，并在安装前验证 Tauri 更新签名。
              </p>
            </div>

            <button
              type="button"
              onClick={
                updateState.stage === "available" ||
                updateState.stage === "downloading" ||
                updateState.stage === "installing"
                  ? onOpenUpdater
                  : onCheckUpdates
              }
              disabled={updateState.stage === "checking"}
              className={`shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium border transition-colors disabled:opacity-50 ${
                updateState.stage === "available"
                  ? "bg-emerald-950/50 text-emerald-300 border-emerald-800/60 hover:bg-emerald-900/50"
                  : "bg-zinc-900 text-zinc-300 border-zinc-700 hover:bg-zinc-800"
              }`}
            >
              {updateState.stage === "checking" ? (
                <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
              ) : updateState.stage === "available" ? (
                <Sparkles className="w-3.5 h-3.5" />
              ) : updateState.stage === "downloading" ? (
                <Download className="w-3.5 h-3.5" />
              ) : (
                <RefreshCw className="w-3.5 h-3.5" />
              )}
              <span>
                {updateState.stage === "checking"
                  ? "正在检查"
                  : updateState.stage === "available"
                  ? `查看 v${updateState.latestVersion}`
                  : updateState.stage === "downloading"
                  ? `下载 ${updateState.totalBytes ? `${updateState.progressPercent}%` : "中"}`
                  : updateState.stage === "installing"
                  ? "正在安装"
                  : "检查更新"}
              </span>
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-[11px] font-mono">
            <div className="p-3 rounded bg-zinc-950 border border-zinc-800 space-y-1">
              <div className="text-zinc-600">当前版本</div>
              <div className="text-emerald-400 font-medium">v{APP_VERSION}</div>
            </div>
            <div className="p-3 rounded bg-zinc-950 border border-zinc-800 space-y-1">
              <div className="text-zinc-600">更新状态</div>
              <div className={updateState.stage === "error" ? "text-rose-400" : updateState.stage === "available" ? "text-amber-300" : "text-zinc-300"}>
                {updateState.stage === "available"
                  ? `发现 v${updateState.latestVersion}`
                  : updateState.stage === "downloading"
                  ? "后台下载中"
                  : updateState.stage === "installing"
                  ? "安装中"
                  : updateState.stage === "installed"
                  ? "已安装，重启后生效"
                  : updateState.stage === "error"
                  ? "检查/更新失败"
                  : updateState.stage === "checking"
                  ? "检查中"
                  : "已启用自动检查"}
              </div>
            </div>
            <div className="p-3 rounded bg-zinc-950 border border-zinc-800 space-y-1">
              <div className="text-zinc-600">Release 源</div>
              <div className="text-zinc-300 truncate">github.com/t59688/hola</div>
            </div>
          </div>

          <div className="text-[11px] text-zinc-600 leading-relaxed">
            版本号仍以根目录 <code className="text-zinc-400 font-mono">version.json</code> 为单一真实来源；Release 工作流会校验 Git tag 与版本一致后生成三端更新资产、签名和 <code className="text-zinc-400 font-mono">latest.json</code>。
          </div>
        </div>
      </div>
    </div>
  );
};
