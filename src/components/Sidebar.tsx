// Barra lateral: filtros de la biblioteca, etiquetas y estado de la instalación.
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useSearchParams } from "react-router";
import { AlertTriangle, CheckCircle2, Library, PlugZap, RefreshCcw, Sparkles, XCircle } from "lucide-react";
import type { ReactNode } from "react";
import { api, prettyPath } from "../lib/api";
import { useLibraryData } from "../lib/useLibraryData";
import { AgentAvatar, STATE_LABEL } from "./AgentAvatar";
import { AGENT_NAMES } from "../lib/format";

function NavItem({
  icon,
  label,
  count,
  active,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  count?: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-sm transition ${
        active
          ? "bg-brand-600/10 font-medium text-brand-700 dark:bg-brand-500/20 dark:text-brand-100"
          : "text-zinc-600 hover:bg-zinc-200/60 dark:text-zinc-400 dark:hover:bg-zinc-800"
      }`}
    >
      <span className="shrink-0">{icon}</span>
      <span className="flex-1 truncate text-left">{label}</span>
      {count !== undefined && count > 0 && <span className="text-xs tabular-nums text-zinc-400">{count}</span>}
    </button>
  );
}

export default function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  const onLibrary = location.pathname === "/";
  const filter = onLibrary ? (params.get("filtro") ?? "todas") : "";
  const activeTag = onLibrary ? params.get("tag") : null;
  const { skills, tags, outdatedBySkill } = useLibraryData();
  const app = useQuery({ queryKey: ["app"], queryFn: api.appInfo });
  const agents = useQuery({ queryKey: ["agents"], queryFn: api.agentsStatus });
  const onAgents = location.pathname === "/agentes";
  const connectedCount = (agents.data ?? []).filter((a) => a.state === "connected").length;

  const go = (next: Record<string, string>) => {
    const p = new URLSearchParams();
    const q = params.get("q");
    if (q && onLibrary) p.set("q", q);
    Object.entries(next).forEach(([k, v]) => v && p.set(k, v));
    navigate({ pathname: "/", search: p.toString() ? `?${p}` : "" });
  };

  const list = skills.data ?? [];
  const counts = {
    todas: list.length,
    avisos: list.filter((s) => s.valid && s.warnings > 0).length,
    invalidas: list.filter((s) => !s.valid).length,
    claude: list.filter((s) => s.claude_code_fields.length > 0).length,
    desactualizadas: Object.keys(outdatedBySkill).length,
  };


  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-zinc-200 bg-zinc-100/70 dark:border-zinc-800 dark:bg-zinc-900/60">
      <div className="flex items-center gap-2.5 px-4 pt-4 pb-5">
        <img src="/icon.png" alt="" className="size-8 rounded-lg" />
        <div>
          <div className="text-sm font-semibold leading-tight">Banco de Skills</div>
          <div className="text-[11px] text-zinc-500">{app.data ? `v${app.data.version}` : " "}</div>
        </div>
      </div>

      <nav className="space-y-0.5 px-2">
        <NavItem icon={<Library className="size-4" />} label="Todas" count={counts.todas} active={filter === "todas" && !activeTag} onClick={() => go({})} />
        <NavItem icon={<RefreshCcw className="size-4" />} label="Desactualizadas" count={counts.desactualizadas} active={filter === "desactualizadas"} onClick={() => go({ filtro: "desactualizadas" })} />
        <NavItem icon={<AlertTriangle className="size-4" />} label="Con avisos" count={counts.avisos} active={filter === "avisos"} onClick={() => go({ filtro: "avisos" })} />
        <NavItem icon={<XCircle className="size-4" />} label="Inválidas" count={counts.invalidas} active={filter === "invalidas"} onClick={() => go({ filtro: "invalidas" })} />
        <NavItem icon={<Sparkles className="size-4" />} label="Solo Claude Code" count={counts.claude} active={filter === "claude"} onClick={() => go({ filtro: "claude" })} />
        <div className="my-2 border-t border-zinc-200 dark:border-zinc-800" />
        <NavItem icon={<PlugZap className="size-4" />} label="Agentes" count={connectedCount} active={onAgents} onClick={() => navigate("/agentes")} />
      </nav>

      <div className="mt-6 px-4 text-[11px] font-semibold tracking-wider text-zinc-400 uppercase">Etiquetas</div>
      <div className="mt-2 min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-4">
        {(tags.data ?? []).length === 0 && <p className="px-2.5 text-xs text-zinc-400">Aún no hay etiquetas.</p>}
        {(tags.data ?? []).map((t) => (
          <NavItem
            key={t.tag}
            icon={<span className="text-zinc-400">#</span>}
            label={t.tag}
            count={t.count}
            active={activeTag === t.tag}
            onClick={() => go({ tag: t.tag })}
          />
        ))}
      </div>

      <div className="space-y-1.5 border-t border-zinc-200 px-4 py-3 text-[11px] text-zinc-500 dark:border-zinc-800">
        {app.data && (
          <>
            <button
              type="button"
              className="block w-full truncate text-left hover:text-brand-600"
              title="Abrir la carpeta del banco"
              onClick={() => api.openPath(app.data!.bank_path)}
            >
              📁 {prettyPath(app.data.bank_path, app.data.home)}
            </button>
            <div className="flex items-center gap-1.5">
              {app.data.cli_on_path ? (
                <>
                  <CheckCircle2 className="size-3.5 text-emerald-500" /> CLI <code>skillbank</code> disponible
                </>
              ) : (
                <>
                  <XCircle className="size-3.5 text-red-500" /> CLI no está en el PATH
                </>
              )}
            </div>
            <button
              type="button"
              onClick={() => navigate("/agentes")}
              className="flex w-full items-center gap-2 rounded-lg py-1 text-left hover:text-brand-600"
              title="Ver la conexión de tus agentes"
            >
              <span className="flex gap-1.5">
                {(agents.data ?? []).map((a) => (
                  <AgentAvatar
                    key={a.agent}
                    agent={a.agent}
                    state={a.state}
                    size="sm"
                    title={`${AGENT_NAMES[a.agent]}: ${STATE_LABEL[a.state]}`}
                  />
                ))}
              </span>
              <span className="flex-1 truncate">
                {connectedCount > 0 ? `${connectedCount} conectado${connectedCount > 1 ? "s" : ""}` : "Conectar agentes"}
              </span>
            </button>
          </>
        )}
      </div>
    </aside>
  );
}
