import { describe, expect, it } from "vitest";
import { McpCallRecord } from "../types";
import {
  buildHistoryStats,
  formatTokenCount,
  paginateHistory,
} from "./historyStats";

const history = [
  {
    id: "a-read",
    timestamp: "2026-09-18 10:00:00",
    session_id: "session-a",
    workspace_id: "workspace-a",
    workspace_name: "Alpha",
    tool_name: "read",
    args_json: "{}",
    result_summary: "ok",
    status: "success",
    duration_ms: 20,
    input_tokens: 5,
    output_tokens: 7,
    total_tokens: 12,
  },
  {
    id: "a-bash",
    timestamp: "2026-09-18 10:01:00",
    session_id: "session-a",
    workspace_id: "workspace-a",
    workspace_name: "Alpha",
    tool_name: "bash",
    args_json: "{}",
    result_summary: "denied",
    status: "error",
    duration_ms: 40,
    input_tokens: 3,
    output_tokens: 4,
    total_tokens: 7,
  },
  {
    id: "b-write",
    timestamp: "2026-09-18 10:02:00",
    session_id: "session-b",
    workspace_id: "workspace-b",
    workspace_name: "Beta",
    tool_name: "write",
    args_json: "{}",
    result_summary: "ok",
    status: "success",
    duration_ms: 10,
    input_tokens: 2,
    output_tokens: 8,
    total_tokens: 10,
  },
  {
    id: "legacy",
    timestamp: "2026-09-17 09:00:00",
    session_id: null,
    workspace_name: "Legacy",
    tool_name: "edit",
    args_json: "{}",
    result_summary: "ok",
    status: "success",
    duration_ms: 0,
  },
] as McpCallRecord[];

describe("buildHistoryStats", () => {
  it("aggregates actual calls and tokens across all workspaces", () => {
    const result = buildHistoryStats(history, "all");

    expect(result.records.map((record) => record.id)).toEqual([
      "a-read",
      "a-bash",
      "b-write",
      "legacy",
    ]);
    expect(result.stats).toEqual({
      total: 4,
      readCount: 1,
      bashCount: 1,
      writeCount: 2,
      successRate: 75,
      avgDuration: 18,
      inputTokens: 10,
      outputTokens: 19,
      totalTokens: 29,
    });
  });

  it("isolates one workspace by its stable id", () => {
    const result = buildHistoryStats(history, "workspace-a");

    expect(result.records.map((record) => record.id)).toEqual([
      "a-read",
      "a-bash",
    ]);
    expect(result.stats).toEqual({
      total: 2,
      readCount: 1,
      bashCount: 1,
      writeCount: 0,
      successRate: 50,
      avgDuration: 30,
      inputTokens: 8,
      outputTokens: 11,
      totalTokens: 19,
    });
  });

  it("returns empty real statistics for a workspace without calls", () => {
    expect(buildHistoryStats(history, "workspace-empty")).toEqual({
      records: [],
      stats: {
        total: 0,
        readCount: 0,
        bashCount: 0,
        writeCount: 0,
        successRate: null,
        avgDuration: 0,
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
      },
    });
  });
});

describe("formatTokenCount", () => {
  it.each([
    [0, "0"],
    [481, "481"],
    [1_000, "1K"],
    [6_101, "6.1K"],
    [65_071, "65.1K"],
    [1_000_000, "1M"],
    [2_560_000, "2.6M"],
  ])("formats %i as %s", (value, expected) => {
    expect(formatTokenCount(value)).toBe(expected);
  });
});

describe("paginateHistory", () => {
  const records = Array.from({ length: 45 }, (_, index) => ({
    id: `record-${index + 1}`,
  }));

  it("returns only the requested page and its display range", () => {
    const result = paginateHistory(records, 2, 20);

    expect(result.items.map((record) => record.id)).toEqual(
      Array.from({ length: 20 }, (_, index) => `record-${index + 21}`)
    );
    expect(result.page).toBe(2);
    expect(result.totalPages).toBe(3);
    expect(result.start).toBe(21);
    expect(result.end).toBe(40);
    expect(result.total).toBe(45);
  });

  it("clamps a stale page after filtering reduces the result set", () => {
    const result = paginateHistory(records.slice(0, 5), 3, 20);

    expect(result.page).toBe(1);
    expect(result.items.map((record) => record.id)).toEqual([
      "record-1",
      "record-2",
      "record-3",
      "record-4",
      "record-5",
    ]);
  });

  it("returns a stable empty first page", () => {
    expect(paginateHistory([], 4, 20)).toEqual({
      items: [],
      page: 1,
      totalPages: 1,
      start: 0,
      end: 0,
      total: 0,
    });
  });
});
