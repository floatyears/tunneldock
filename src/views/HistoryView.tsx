import React, { useState, useMemo } from "react";
import {
  Search,
  Download,
  Trash2,
  Code2,
  CheckCircle2,
  XCircle,
  Copy,
  Check,
  RefreshCw,
  Eye,
} from "lucide-react";
import { McpCallRecord } from "../types";
import { clearHistory, exportHistoryJson } from "../api";

interface HistoryViewProps {
  history: McpCallRecord[];
  onRefresh: () => void;
}

export const HistoryView: React.FC<HistoryViewProps> = ({
  history,
  onRefresh,
}) => {
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedTool, setSelectedTool] = useState<string>("all");
  const [selectedStatus, setSelectedStatus] = useState<string>("all");
  const [inspectRecord, setInspectRecord] = useState<McpCallRecord | null>(null);
  const [copiedInspect, setCopiedInspect] = useState(false);

  const filteredHistory = useMemo(() => {
    return history.filter((item) => {
      const matchSearch =
        searchTerm === "" ||
        item.tool_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.args_json.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.result_summary.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (item.workspace_name &&
          item.workspace_name.toLowerCase().includes(searchTerm.toLowerCase()));

      const matchTool =
        selectedTool === "all" || item.tool_name === selectedTool;

      const matchStatus =
        selectedStatus === "all" || item.status === selectedStatus;

      return matchSearch && matchTool && matchStatus;
    });
  }, [history, searchTerm, selectedTool, selectedStatus]);

  // Metric stats
  const stats = useMemo(() => {
    const total = history.length;
    const readCount = history.filter((i) => i.tool_name === "read").length;
    const bashCount = history.filter((i) => i.tool_name === "bash").length;
    const writeCount = history.filter(
      (i) => i.tool_name === "write" || i.tool_name === "edit"
    ).length;
    const successCount = history.filter((i) => i.status === "success").length;
    const avgDuration =
      total > 0
        ? Math.round(
            history.reduce((acc, curr) => acc + curr.duration_ms, 0) / total
          )
        : 0;

    return {
      total,
      readCount,
      bashCount,
      writeCount,
      successRate: total > 0 ? Math.round((successCount / total) * 100) : 100,
      avgDuration,
    };
  }, [history]);

  const handleClear = async () => {
    if (confirm("确定要清空所有 MCP 调用历史记录吗？")) {
      await clearHistory();
      onRefresh();
    }
  };

  const handleExport = async () => {
    try {
      const json = await exportHistoryJson();
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `tunneldock-mcp-history-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error(e);
    }
  };

  const getToolBadge = (name: string) => {
    switch (name) {
      case "read":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
            read
          </span>
        );
      case "bash":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-950/50 text-amber-400 border border-amber-800/60">
            bash
          </span>
        );
      case "edit":
      case "write":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-950/50 text-emerald-400 border border-emerald-800/60">
            {name}
          </span>
        );
      case "init":
      case "sessions":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 text-zinc-200 border border-zinc-700 font-semibold">
            {name}
          </span>
        );
      default:
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-900 text-zinc-400 border border-zinc-800">
            {name}
          </span>
        );
    }
  };

  return (
    <div className="p-6 space-y-6 max-w-6xl mx-auto">
      {/* Top Banner */}
      <div className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg flex items-center justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold text-zinc-100">
              MCP 工具调用审计与追踪
            </h2>
            <span className="text-xs font-mono text-zinc-500">
              ({history.length} 条记录)
            </span>
          </div>
          <p className="text-xs text-zinc-400 max-w-xl">
            完整捕获云端 ChatGPT 通过 Tunnel 下发给本地的一切工具调用（read/bash/edit/write/transfer/sessions 等），确保本地开发操作透明与安全追溯。
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onRefresh}
            className="p-2 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors"
            title="刷新历史"
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 transition-colors"
          >
            <Download className="w-3.5 h-3.5" />
            <span>导出 JSON</span>
          </button>
          <button
            onClick={handleClear}
            className="flex items-center gap-1.5 px-3 py-2 rounded text-xs font-medium bg-rose-950/30 hover:bg-rose-900/40 text-rose-300 border border-rose-800/60 transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>清空审计</span>
          </button>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 md:grid-cols-6 gap-3">
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">总调用次数</div>
          <div className="text-lg font-bold font-mono text-zinc-100">
            {stats.total}
          </div>
        </div>
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">只读检查 (read)</div>
          <div className="text-lg font-bold font-mono text-zinc-300">
            {stats.readCount}
          </div>
        </div>
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">命令执行 (bash)</div>
          <div className="text-lg font-bold font-mono text-amber-400">
            {stats.bashCount}
          </div>
        </div>
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">代码修改 (write)</div>
          <div className="text-lg font-bold font-mono text-emerald-400">
            {stats.writeCount}
          </div>
        </div>
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">调用成功率</div>
          <div className="text-lg font-bold font-mono text-emerald-400">
            {stats.successRate}%
          </div>
        </div>
        <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 space-y-1">
          <div className="text-[11px] font-mono text-zinc-500">平均耗时</div>
          <div className="text-lg font-bold font-mono text-zinc-300">
            {stats.avgDuration} ms
          </div>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="p-3 rounded-lg bg-dark-card border border-zinc-800 flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2 flex-1 min-w-[240px]">
          <div className="relative w-full max-w-sm">
            <Search className="w-3.5 h-3.5 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="搜索参数、工具名、命令内容..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full bg-zinc-950 border border-zinc-800 rounded pl-8 pr-3 py-1.5 text-xs text-zinc-100 focus:outline-none focus:border-zinc-600 font-mono"
            />
          </div>
        </div>

        <div className="flex items-center gap-3">
          {/* Tool Filter */}
          <div className="flex items-center gap-1.5 text-xs">
            <span className="text-zinc-500 font-mono">工具:</span>
            <select
              value={selectedTool}
              onChange={(e) => setSelectedTool(e.target.value)}
              className="bg-zinc-950 border border-zinc-800 rounded px-2 py-1 text-xs text-zinc-300 font-mono focus:outline-none"
            >
              <option value="all">全部工具</option>
              <option value="read">read (读取文件)</option>
              <option value="bash">bash (运行命令)</option>
              <option value="edit">edit (修改代码)</option>
              <option value="write">write (写入文件)</option>
              <option value="transfer">transfer (文件传输)</option>
              <option value="sessions">sessions (会话列表)</option>
              <option value="init">init (绑定工作区)</option>
              <option value="chat">chat (向本地发送消息)</option>
            </select>
          </div>

          {/* Status Filter */}
          <div className="flex items-center gap-1.5 text-xs">
            <span className="text-zinc-500 font-mono">状态:</span>
            <select
              value={selectedStatus}
              onChange={(e) => setSelectedStatus(e.target.value)}
              className="bg-zinc-950 border border-zinc-800 rounded px-2 py-1 text-xs text-zinc-300 font-mono focus:outline-none"
            >
              <option value="all">全部状态</option>
              <option value="success">成功 (Success)</option>
              <option value="error">失败 (Error)</option>
            </select>
          </div>
        </div>
      </div>

      {/* History Table */}
      <div className="rounded-lg bg-dark-card border border-zinc-800 overflow-hidden">
        {filteredHistory.length === 0 ? (
          <div className="p-8 text-center text-xs text-zinc-500 italic">
            没有匹配的 MCP 调用记录
          </div>
        ) : (
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="border-b border-zinc-800 bg-zinc-950/60 font-mono text-[11px] text-zinc-400 select-none">
                <th className="py-2.5 px-4">时间戳</th>
                <th className="py-2.5 px-3">工具</th>
                <th className="py-2.5 px-3">工作区 / Session</th>
                <th className="py-2.5 px-3">调用参数</th>
                <th className="py-2.5 px-3">执行耗时</th>
                <th className="py-2.5 px-3">状态</th>
                <th className="py-2.5 px-4 text-right">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-800/60 font-mono">
              {filteredHistory.map((item) => (
                <tr
                  key={item.id}
                  className="hover:bg-zinc-900/50 transition-colors"
                >
                  <td className="py-2.5 px-4 text-zinc-400 whitespace-nowrap">
                    {item.timestamp}
                  </td>
                  <td className="py-2.5 px-3">{getToolBadge(item.tool_name)}</td>
                  <td className="py-2.5 px-3 text-zinc-300 truncate max-w-[140px]">
                    {item.workspace_name || "默认会话"}
                  </td>
                  <td className="py-2.5 px-3 text-zinc-400 truncate max-w-[280px]">
                    {item.args_json}
                  </td>
                  <td className="py-2.5 px-3 text-zinc-400 whitespace-nowrap">
                    {item.duration_ms} ms
                  </td>
                  <td className="py-2.5 px-3">
                    {item.status === "success" ? (
                      <span className="flex items-center gap-1 text-[11px] text-emerald-400">
                        <CheckCircle2 className="w-3 h-3" />
                        <span>成功</span>
                      </span>
                    ) : (
                      <span className="flex items-center gap-1 text-[11px] text-rose-400">
                        <XCircle className="w-3 h-3" />
                        <span>失败</span>
                      </span>
                    )}
                  </td>
                  <td className="py-2.5 px-4 text-right">
                    <button
                      onClick={() => setInspectRecord(item)}
                      className="px-2 py-1 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-[11px] inline-flex items-center gap-1 transition-colors"
                    >
                      <Eye className="w-3 h-3" />
                      <span>查看</span>
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Inspect Modal */}
      {inspectRecord && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-dark-card border border-zinc-800 rounded-lg max-w-2xl w-full p-6 space-y-4 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-zinc-800 pb-3">
              <div className="space-y-0.5">
                <h3 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
                  <Code2 className="w-4 h-4 text-emerald-400" />
                  MCP 工具调用详情
                </h3>
                <div className="text-[11px] font-mono text-zinc-400 flex items-center gap-3">
                  <span>ID: {inspectRecord.id}</span>
                  <span>时间: {inspectRecord.timestamp}</span>
                  <span>耗时: {inspectRecord.duration_ms}ms</span>
                </div>
              </div>

              <div className="flex items-center gap-2">
                {getToolBadge(inspectRecord.tool_name)}
              </div>
            </div>

            <div className="space-y-3 font-mono text-xs">
              <div className="space-y-1">
                <div className="text-zinc-400 font-semibold">执行参数 / 原始日志:</div>
                <div className="p-3 rounded bg-zinc-950 border border-zinc-800 text-zinc-300 whitespace-pre-wrap max-h-48 overflow-y-auto select-text">
                  {inspectRecord.args_json}
                </div>
              </div>

              <div className="space-y-1">
                <div className="text-zinc-400 font-semibold">返回结果摘要:</div>
                <div className="p-3 rounded bg-zinc-950 border border-zinc-800 text-zinc-300 select-text">
                  {inspectRecord.result_summary}
                </div>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-zinc-800">
              <button
                onClick={() => {
                  navigator.clipboard.writeText(
                    JSON.stringify(inspectRecord, null, 2)
                  );
                  setCopiedInspect(true);
                  setTimeout(() => setCopiedInspect(false), 2000);
                }}
                className="px-3 py-1.5 rounded text-xs bg-zinc-800 hover:bg-zinc-700 text-zinc-200 flex items-center gap-1.5 transition-colors"
              >
                {copiedInspect ? (
                  <Check className="w-3 h-3 text-emerald-400" />
                ) : (
                  <Copy className="w-3 h-3" />
                )}
                <span>{copiedInspect ? "已复制 JSON" : "复制完整数据"}</span>
              </button>
              <button
                onClick={() => setInspectRecord(null)}
                className="px-4 py-1.5 rounded text-xs font-medium bg-zinc-200 hover:bg-white text-zinc-950 font-semibold"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
