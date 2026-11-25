// Biblioteca: todas las skills del banco con búsqueda, filtros y etiquetas.
import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useSearchParams } from "react-router";
import { ArrowDownAZ, Clock, Copy, Search, Terminal, X } from "lucide-react";
import { api, type SkillSummary } from "../lib/api";
import { plural, relativeTime } from "../lib/format";
import { useLibraryData } from "../lib/useLibraryData";
import { Badge, ErrorBox, Spinner, TagChip } from "../components/ui";

const FILTER_TITLES: Record<string, string> = {
  todas: "Todas las skills",
  desactualizadas: "Con copias desactualizadas",
  avisos: "Con avisos de validación",
  invalidas: "Inválidas",
  claude: "Con campos solo de Claude Code",
};

function SkillCard({
  s,
  installs,
  outdated,
  onTag,
  snippet,
}: {
  s: SkillSummary;
  installs: number;
  outdated: number;
  onTag: (t: string) => void;
  snippet?: string;
}) {
  return (
    <Link
      to={`/skill/${encodeURIComponent(s.name)}`}
      className="group flex flex-col rounded-xl border border-zinc-200 bg-white p-4 shadow-xs transition hover:-translate-y-0.5 hover:border-brand-500/50 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-brand-500/50"
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="truncate font-mono text-[15px] font-semibold text-zinc-900 group-hover:text-brand-700 dark:text-zinc-100 dark:group-hover:text-brand-100">
          {s.name}
        </h3>
        <span className="shrink-0 text-[11px] text-zinc-400" title={s.updated_at}>
          {relativeTime(s.updated_at)}
        </span>
      </div>
      <p className="mt-1.5 line-clamp-3 text-sm leading-relaxed text-zinc-600 dark:text-zinc-400">
        {s.description ?? <em className="text-zinc-400">Sin descripción</em>}
      </p>
      {snippet && <p className="mt-2 line-clamp-1 text-xs text-zinc-400 italic">«{snippet}»</p>}
      <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-3">
        {s.tags.map((t) => (
          <TagChip key={t} tag={t} onClick={() => onTag(t)} />
        ))}
        <span className="flex-1" />
        {!s.valid && <Badge tone="red">inválida</Badge>}
        {s.valid && s.warnings > 0 && <Badge tone="amber">{plural(s.warnings, "aviso", "avisos")}</Badge>}
        {s.claude_code_fields.length > 0 && (
          <Badge tone="violet" title={s.claude_code_fields.join(", ")}>
            solo Claude Code
          </Badge>
        )}
        {outdated > 0 && <Badge tone="amber">{plural(outdated, "desactualizada", "desactualizadas")}</Badge>}
        {installs > 0 && <Badge tone="neutral">{plural(installs, "instalación", "instalaciones")}</Badge>}
      </div>
    </Link>
  );
}

function EmptyBank() {
  const [copied, setCopied] = useState("");
  const copy = (t: string) => {
    navigator.clipboard.writeText(t);
    setCopied(t);
    setTimeout(() => setCopied(""), 1500);
  };
  const cmds = ["skillbank scan --global", "skillbank scan --global --import", "skillbank push .claude/skills/mi-skill"];
  return (
    <div className="mx-auto mt-16 max-w-xl text-center">
      <img src="/icon.png" alt="" className="mx-auto size-20 opacity-90" />
      <h2 className="mt-4 text-xl font-semibold">Tu banco está vacío</h2>
      <p className="mt-2 text-sm text-zinc-500">
        Trabaja con tu agente como siempre y, cuando una skill te guste, dile:
        <br />
        <span className="font-medium text-zinc-700 dark:text-zinc-300">“sube esta skill a mi banco de skills”</span>.
      </p>
      <div className="mt-6 space-y-2 text-left">
        <p className="text-xs font-semibold tracking-wider text-zinc-400 uppercase">O desde la terminal</p>
        {cmds.map((c) => (
          <button
            type="button"
            key={c}
            onClick={() => copy(c)}
            className="flex w-full items-center gap-2 rounded-lg border border-zinc-200 bg-white px-3 py-2 font-mono text-[13px] hover:border-brand-500/50 dark:border-zinc-800 dark:bg-zinc-900"
          >
            <Terminal className="size-4 text-zinc-400" />
            <span className="flex-1">{c}</span>
            <span className="text-xs text-zinc-400">{copied === c ? "¡Copiado!" : <Copy className="size-3.5" />}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

export default function LibraryPage() {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const filter = params.get("filtro") ?? "todas";
  const tag = params.get("tag");
  const [query, setQuery] = useState(params.get("q") ?? "");
  const [sort, setSort] = useState<"name" | "recent">("name");
  const inputRef = useRef<HTMLInputElement>(null);
  const { skills, installsBySkill, outdatedBySkill } = useLibraryData();

  // Ctrl+K o / enfocan la búsqueda.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey && e.key.toLowerCase() === "k") || (e.key === "/" && document.activeElement?.tagName !== "INPUT")) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Mantener la búsqueda en la URL (sin recargar).
  useEffect(() => {
    const t = setTimeout(() => {
      const p = new URLSearchParams(params);
      if (query.trim()) p.set("q", query.trim());
      else p.delete("q");
      if (p.toString() !== params.toString()) setParams(p, { replace: true });
    }, 200);
    return () => clearTimeout(t);
  }, [query]); // eslint-disable-line react-hooks/exhaustive-deps

  const q = (params.get("q") ?? "").trim();
  const search = useQuery({
    queryKey: ["bank", "search", q],
    queryFn: () => api.bankSearch(q),
    enabled: q.length > 0,
  });

  const visible = useMemo(() => {
    let list = skills.data ?? [];
    if (filter === "avisos") list = list.filter((s) => s.valid && s.warnings > 0);
    if (filter === "invalidas") list = list.filter((s) => !s.valid);
    if (filter === "claude") list = list.filter((s) => s.claude_code_fields.length > 0);
    if (filter === "desactualizadas") list = list.filter((s) => outdatedBySkill[s.name]);
    if (tag) list = list.filter((s) => s.tags.includes(tag));
    if (q && search.data) {
      const order = new Map(search.data.map((h, i) => [h.name, i]));
      list = list.filter((s) => order.has(s.name)).sort((a, b) => order.get(a.name)! - order.get(b.name)!);
    } else if (sort === "recent") {
      list = [...list].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    }
    return list;
  }, [skills.data, filter, tag, q, search.data, sort, outdatedBySkill]);

  const snippets = useMemo(() => new Map((search.data ?? []).map((h) => [h.name, h.snippet])), [search.data]);
  const setTag = (t: string) => navigate(`/?tag=${encodeURIComponent(t)}`);

  if (skills.isLoading) return <Spinner />;
  if (skills.error) return <ErrorBox error={skills.error} />;
  if ((skills.data ?? []).length === 0) return <EmptyBank />;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-3 border-b border-zinc-200 px-6 py-3 dark:border-zinc-800">
        <div className="relative max-w-xl flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-zinc-400" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar por nombre, etiqueta, descripción o contenido…   Ctrl+K"
            className="w-full rounded-lg border border-zinc-200 bg-white py-2 pr-9 pl-9 text-sm outline-none placeholder:text-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-zinc-800 dark:bg-zinc-900"
          />
          {query && (
            <button
              type="button"
              onClick={() => setQuery("")}
              className="absolute top-1/2 right-2 -translate-y-1/2 rounded p-1 text-zinc-400 hover:text-zinc-600"
              title="Limpiar"
            >
              <X className="size-4" />
            </button>
          )}
        </div>
        <div className="flex rounded-lg bg-zinc-100 p-0.5 dark:bg-zinc-900">
          {(
            [
              ["name", "Nombre", ArrowDownAZ],
              ["recent", "Recientes", Clock],
            ] as const
          ).map(([key, label, Icon]) => (
            <button
              type="button"
              key={key}
              onClick={() => setSort(key)}
              className={`flex items-center gap-1 rounded-md px-2.5 py-1 text-xs font-medium ${
                sort === key ? "bg-white shadow-xs dark:bg-zinc-800" : "text-zinc-500"
              }`}
            >
              <Icon className="size-3.5" /> {label}
            </button>
          ))}
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <div className="mb-4 flex items-baseline gap-2">
          <h1 className="text-lg font-semibold">{tag ? `#${tag}` : FILTER_TITLES[filter] ?? "Skills"}</h1>
          <span className="text-sm text-zinc-500">
            {plural(visible.length, "skill", "skills")}
            {q && ` para “${q}”`}
          </span>
        </div>
        {visible.length === 0 ? (
          <p className="mt-10 text-center text-sm text-zinc-500">Ninguna skill coincide.</p>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
            {visible.map((s) => (
              <SkillCard
                key={s.name}
                s={s}
                installs={installsBySkill[s.name] ?? 0}
                outdated={outdatedBySkill[s.name] ?? 0}
                onTag={setTag}
                snippet={q ? snippets.get(s.name) : undefined}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
