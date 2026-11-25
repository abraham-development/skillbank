// Diálogo "Instalar…": elige global (usuario) o proyecto, los agentes, revisa la vista previa
// (carpetas destino, duplicados, conflictos) y aplica.
import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, FolderOpen, Globe, Loader2, X } from "lucide-react";
import { api, errorMessage, prettyPath, type AgentKey, type PullItem } from "../lib/api";
import { AGENT_NAMES } from "../lib/format";
import { AgentAvatar } from "./AgentAvatar";
import { Badge, Button } from "./ui";

type Scope = "global" | "project";

const ACTION: Record<PullItem["action"], { label: string; tone: "green" | "sky" | "neutral" | "red" }> = {
  installed: { label: "se instalará", tone: "green" },
  updated: { label: "se actualizará", tone: "sky" },
  unchanged: { label: "ya está al día", tone: "neutral" },
  conflict: { label: "conflicto", tone: "red" },
};

const REASON: Record<string, string> = {
  modified: "la copia de esa carpeta tiene cambios que no están en el banco",
  unregistered: "ya hay una copia distinta que no se instaló desde el banco",
  symlink: "en esa carpeta hay un enlace simbólico",
  not_a_dir: "hay un archivo con ese nombre",
};

export default function InstallDialog({ name, home, onClose }: { name: string; home?: string; onClose: () => void }) {
  const qc = useQueryClient();
  const [scope, setScope] = useState<Scope>("project");
  const [project, setProject] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentKey[]>([]);
  const [force, setForce] = useState(false);
  const [done, setDone] = useState<string[] | null>(null);

  const status = useQuery({ queryKey: ["agents"], queryFn: api.agentsStatus });
  const recent = useQuery({ queryKey: ["recent-projects"], queryFn: api.recentProjects });

  // Por defecto, los agentes instalados en esta computadora.
  useEffect(() => {
    if (status.data && agents.length === 0) {
      const present = status.data.filter((a) => a.present).map((a) => a.agent);
      setAgents(present.length > 0 ? present : ["claude"]);
    }
  }, [status.data]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const ready = agents.length > 0 && (scope === "global" || !!project);
  const preview = useQuery({
    queryKey: ["install-preview", name, scope, project, agents],
    queryFn: () => api.installPreview(name, scope, scope === "project" ? project : null, agents),
    enabled: ready,
  });

  const apply = useMutation({
    mutationFn: () => api.installApply(name, scope, scope === "project" ? project : null, agents, force),
    onSuccess: (out) => {
      setDone(out.items.map((i) => i.path));
      qc.invalidateQueries({ queryKey: ["installs"] });
      qc.invalidateQueries({ queryKey: ["skill", name] });
      qc.invalidateQueries({ queryKey: ["recent-projects"] });
    },
  });

  const pickFolder = async () => {
    const dir = await open({ directory: true, multiple: false, title: "Elige la carpeta del proyecto" });
    if (typeof dir === "string") setProject(dir);
  };
  const toggle = (a: AgentKey) => setAgents((cur) => (cur.includes(a) ? cur.filter((x) => x !== a) : [...cur, a]));

  const p = preview.data;
  const allDone = !!p && p.items.every((i) => i.action === "unchanged");
  const blocked = !ready || preview.isLoading || !!preview.error || (p?.conflicts && !force) || allDone;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6" onClick={onClose}>
      <div
        className="flex max-h-full w-full max-w-2xl flex-col overflow-hidden rounded-2xl bg-white shadow-2xl dark:bg-zinc-900"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between border-b border-zinc-200 px-5 py-3 dark:border-zinc-800">
          <h2 className="font-semibold">
            Instalar <span className="font-mono">{name}</span>
          </h2>
          <button type="button" onClick={onClose} className="rounded p-1 text-zinc-400 hover:text-zinc-700">
            <X className="size-5" />
          </button>
        </header>

        {done ? (
          <div className="space-y-3 p-6">
            <div className="flex items-center gap-2 text-emerald-700 dark:text-emerald-400">
              <CheckCircle2 className="size-5" /> <span className="font-medium">Instalada en:</span>
            </div>
            <ul className="space-y-1 font-mono text-xs">
              {done.map((d) => (
                <li key={d}>{prettyPath(d, home)}</li>
              ))}
            </ul>
            <p className="text-sm text-zinc-500">
              Claude Code la detecta al momento; en Cursor puede hacer falta reiniciar y en Antigravity, recargar las skills.
            </p>
            <div className="flex justify-end">
              <Button variant="primary" onClick={onClose}>
                Listo
              </Button>
            </div>
          </div>
        ) : (
          <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-5">
            <section>
              <h3 className="mb-2 text-xs font-semibold tracking-wider text-zinc-400 uppercase">¿Dónde?</h3>
              <div className="grid grid-cols-2 gap-2">
                {(
                  [
                    ["project", FolderOpen, "Solo un proyecto", "Se copia dentro de la carpeta del proyecto."],
                    ["global", Globe, "Global (usuario)", "Disponible en todos tus proyectos."],
                  ] as const
                ).map(([key, Icon, title, desc]) => (
                  <button
                    type="button"
                    key={key}
                    onClick={() => setScope(key)}
                    className={`rounded-xl border p-3 text-left transition ${
                      scope === key
                        ? "border-brand-500 bg-brand-50 ring-2 ring-brand-500/20 dark:bg-brand-500/10"
                        : "border-zinc-200 hover:border-zinc-300 dark:border-zinc-700"
                    }`}
                  >
                    <div className="flex items-center gap-2 text-sm font-medium">
                      <Icon className="size-4" /> {title}
                    </div>
                    <div className="mt-1 text-xs text-zinc-500">{desc}</div>
                  </button>
                ))}
              </div>
              {scope === "project" && (
                <div className="mt-3 space-y-2">
                  <div className="flex gap-2">
                    <div className="min-w-0 flex-1 truncate rounded-lg border border-zinc-200 px-3 py-1.5 font-mono text-xs leading-6 dark:border-zinc-700">
                      {project ? prettyPath(project, home) : <span className="text-zinc-400">Ninguna carpeta elegida</span>}
                    </div>
                    <Button onClick={pickFolder}>Elegir carpeta…</Button>
                  </div>
                  {(recent.data ?? []).length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {(recent.data ?? []).slice(0, 6).map((r) => (
                        <button
                          type="button"
                          key={r}
                          onClick={() => setProject(r)}
                          className={`rounded-md px-2 py-0.5 font-mono text-[11px] ${
                            project === r
                              ? "bg-brand-600 text-white"
                              : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300"
                          }`}
                        >
                          {prettyPath(r, home)}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </section>

            <section>
              <h3 className="mb-2 text-xs font-semibold tracking-wider text-zinc-400 uppercase">¿Para qué agentes?</h3>
              <div className="flex flex-wrap gap-2">
                {(status.data ?? []).map((a) => (
                  <label
                    key={a.agent}
                    className={`flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-sm ${
                      agents.includes(a.agent)
                        ? "border-brand-500 bg-brand-50 dark:bg-brand-500/10"
                        : "border-zinc-200 dark:border-zinc-700"
                    }`}
                  >
                    <input type="checkbox" className="accent-brand-600" checked={agents.includes(a.agent)} onChange={() => toggle(a.agent)} />
                    <AgentAvatar agent={a.agent} size="sm" />
                    {a.name}
                    {!a.present && <span className="text-xs text-zinc-400">(no instalado)</span>}
                  </label>
                ))}
              </div>
            </section>

            <section>
              <h3 className="mb-2 text-xs font-semibold tracking-wider text-zinc-400 uppercase">Vista previa</h3>
              {!ready && <p className="text-sm text-zinc-500">Elige {scope === "project" ? "la carpeta y " : ""}al menos un agente.</p>}
              {ready && preview.isLoading && (
                <p className="flex items-center gap-2 text-sm text-zinc-500">
                  <Loader2 className="size-4 animate-spin" /> Calculando…
                </p>
              )}
              {preview.error && <p className="text-sm text-red-600">{errorMessage(preview.error)}</p>}
              {p && (
                <div className="space-y-2">
                  {p.items.map((it) => (
                    <div key={it.path} className="rounded-lg border border-zinc-200 p-3 dark:border-zinc-700">
                      <div className="flex items-center gap-2">
                        <span className="min-w-0 flex-1 truncate font-mono text-xs">{prettyPath(it.path, home)}</span>
                        <Badge tone={ACTION[it.action].tone}>{ACTION[it.action].label}</Badge>
                      </div>
                      <div className="mt-1 text-xs text-zinc-500">
                        La leerá: {it.agents.map((a) => AGENT_NAMES[a]).join(", ")}
                      </div>
                      {it.action === "conflict" && (
                        <div className="mt-2 text-xs text-red-700 dark:text-red-400">
                          {REASON[it.reason ?? ""] ?? "conflicto"}
                          {it.diff && it.diff.files.length > 0 && (
                            <ul className="mt-1 font-mono text-[11px] text-zinc-500">
                              {it.diff.files.map((f) => (
                                <li key={f.path}>
                                  {f.status === "added" ? "+" : f.status === "removed" ? "−" : "~"} {f.path} (+{f.added} −{f.removed})
                                </li>
                              ))}
                            </ul>
                          )}
                        </div>
                      )}
                    </div>
                  ))}
                  {p.duplicated_for.length > 0 && (
                    <p className="flex gap-2 rounded-lg bg-amber-50 p-2.5 text-xs text-amber-800 dark:bg-amber-500/10 dark:text-amber-300">
                      <AlertTriangle className="size-4 shrink-0" />
                      {p.duplicated_for.join(", ")} verá esta skill dos veces: lee más de una de estas carpetas. Es inevitable si
                      instalas a la vez para Claude Code y Antigravity en un proyecto.
                    </p>
                  )}
                  {p.warnings.map((w, i) => (
                    <p key={i} className="text-xs text-amber-700 dark:text-amber-400">
                      ⚠ {w.message}
                    </p>
                  ))}
                  {p.conflicts && (
                    <label className="flex items-center gap-2 text-sm">
                      <input type="checkbox" className="accent-red-600" checked={force} onChange={(e) => setForce(e.target.checked)} />
                      Sobrescribir las copias en conflicto (se pierden sus cambios locales)
                    </label>
                  )}
                </div>
              )}
              {apply.error && <p className="mt-2 text-sm text-red-600">{errorMessage(apply.error)}</p>}
            </section>
          </div>
        )}

        {!done && (
          <footer className="flex justify-end gap-2 border-t border-zinc-200 px-5 py-3 dark:border-zinc-800">
            <Button variant="ghost" onClick={onClose}>
              Cancelar
            </Button>
            <Button variant="primary" onClick={() => apply.mutate()} disabled={blocked || apply.isPending}>
              {apply.isPending && <Loader2 className="size-4 animate-spin" />}
              {allDone ? "Ya está instalada" : "Instalar"}
            </Button>
          </footer>
        )}
      </div>
    </div>
  );
}
