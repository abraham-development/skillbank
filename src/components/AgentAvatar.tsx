// Ícono de cada agente: iniciales con color propio (sin logos de terceros) y punto de estado.
import type { AgentKey, ConnState } from "../lib/api";

const STYLE: Record<AgentKey, { initials: string; bg: string }> = {
  claude: { initials: "CC", bg: "bg-[#d97757]" },
  cursor: { initials: "Cu", bg: "bg-zinc-800 dark:bg-zinc-600" },
  antigravity: { initials: "AG", bg: "bg-[#4285f4]" },
};

export const STATE_LABEL: Record<ConnState, string> = {
  connected: "Conectado",
  ready: "Listo para conectar",
  needs_guide: "Falta la guía",
  not_installed: "No instalado",
};

const DOT: Record<ConnState, string> = {
  connected: "bg-emerald-500",
  ready: "bg-sky-500",
  needs_guide: "bg-amber-500",
  not_installed: "bg-zinc-400",
};

export function AgentAvatar({
  agent,
  state,
  size = "md",
  title,
}: {
  agent: AgentKey;
  state?: ConnState;
  size?: "sm" | "md" | "lg";
  title?: string;
}) {
  const s = STYLE[agent];
  const dims = { sm: "size-6 text-[10px]", md: "size-8 text-xs", lg: "size-12 text-base" }[size];
  const dot = { sm: "size-2", md: "size-2.5", lg: "size-3.5" }[size];
  const dim = state === "not_installed" ? "opacity-40 grayscale" : "";
  return (
    <span className="relative inline-flex shrink-0" title={title}>
      <span className={`inline-flex items-center justify-center rounded-lg font-bold text-white ${s.bg} ${dims} ${dim}`}>
        {s.initials}
      </span>
      {state && (
        <span
          className={`absolute -right-0.5 -bottom-0.5 rounded-full ring-2 ring-zinc-100 dark:ring-zinc-900 ${dot} ${DOT[state]}`}
        />
      )}
    </span>
  );
}
