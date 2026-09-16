import React, { useState } from "react";
import {
  Layers,
  Plus,
  Play,
  Square,
  RotateCcw,
  GitBranch,
  FolderOpen,
  Copy,
  Check,
  Terminal,
  Trash2,
  Sparkles,
} from "lucide-react";
import { WorkspaceItem } from "../types";
import {
  addWorkspace,
  removeWorkspace,
  startWorkspaceSession,
  stopWorkspaceSession,
  restartWorkspaceSession,
  generateChatGptPrompt,
  openPathInExplorer,
} from "../api";

interface WorkspaceViewProps {
  workspaces: WorkspaceItem[];
  loading: boolean;
  onRefresh: () => void;
  onOpenTerminalForWorkspace: (workspaceId: string, title: string) => void;
}

export const WorkspaceView: React.FC<WorkspaceViewProps> = ({
  workspaces,
  loading: _loading,
  onRefresh,
  onOpenTerminalForWorkspace,
}) => {
  const [showAddModal, setShowAddModal] = useState(false);
  const [newPath, setNewPath] = useState("");
  const [newName, setNewName] = useState("");
  const [addingError, setAddingError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // ChatGPT prompt modal state
  const [promptModalWs, setPromptModalWs] = useState<WorkspaceItem | null>(null);
  const [promptText, setPromptText] = useState("");
  const [copiedPrompt, setCopiedPrompt] = useState(false);

  // Loading state per workspace action
  const [actionLoadingId, setActionLoadingId] = useState<string | null>(null);

  const handleAddWorkspace = async () => {
    if (!newPath.trim()) {
      setAddingError("项目路径不能为空");
      return;
    }
    try {
      setIsSubmitting(true);
      setAddingError(null);
      await addWorkspace(newPath.trim(), newName.trim() || undefined);
      setNewPath("");
      setNewName("");
      setShowAddModal(false);
      await onRefresh();
    } catch (err: any) {
      setAddingError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleStart = async (id: string, name: string) => {
    try {
      setActionLoadingId(id);
      await startWorkspaceSession(id);
      onOpenTerminalForWorkspace(id, `工作区: ${name} (Pi Session)`);
      await onRefresh();
    } catch (err) {
      console.error(err);
    } finally {
      setActionLoadingId(null);
    }
  };

  const handleStop = async (id: string) => {
    try {
      setActionLoadingId(id);
      await stopWorkspaceSession(id);
      await onRefresh();
    } catch (err) {
      console.error(err);
    } finally {
      setActionLoadingId(null);
    }
  };

  const handleRestart = async (id: string, name: string) => {
    try {
      setActionLoadingId(id);
      await restartWorkspaceSession(id);
      onOpenTerminalForWorkspace(id, `工作区: ${name} (Pi Session)`);
      await onRefresh();
    } catch (err) {
      console.error(err);
    } finally {
      setActionLoadingId(null);
    }
  };

  const handleRemove = async (id: string) => {
    if (confirm("确定要移除该工作区吗？若正在运行将自动终止进程。")) {
      try {
        await removeWorkspace(id);
        await onRefresh();
      } catch (err) {
        console.error(err);
      }
    }
  };

  const handleOpenPromptModal = async (ws: WorkspaceItem) => {
    try {
      const p = await generateChatGptPrompt(ws.path, ws.session_id);
      setPromptText(p);
      setPromptModalWs(ws);
    } catch (err) {
      console.error(err);
    }
  };

  const handleCopyPrompt = () => {
    navigator.clipboard.writeText(promptText);
    setCopiedPrompt(true);
    setTimeout(() => setCopiedPrompt(false), 2000);
  };

  return (
    <div className="p-6 space-y-6 max-w-6xl mx-auto">
      {/* Header Banner */}
      <div className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg flex items-center justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold text-zinc-100">
              本地项目工作区管理
            </h2>
            <span className="text-xs font-mono text-zinc-500">
              ({workspaces.length} 个项目)
            </span>
          </div>
          <p className="text-xs text-zinc-400 max-w-2xl">
            每个项目对应一个专属的 Pi Session 进程。通过 Chappie MCP Broker，ChatGPT 网页版可根据项目路径或 Session ID 精确绑定并进行代码读写与执行，彻底隔绝项目上下文。
          </p>
        </div>

        <button
          onClick={() => setShowAddModal(true)}
          className="flex items-center gap-2 px-4 py-2 rounded text-xs font-medium bg-zinc-100 hover:bg-white text-zinc-950 font-semibold shadow transition-all"
        >
          <Plus className="w-4 h-4" />
          <span>添加项目工作区</span>
        </button>
      </div>

      {/* Workspaces Grid */}
      {workspaces.length === 0 ? (
        <div className="p-12 text-center rounded-lg bg-dark-card/50 border border-zinc-800/80 space-y-4">
          <div className="w-12 h-12 rounded-full bg-zinc-900 border border-zinc-800 flex items-center justify-center mx-auto text-zinc-400">
            <Layers className="w-6 h-6" />
          </div>
          <div className="space-y-1">
            <h3 className="text-sm font-medium text-zinc-200">
              暂未添加任何本地工作区
            </h3>
            <p className="text-xs text-zinc-500 max-w-md mx-auto">
              点击上方“添加项目工作区”按钮，输入本地代码工程目录（如 D:\work\NewsNook），即可一键拉起 Pi 开发会话。
            </p>
          </div>
          <button
            onClick={() => setShowAddModal(true)}
            className="px-4 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors inline-flex items-center gap-1.5"
          >
            <Plus className="w-4 h-4" />
            <span>立即添加首个项目</span>
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {workspaces.map((ws) => {
            const isRunning = ws.status === "ready" || ws.status === "executing";
            const isLoading = actionLoadingId === ws.id;

            return (
              <div
                key={ws.id}
                className={`p-5 rounded-lg bg-dark-card border transition-all flex flex-col justify-between space-y-4 ${
                  isRunning
                    ? "border-emerald-800/50 shadow-[0_0_15px_rgba(16,185,129,0.06)]"
                    : "border-zinc-800 hover:border-zinc-700/80"
                }`}
              >
                <div className="space-y-3">
                  {/* Top Bar: Title & Status */}
                  <div className="flex items-start justify-between">
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <h3 className="text-sm font-semibold text-zinc-100">
                          {ws.name}
                        </h3>
                        {isRunning && (
                          <span className="flex items-center gap-1 text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-950/60 text-emerald-400 border border-emerald-800/60 shrink-0 whitespace-nowrap">
                            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-ping" />
                            Session 在线
                          </span>
                        )}
                        {!isRunning && (
                          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-zinc-900 text-zinc-500 border border-zinc-800 shrink-0 whitespace-nowrap">
                            未启动
                          </span>
                        )}
                      </div>

                      {/* Path */}
                      <div className="flex items-center gap-1.5 text-xs text-zinc-400 font-mono">
                        <span className="truncate max-w-[280px]">{ws.path}</span>
                        <button
                          onClick={() => openPathInExplorer(ws.path)}
                          className="text-zinc-500 hover:text-zinc-300 transition-colors"
                          title="在文件夹中打开"
                        >
                          <FolderOpen className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <button
                      onClick={() => handleRemove(ws.id)}
                      className="p-1 rounded text-zinc-500 hover:text-rose-400 hover:bg-zinc-900 transition-colors"
                      title="移除工作区"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>

                  {/* Metadata Row: Git & Session Details */}
                  <div className="grid grid-cols-2 gap-2 text-[11px] font-mono">
                    <div className="p-2 rounded bg-zinc-950/70 border border-zinc-800/80 space-y-0.5">
                      <div className="text-zinc-500 flex items-center gap-1">
                        <GitBranch className="w-3 h-3 text-zinc-400" />
                        <span>Git 状态</span>
                      </div>
                      <div className="text-zinc-300 font-medium truncate">
                        {ws.git_branch ? ws.git_branch : "无 Git 仓库"}
                      </div>
                      {ws.git_status && (
                        <div className="text-[10px] text-zinc-400 truncate">
                          {ws.git_status}
                        </div>
                      )}
                    </div>

                    <div className="p-2 rounded bg-zinc-950/70 border border-zinc-800/80 space-y-0.5">
                      <div className="text-zinc-500 flex items-center justify-between">
                        <span>Session ID</span>
                        {ws.pid && (
                          <span className="text-[10px] text-zinc-500">
                            PID: {ws.pid}
                          </span>
                        )}
                      </div>
                      <div className="text-zinc-300 font-medium truncate">
                        {ws.session_id ? ws.session_id : isRunning ? "等待探测..." : "未激活"}
                      </div>
                      <div className="text-[10px] text-zinc-500">
                        绑定的 ChatGPT: {ws.binding_count}
                      </div>
                    </div>
                  </div>
                </div>

                {/* Action Buttons Row */}
                <div className="pt-3 border-t border-zinc-800/80 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    {!isRunning ? (
                      <button
                        onClick={() => handleStart(ws.id, ws.name)}
                        disabled={isLoading}
                        className="px-3 py-1.5 rounded text-xs font-medium bg-emerald-950/40 hover:bg-emerald-900/40 text-emerald-300 border border-emerald-800/60 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                      >
                        <Play className="w-3 h-3 text-emerald-400" />
                        <span>启动 Pi Session</span>
                      </button>
                    ) : (
                      <div className="flex items-center gap-1.5">
                        <button
                          onClick={() => handleStop(ws.id)}
                          disabled={isLoading}
                          className="px-2.5 py-1.5 rounded text-xs font-medium bg-rose-950/40 hover:bg-rose-900/40 text-rose-300 border border-rose-800/60 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                        >
                          <Square className="w-3 h-3 text-rose-400" />
                          <span>停止</span>
                        </button>
                        <button
                          onClick={() => handleRestart(ws.id, ws.name)}
                          disabled={isLoading}
                          className="p-1.5 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border border-zinc-700 transition-colors disabled:opacity-50"
                          title="重启 Session"
                        >
                          <RotateCcw className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    )}

                    <button
                      onClick={() =>
                        onOpenTerminalForWorkspace(
                          ws.id,
                          `工作区: ${ws.name} (Pi Session)`
                        )
                      }
                      className="p-1.5 rounded text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors border border-transparent hover:border-zinc-700"
                      title="查看实时终端输出"
                    >
                      <Terminal className="w-4 h-4" />
                    </button>
                  </div>

                  <button
                    onClick={() => handleOpenPromptModal(ws)}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 hover:border-zinc-700 transition-colors"
                  >
                    <Sparkles className="w-3 h-3 text-amber-400" />
                    <span>ChatGPT 指令</span>
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Add Workspace Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-dark-card border border-zinc-800 rounded-lg max-w-md w-full p-6 space-y-5 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="space-y-1">
              <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
                <Plus className="w-4 h-4 text-emerald-400" />
                添加本地项目工作区
              </h3>
              <p className="text-xs text-zinc-400">
                输入本地代码工程所在目录的绝对路径，系统将自动读取 Git 状态并准备 Pi Session。
              </p>
            </div>

            {addingError && (
              <div className="p-3 rounded bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300">
                {addingError}
              </div>
            )}

            <div className="space-y-4">
              <div className="space-y-1.5">
                <label className="text-xs text-zinc-300 font-medium">
                  项目根目录路径 (绝对路径)
                </label>
                <input
                  type="text"
                  placeholder="例如: D:\work\NewsNook"
                  value={newPath}
                  onChange={(e) => setNewPath(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs font-mono text-zinc-100 focus:outline-none focus:border-zinc-600"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs text-zinc-300 font-medium">
                  工作区名称 (可选，留空则使用文件夹名)
                </label>
                <input
                  type="text"
                  placeholder="例如: NewsNook 核心系统"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded px-3 py-2 text-xs text-zinc-100 focus:outline-none focus:border-zinc-600"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => setShowAddModal(false)}
                className="px-3 py-1.5 rounded text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800"
              >
                取消
              </button>
              <button
                onClick={handleAddWorkspace}
                disabled={isSubmitting}
                className="px-4 py-1.5 rounded text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-zinc-950 font-semibold disabled:opacity-50"
              >
                {isSubmitting ? "正在添加..." : "添加并就绪"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ChatGPT Prompt Modal */}
      {promptModalWs && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-dark-card border border-zinc-800 rounded-lg max-w-lg w-full p-6 space-y-4 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <h3 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
                  <Sparkles className="w-4 h-4 text-amber-400" />
                  ChatGPT 官方推荐绑定提示词
                </h3>
                <p className="text-xs text-zinc-400 font-mono">
                  {promptModalWs.name} ({promptModalWs.path})
                </p>
              </div>
            </div>

            <div className="p-3 rounded bg-zinc-950 border border-zinc-800/80 font-mono text-xs text-zinc-300 whitespace-pre-wrap leading-relaxed select-text max-h-72 overflow-y-auto">
              {promptText}
            </div>

            <div className="flex items-center justify-between pt-1">
              <p className="text-[11px] text-zinc-500">
                将此内容直接发送给 ChatGPT 网页版，即可以防误触方式精确连接本项目。
              </p>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setPromptModalWs(null)}
                  className="px-3 py-1.5 rounded text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800"
                >
                  关闭
                </button>
                <button
                  onClick={handleCopyPrompt}
                  className="px-3.5 py-1.5 rounded text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-zinc-950 font-semibold flex items-center gap-1.5"
                >
                  {copiedPrompt ? (
                    <Check className="w-3.5 h-3.5" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                  <span>{copiedPrompt ? "已复制到剪贴板" : "复制提示词"}</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
