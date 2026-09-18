import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CloseConfirmDialog } from "./CloseConfirmDialog";

describe("CloseConfirmDialog", () => {
  it("renders an accessible themed choice between tray and exit", () => {
    const closedMarkup = renderToStaticMarkup(
      <CloseConfirmDialog
        open={false}
        busy={false}
        error={null}
        onResolve={() => undefined}
      />
    );
    const openMarkup = renderToStaticMarkup(
      <CloseConfirmDialog
        open
        busy={false}
        error={null}
        onResolve={() => undefined}
      />
    );

    expect(closedMarkup).toBe("");
    expect(openMarkup).toContain('role="dialog"');
    expect(openMarkup).toContain('aria-modal="true"');
    expect(openMarkup).toContain("最小化到托盘");
    expect(openMarkup).toContain("直接退出");
    expect(openMarkup).toContain("取消");
  });

  it("shows action-neutral progress while resolving the choice", () => {
    const markup = renderToStaticMarkup(
      <CloseConfirmDialog
        open
        busy
        error={null}
        onResolve={() => undefined}
      />
    );

    expect(markup).toContain("正在处理关闭操作");
  });
});
