import { McpCallRecord } from "../types";

export interface HistoryStats {
  total: number;
  readCount: number;
  bashCount: number;
  writeCount: number;
  successRate: number | null;
  avgDuration: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface PaginatedHistory<T> {
  items: T[];
  page: number;
  totalPages: number;
  start: number;
  end: number;
  total: number;
}

export function formatTokenCount(value: number): string {
  if (value < 1_000) return Math.round(value).toString();

  const divisor = value >= 1_000_000 ? 1_000_000 : 1_000;
  const suffix = value >= 1_000_000 ? "M" : "K";
  const compactValue = Math.round((value / divisor) * 10) / 10;

  return `${compactValue.toFixed(Number.isInteger(compactValue) ? 0 : 1)}${suffix}`;
}

export function paginateHistory<T>(
  records: T[],
  requestedPage: number,
  requestedPageSize: number
): PaginatedHistory<T> {
  const pageSize = Math.max(1, Math.floor(requestedPageSize));
  const total = records.length;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const page = Math.min(
    totalPages,
    Math.max(1, Math.floor(requestedPage) || 1)
  );
  const offset = (page - 1) * pageSize;
  const items = records.slice(offset, offset + pageSize);

  return {
    items,
    page,
    totalPages,
    start: total === 0 ? 0 : offset + 1,
    end: offset + items.length,
    total,
  };
}

export function buildHistoryStats(
  history: McpCallRecord[],
  workspaceId: string
): { records: McpCallRecord[]; stats: HistoryStats } {
  const records: McpCallRecord[] = [];
  let readCount = 0;
  let bashCount = 0;
  let writeCount = 0;
  let successCount = 0;
  let durationTotal = 0;
  let inputTokens = 0;
  let outputTokens = 0;
  let totalTokens = 0;

  for (const record of history) {
    if (workspaceId !== "all" && record.workspace_id !== workspaceId) {
      continue;
    }

    records.push(record);
    if (record.tool_name === "read") readCount += 1;
    if (record.tool_name === "bash") bashCount += 1;
    if (record.tool_name === "write" || record.tool_name === "edit") {
      writeCount += 1;
    }
    if (record.status === "success") successCount += 1;
    durationTotal += record.duration_ms;
    inputTokens += record.input_tokens ?? 0;
    outputTokens += record.output_tokens ?? 0;
    totalTokens += record.total_tokens ?? 0;
  }

  const total = records.length;
  return {
    records,
    stats: {
      total,
      readCount,
      bashCount,
      writeCount,
      successRate: total > 0 ? Math.round((successCount / total) * 100) : null,
      avgDuration: total > 0 ? Math.round(durationTotal / total) : 0,
      inputTokens,
      outputTokens,
      totalTokens,
    },
  };
}
