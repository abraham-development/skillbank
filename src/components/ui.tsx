// Piezas pequeñas de interfaz reutilizables.
import type { ReactNode } from "react";
import { AlertTriangle, Info, Loader2, XCircle } from "lucide-react";
import type { Diagnostic, InstallStatus } from "../lib/api";
import { errorMessage } from "../lib/api";

type Tone = "neutral" | "brand" | "red" | "amber" | "violet" | "green" | "sky";

const TONES: Record<Tone, string> = {
  neutral: "bg-zinc-100 text-zinc-700 ring-zinc-200 dark:bg-zinc-800 dark:text-zinc-300 dark:ring-zinc-700",
  brand: "bg-brand-50 text-brand-700 ring-brand-100 dark:bg-brand-500/15 dark:text-brand-100 dark:ring-brand-500/30",
  red: "bg-red-50 text-red-700 ring-red-200 dark:bg-red-500/15 dark:text-red-300 dark:ring-red-500/30",
  amber: "bg-amber-50 text-amber-800 ring-amber-200 dark:bg-amber-500/15 dark:text-amber-300 dark:ring-amber-500/30",
  violet: "bg-violet-50 text-violet-700 ring-violet-200 dark:bg-violet-500/15 dark:text-violet-300 dark:ring-violet-500/30",
  green: "bg-emerald-50 text-emerald-700 ring-emerald-200 dark:bg-emerald-500/15 dark:text-emerald-300 dark:ring-emerald-500/30",
  sky: "bg-sky-50 text-sky-700 ring-sky-200 dark:bg-sky-500/15 dark:text-sky-300 dark:ring-sky-500/30",
};

export function Badge({ tone = "neutral", children, title }: { tone?: Tone; children: ReactNode; title?: string }) {
  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ring-1 ring-inset ${TONES[tone]}`}
    >
      {children}
    </span>
  );
}

export function TagChip({ tag, onClick, active }: { tag: string; onClick?: () => void; active?: boolean }) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        e.preventDefault();
        onClick?.();
      }}
      className={`rounded-md px-1.5 py-0.5 text-[11px] font-medium transition ${
        active
          ? "bg-brand-600 text-white"
          : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-700"
      }`}
    >
      #{tag}
    </button>
  );
}

const STATUS_TONE: Record<InstallStatus, Tone> = {
  up_to_date: "green",
  outdated: "amber",
  modified: "sky",
  diverged: "red",
  missing: "neutral",
  orphan: "neutral",
};

export function StatusBadge({ status, label }: { status: InstallStatus; label: string }) {
  return <Badge tone={STATUS_TONE[status]}>{label}</Badge>;
}

export function Spinner({ label = "Cargando…" }: { label?: string }) {
  return (
    <div className="flex items-center gap-2 p-8 text-sm text-zinc-500">
      <Loader2 className="size-4 animate-spin" /> {label}
    </div>
  );
}

export function ErrorBox({ error }: { error: unknown }) {
  const hint = error && typeof error === "object" && "hint" in error ? String((error as { hint?: string }).hint ?? "") : "";
  return (
    <div className="m-6 rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-800 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300">
      <div className="font-medium">{errorMessage(error)}</div>
      {hint && <div className="mt-1 opacity-80">{hint}</div>}
    </div>
  );
}

export function Button({
  children,
  onClick,
  variant = "secondary",
  title,
  disabled,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "ghost";
  title?: string;
  disabled?: boolean;
}) {
  const styles = {
    primary: "bg-brand-600 text-white hover:bg-brand-700 shadow-sm",
    secondary:
      "bg-white text-zinc-700 ring-1 ring-inset ring-zinc-200 hover:bg-zinc-50 dark:bg-zinc-900 dark:text-zinc-200 dark:ring-zinc-700 dark:hover:bg-zinc-800",
    ghost: "text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800",
  }[variant];
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition disabled:opacity-50 ${styles}`}
    >
      {children}
    </button>
  );
}

export function DiagnosticsList({ diagnostics }: { diagnostics: Diagnostic[] }) {
  if (diagnostics.length === 0) {
    return <p className="text-sm text-emerald-700 dark:text-emerald-400">✓ Sin observaciones: la skill cumple la especificación.</p>;
  }
  const order = { error: 0, warning: 1, info: 2 };
  const sorted = [...diagnostics].sort((a, b) => order[a.severity] - order[b.severity]);
  return (
    <ul className="space-y-2">
      {sorted.map((d, i) => {
        const Icon = d.severity === "error" ? XCircle : d.severity === "warning" ? AlertTriangle : Info;
        const color =
          d.severity === "error"
            ? "text-red-600 dark:text-red-400"
            : d.severity === "warning"
              ? "text-amber-600 dark:text-amber-400"
              : "text-sky-600 dark:text-sky-400";
        return (
          <li
            key={i}
            className="flex gap-3 rounded-lg border border-zinc-200 bg-white p-3 text-sm dark:border-zinc-800 dark:bg-zinc-900"
          >
            <Icon className={`mt-0.5 size-4 shrink-0 ${color}`} />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="selectable">{d.message}</span>
                {d.tag && <Badge tone="violet">{d.tag}</Badge>}
              </div>
              {d.hint && <div className="selectable mt-1 whitespace-pre-wrap text-zinc-500">→ {d.hint}</div>}
              <div className="mt-1 font-mono text-[11px] text-zinc-400">
                {d.code} · {d.file}
                {d.line ? `:${d.line}` : ""}
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
