import React, { useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ClipboardPaste,
  Eye,
  FileCode2,
  Inbox,
  Loader2,
  RotateCcw,
  ShieldCheck,
  X,
} from "lucide-react";
import { applyPatch, previewPatch } from "../api";
import { PatchPreview, WorkspaceItem } from "../types";
import { useTranslation } from "../i18n";

interface PatchInboxViewProps {
  workspaces: WorkspaceItem[];
  onRefreshWorkspaces: () => Promise<void>;
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

export const PatchInboxView: React.FC<PatchInboxViewProps> = ({
  workspaces,
  onRefreshWorkspaces,
}) => {
  const { t } = useTranslation();
  const [workspaceId, setWorkspaceId] = useState("");
  const [patchText, setPatchText] = useState("");
  const [preview, setPreview] = useState<PatchPreview | null>(null);
  const [diffOpen, setDiffOpen] = useState(false);
  const [busy, setBusy] = useState<"paste" | "preview" | "apply" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    if (!workspaces.some((workspace) => workspace.id === workspaceId)) {
      setWorkspaceId(workspaces[0]?.id ?? "");
      setPreview(null);
    }
  }, [workspaceId, workspaces]);

  const resetPreview = (nextPatch = patchText) => {
    setPatchText(nextPatch);
    setPreview(null);
    setDiffOpen(false);
    setError(null);
    setSuccess(null);
  };

  const handlePaste = async () => {
    setBusy("paste");
    setError(null);
    setSuccess(null);
    try {
      if (!navigator.clipboard?.readText) {
        throw new Error(t("patch_view.clipboard_unavailable"));
      }
      resetPreview(await navigator.clipboard.readText());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(null);
    }
  };

  const handlePreview = async () => {
    if (!workspaceId) return;
    setBusy("preview");
    setError(null);
    setSuccess(null);
    setPreview(null);
    setDiffOpen(false);
    try {
      setPreview(await previewPatch(workspaceId, patchText));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(null);
    }
  };

  const handleApply = async () => {
    if (!workspaceId || !preview?.can_apply) return;
    setBusy("apply");
    setError(null);
    setSuccess(null);
    try {
      const result = await applyPatch(workspaceId, patchText);
      setSuccess(result.message);
      setPatchText("");
      setPreview(null);
      setDiffOpen(false);
      await onRefreshWorkspaces();
    } catch (reason) {
      setError(errorMessage(reason));
      setPreview(null);
    } finally {
      setBusy(null);
    }
  };

  const handleReject = () => resetPreview("");
  const isBusy = busy !== null;

  return (
    <div className="p-6 space-y-5 max-w-6xl mx-auto">
      <section className="p-5 rounded-lg bg-dark-card border border-zinc-800/90 shadow-lg">
        <div className="flex items-start justify-between gap-5">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <Inbox className="w-4 h-4 text-zinc-300" />
              <h2 className="text-base font-semibold text-zinc-100">
                {t("patch_view.title")}
              </h2>
            </div>
            <p className="text-xs text-zinc-400 max-w-2xl leading-relaxed">
              {t("patch_view.description")}
            </p>
          </div>
          <label className="shrink-0 flex items-center gap-2 text-xs text-zinc-400">
            <span>{t("patch_view.workspace")}</span>
            <select
              value={workspaceId}
              onChange={(event) => {
                setWorkspaceId(event.target.value);
                setPreview(null);
                setDiffOpen(false);
                setError(null);
                setSuccess(null);
              }}
              disabled={workspaces.length === 0 || isBusy}
              className="max-w-64 rounded border border-zinc-700 bg-zinc-900 px-2.5 py-2 text-xs text-zinc-200 outline-none focus:border-zinc-500 disabled:opacity-50"
            >
              {workspaces.length === 0 && (
                <option value="">{t("patch_view.no_workspaces")}</option>
              )}
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name} — {workspace.path}
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>

      {workspaces.length === 0 ? (
        <div className="p-8 rounded-lg border border-dashed border-zinc-700 bg-zinc-900/40 text-center text-sm text-zinc-400">
          {t("patch_view.no_workspaces")}
        </div>
      ) : (
        <>
          <section className="rounded-lg bg-dark-card border border-zinc-800/90 overflow-hidden">
            <div className="px-4 py-3 flex items-center justify-between border-b border-zinc-800">
              <label
                htmlFor="patch-inbox-input"
                className="text-xs font-medium text-zinc-300"
              >
                {t("patch_view.input_label")}
              </label>
              <button
                type="button"
                onClick={handlePaste}
                disabled={isBusy}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded border border-zinc-700 bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-200 disabled:opacity-50"
              >
                {busy === "paste" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <ClipboardPaste className="w-3.5 h-3.5" />
                )}
                {t("patch_view.paste_clipboard")}
              </button>
            </div>
            <textarea
              id="patch-inbox-input"
              value={patchText}
              onChange={(event) => resetPreview(event.target.value)}
              placeholder={t("patch_view.placeholder")}
              spellCheck={false}
              disabled={isBusy}
              className="w-full min-h-64 resize-y bg-zinc-950/70 px-4 py-3 font-mono text-[11px] leading-relaxed text-zinc-200 placeholder:text-zinc-600 outline-none disabled:opacity-60"
            />
            <div className="px-4 py-2 border-t border-zinc-800 text-[10px] text-zinc-500 leading-relaxed">
              {t("patch_view.sha_hint")}
            </div>
          </section>

          {error && (
            <div
              role="alert"
              className="p-3 rounded border border-rose-800/70 bg-rose-950/30 text-xs text-rose-200 whitespace-pre-wrap break-words"
            >
              {error}
            </div>
          )}
          {success && (
            <div
              role="status"
              className="p-3 rounded border border-emerald-800/70 bg-emerald-950/30 text-xs text-emerald-200"
            >
              {success}
            </div>
          )}

          {preview && (
            <section className="rounded-lg bg-dark-card border border-zinc-800/90 overflow-hidden">
              <div className="px-4 py-3 border-b border-zinc-800 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <FileCode2 className="w-4 h-4 text-zinc-400" />
                  <h3 className="text-xs font-semibold text-zinc-200">
                    {t("patch_view.files")}
                  </h3>
                  <span className="text-[10px] font-mono text-zinc-500">
                    {t("patch_view.file_count", {
                      count: preview.files.length,
                    })}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setDiffOpen((open) => !open)}
                    className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded border border-zinc-700 bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-200"
                  >
                    <Eye className="w-3.5 h-3.5" />
                    {diffOpen
                      ? t("patch_view.hide_diff")
                      : t("patch_view.view_diff")}
                  </button>
                  <button
                    type="button"
                    onClick={handleReject}
                    disabled={isBusy}
                    className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-xs text-zinc-300 disabled:opacity-50"
                  >
                    <X className="w-3.5 h-3.5" />
                    {t("patch_view.reject")}
                  </button>
                  <button
                    type="button"
                    onClick={handleApply}
                    disabled={!preview.can_apply || isBusy}
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded border border-emerald-700/80 bg-emerald-900/50 hover:bg-emerald-800/60 text-xs font-semibold text-emerald-100 disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    {busy === "apply" ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Check className="w-3.5 h-3.5" />
                    )}
                    {t("patch_view.apply")}
                  </button>
                </div>
              </div>

              <div
                role={preview.can_apply ? "status" : "alert"}
                className={
                  "px-4 py-3 flex items-start gap-2 text-xs border-b border-zinc-800 " +
                  (preview.can_apply
                    ? "bg-emerald-950/20 text-emerald-200"
                    : "bg-amber-950/20 text-amber-200")
                }
              >
                {preview.can_apply ? (
                  <ShieldCheck className="w-4 h-4 shrink-0" />
                ) : (
                  <AlertTriangle className="w-4 h-4 shrink-0" />
                )}
                <span className="break-words">{preview.validation_message}</span>
              </div>

              <div className="divide-y divide-zinc-800/80">
                {preview.files.map((file) => (
                  <div
                    key={file.path}
                    className="px-4 py-3 grid grid-cols-[minmax(0,1fr)_auto] gap-4"
                  >
                    <div className="min-w-0 space-y-2">
                      <div className="flex items-center gap-2 min-w-0">
                        <span
                          className={
                            "shrink-0 px-1.5 py-0.5 rounded border text-[10px] font-mono font-bold " +
                            (file.change_type === "A"
                              ? "border-emerald-800 bg-emerald-950/50 text-emerald-300"
                              : file.change_type === "D"
                                ? "border-rose-800 bg-rose-950/50 text-rose-300"
                                : "border-sky-800 bg-sky-950/50 text-sky-300")
                          }
                        >
                          {file.change_type}
                        </span>
                        <code className="text-xs text-zinc-200 truncate select-text">
                          {file.path}
                        </code>
                        <span className="shrink-0 text-[10px] font-mono">
                          <span className="text-emerald-400">
                            +{file.additions}
                          </span>
                          <span className="text-zinc-600"> / </span>
                          <span className="text-rose-400">
                            -{file.deletions}
                          </span>
                        </span>
                      </div>
                      <div className="grid sm:grid-cols-2 gap-x-5 gap-y-1 text-[10px] font-mono">
                        <div className="min-w-0">
                          <span className="text-zinc-500">
                            {t("patch_view.expected_sha")}{" "}
                          </span>
                          <span className="text-zinc-300 break-all select-text">
                            {file.expected_sha256 ?? t("patch_view.missing")}
                          </span>
                        </div>
                        <div className="min-w-0">
                          <span className="text-zinc-500">
                            {t("patch_view.current_sha")}{" "}
                          </span>
                          <span className="text-zinc-300 break-all select-text">
                            {file.current_sha256 ?? t("patch_view.absent")}
                          </span>
                        </div>
                      </div>
                    </div>
                    <div
                      className={
                        "flex items-start gap-1.5 text-[10px] " +
                        (file.hash_matches
                          ? "text-emerald-400"
                          : "text-rose-300")
                      }
                    >
                      {file.hash_matches ? (
                        <CheckCircle2 className="w-3.5 h-3.5" />
                      ) : (
                        <AlertTriangle className="w-3.5 h-3.5" />
                      )}
                      <span>
                        {file.hash_matches
                          ? t("patch_view.hash_match")
                          : t("patch_view.hash_mismatch")}
                      </span>
                    </div>
                  </div>
                ))}
              </div>

              {diffOpen && (
                <pre className="max-h-[28rem] overflow-auto border-t border-zinc-800 bg-zinc-950 p-4 font-mono text-[11px] leading-relaxed text-zinc-300 whitespace-pre select-text">
                  {preview.diff}
                </pre>
              )}
            </section>
          )}

          {!preview && (
            <div className="flex items-center justify-between gap-3">
              <p className="text-[11px] text-zinc-500 flex items-center gap-1.5">
                <RotateCcw className="w-3 h-3" />
                {t("patch_view.preview_before_apply")}
              </p>
              <button
                type="button"
                onClick={handlePreview}
                disabled={!patchText.trim() || isBusy || !workspaceId}
                className="inline-flex items-center gap-1.5 px-3 py-2 rounded border border-zinc-600 bg-zinc-800 hover:bg-zinc-700 text-xs font-medium text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40"
              >
                {busy === "preview" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Eye className="w-3.5 h-3.5" />
                )}
                {t("patch_view.preview")}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};
