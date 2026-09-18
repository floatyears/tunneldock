import { describe, expect, it } from "vitest";
import { formatLocalTimestamp } from "./time";

describe("formatLocalTimestamp", () => {
  it("converts an offset timestamp to the requested local timezone", () => {
    expect(
      formatLocalTimestamp("2026-09-18T03:04:04Z", "Asia/Shanghai")
    ).toBe("2026-09-18 11:04:04");
  });

  it("treats legacy timestamps without an offset as UTC", () => {
    expect(
      formatLocalTimestamp("2026-09-18 03:04:04", "Asia/Shanghai")
    ).toBe("2026-09-18 11:04:04");
  });

  it("keeps invalid persisted values visible", () => {
    expect(formatLocalTimestamp("unknown", "Asia/Shanghai")).toBe("unknown");
  });
});
