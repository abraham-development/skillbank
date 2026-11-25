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
        default:
          throw { kind: "ERROR", message: `Comando no simulado: ${cmd}` };
      }
    },
    { shouldMockEvents: true },
  );
}
