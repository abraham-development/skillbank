// Vista previa en el navegador (solo `pnpm dev` fuera de Tauri): simula los comandos del
// backend con datos de ejemplo generados con el CLI real (fixtures.json). No se incluye en
// la app compilada: main.tsx solo lo importa en desarrollo y sin Tauri.
import { mockIPC } from "@tauri-apps/api/mocks";
import fixtures from "./fixtures.json";

type Fixtures = typeof fixtures & { details: Record<string, Record<string, unknown> & { _files: Record<string, string | null> }> };
const fx = fixtures as unknown as Fixtures;

function fold(s: string) {
  return s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

// Estado simulado de los agentes (se reinicia al recargar).
const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();
const agentState: Record<string, Record<string, unknown>> = {
  claude: {
    agent: "claude", name: "Claude Code", present: true, guide: "current", state: "connected",
    connected_at: minutesAgo(60 * 26), last_seen: minutesAgo(3), last_command: "push", last_skill: "revisar-prs",
    last_project: "/home/usuario/Proyectos/web-app", count: 14,
    project_dir: ".claude/skills", global_dir: "/home/usuario/.claude/skills",
  },
  cursor: {
    agent: "cursor", name: "Cursor", present: true, guide: "current", state: "ready", count: 0,
    project_dir: ".cursor/skills", global_dir: "/home/usuario/.cursor/skills",
  },
  antigravity: {
    agent: "antigravity", name: "Antigravity", present: false, guide: "missing", state: "not_installed", count: 0,
    project_dir: ".agents/skills", global_dir: "/home/usuario/.gemini/config/skills",
  },
};
const activity = [
  { at: minutesAgo(3), agent: "claude", command: "push", skill: "revisar-prs", project: "/home/usuario/Proyectos/web-app", ok: true },
  { at: minutesAgo(40), agent: "claude", command: "pull", skill: "extraer-pdf", project: "/home/usuario/Proyectos/api", ok: true },
  { at: minutesAgo(41), agent: "claude", command: "search", project: "/home/usuario/Proyectos/api", ok: true },
  { at: minutesAgo(60 * 26), agent: "claude", command: "connect", project: "/home/usuario/Proyectos/web-app", ok: true },
];

const PRIMARY: Record<string, { project: string; global: string; readsProject: string[]; readsGlobal: string[] }> = {
  claude: { project: ".claude/skills", global: "~/.claude/skills", readsProject: [], readsGlobal: [] },
  cursor: {
    project: ".cursor/skills", global: "~/.cursor/skills",
    readsProject: [".agents/skills", ".claude/skills"], readsGlobal: ["~/.claude/skills"],
  },
  antigravity: { project: ".agents/skills", global: "~/.gemini/config/skills", readsProject: [], readsGlobal: [] },
};

function simulatePreview(name: string, scope: string, project: string | null, agents: string[]) {
  const base = scope === "global" ? "/home/usuario" : project ?? "";
  const reads = (a: string) =>
    scope === "global" ? [PRIMARY[a].global, ...PRIMARY[a].readsGlobal] : [PRIMARY[a].project, ...PRIMARY[a].readsProject];
  const candidates = [...new Set(agents.map((a) => (scope === "global" ? PRIMARY[a].global : PRIMARY[a].project)))];
  // Igual que plan_destinations del núcleo: el menor subconjunto de carpetas que cubra a todos.
  let dirs = candidates;
  search: for (let size = 1; size <= candidates.length; size++) {
    for (let mask = 0; mask < 1 << candidates.length; mask++) {
      const pick = candidates.filter((_, i) => mask & (1 << i));
      if (pick.length !== size) continue;
      if (agents.every((a) => pick.some((d) => reads(a).includes(d)))) {
        dirs = pick;
        break search;
      }
    }
  }
  const destinations = dirs.map((d) => ({
    dir: `${base}/${d.replace(/^~\//, "")}`,
    agents: agents.filter((a) => reads(a).includes(d)),
  }));
  const existing = fx.installs.filter((i) => i.skill === name).map((i) => i.path);
  const items = destinations.map((d) => {
    const path = `${d.dir}/${name}`;
    return { skill: name, path, agents: d.agents, action: existing.includes(path) ? "unchanged" : "installed", bank_hash: "" };
  });
  const duplicated_for = agents
    .filter((a) => destinations.filter((d) => d.agents.includes(a as never)).length > 1)
    .map((a) => (agentState[a].name as string));
  return { scope, project_root: project, agents, agent_source: "explicit", destinations, items, warnings: [], dry_run: true, conflicts: false, duplicated_for };
}

export function installMock() {
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, string | null>;
      switch (cmd) {
        case "app_info":
          return fx.app;
        case "bank_list":
          return fx.list;
        case "bank_tags":
          return fx.tags;
        case "installs_list":
          return a.skill ? fx.installs.filter((i) => i.skill === a.skill) : fx.installs;
        case "bank_search": {
          const terms = fold(a.query ?? "").split(/\s+/).filter(Boolean);
          return fx.list
            .filter((s) => {
              const d = fx.details[s.name];
              const hay = fold(`${s.name} ${s.description ?? ""} ${s.tags.join(" ")} ${String(d?.body ?? "")}`);
              return terms.every((t) => hay.includes(t));
            })
            .map((s) => ({ name: s.name, description: s.description, tags: s.tags, score: 1, matched_in: ["description"] }));
        }
        case "skill_detail": {
          const d = fx.details[a.name ?? ""];
          if (!d) throw { kind: "NOT_FOUND", message: `La skill '${a.name}' no está en el banco.` };
          return d;
        }
        case "skill_read_file": {
          const text = fx.details[a.name ?? ""]?._files[a.path ?? ""] ?? null;
          return { path: a.path, size: text?.length ?? 0, text, binary: text === null };
        }
        case "open_path":
          console.info("[vista previa] abrir carpeta:", a.path);
          return null;
        case "agents_status":
          return Object.values(agentState);
        case "activity_list":
          return activity;
        case "agent_install_guide":
          Object.assign(agentState[a.agent ?? ""], { guide: "current", state: "ready" });
          return [];
        case "agent_disconnect":
          Object.assign(agentState[a.agent ?? ""], { guide: "missing", state: "needs_guide", connected_at: undefined, last_seen: undefined });
          return { removed: [], forgotten: true, still_shared_with: [] };
        case "recent_projects":
          return ["/home/usuario/Proyectos/web-app", "/home/usuario/Proyectos/api"];
        case "install_preview": {
          const b = args as unknown as { name: string; scope: string; project: string | null; agents: string[] };
          return simulatePreview(b.name, b.scope, b.project, b.agents);
        }
        case "install_apply": {
          const b = args as unknown as { name: string; scope: string; project: string | null; agents: string[] };
          return { ...simulatePreview(b.name, b.scope, b.project, b.agents), dry_run: false };
        }
        case "plugin:dialog|open":
          return "/home/usuario/Proyectos/nuevo-proyecto";
        case "plugin:dialog|ask":
          return true;
        default:
          throw { kind: "ERROR", message: `Comando no simulado: ${cmd}` };
      }
    },
    { shouldMockEvents: true },
  );
}
