// Detalle de una skill: vista renderizada, archivos, instalaciones y validación.
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArrowLeft, Check, FileText, FolderOpen, Terminal } from "lucide-react";
import { api, prettyPath, type SkillDetail } from "../lib/api";
import { AGENT_NAMES, fileSize, fullDate, plural, relativeTime, shortHash } from "../lib/format";
import { Badge, Button, DiagnosticsList, ErrorBox, Spinner, StatusBadge, TagChip } from "../components/ui";

type Tab = "vista" | "archivos" | "instalaciones" | "validacion";

const STANDARD = new Set(["name", "description", "license", "compatibility", "metadata", "allowed-tools"]);

function FrontmatterTable({ fm }: { fm: Record<string, unknown> | null }) {
  if (!fm) return null;
  return (
    <table className="w-full text-sm">
      <tbody>
        {Object.entries(fm).map(([k, v]) => (
          <tr key={k} className="border-b border-zinc-100 last:border-0 dark:border-zinc-800">
            <td className="w-44 py-2 pr-4 align-top font-mono text-xs text-zinc-500">
              {k}
              {!STANDARD.has(k) && (
                <div className="mt-1">
                  <Badge tone="violet">solo Claude Code</Badge>
                </div>
              )}
            </td>
            <td className="selectable py-2 align-top whitespace-pre-wrap">
              {typeof v === "string" ? v : <code className="text-xs">{JSON.stringify(v, null, 2)}</code>}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function FilesTab({ d }: { d: SkillDetail }) {
  const [selected, setSelected] = useState("SKILL.md");
  const file = useQuery({
    queryKey: ["skill", d.name, "file", selected, d.content_hash],
    queryFn: () => api.skillReadFile(d.name, selected),
  });
  return (
    <div className="flex min-h-[420px] gap-4">
      <ul className="w-64 shrink-0 space-y-0.5">
        {d.files.map((f) => (
          <li key={f.path}>
            <button
              type="button"
              onClick={() => setSelected(f.path)}
              className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm ${
                selected === f.path
                  ? "bg-brand-600/10 text-brand-700 dark:bg-brand-500/20 dark:text-brand-100"
                  : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
              }`}
            >
              <FileText className="size-3.5 shrink-0 text-zinc-400" />
              <span className="flex-1 truncate font-mono text-xs">{f.path}</span>
              {f.executable && <span title="Ejecutable" className="text-[10px] text-emerald-600">exec</span>}
              <span className="text-[10px] text-zinc-400">{fileSize(f.size)}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="min-w-0 flex-1 overflow-hidden rounded-lg border border-zinc-200 dark:border-zinc-800">
        {file.isLoading && <Spinner />}
        {file.error && <ErrorBox error={file.error} />}
        {file.data &&
          (file.data.binary ? (
            <p className="p-6 text-sm text-zinc-500">Archivo binario o muy grande ({fileSize(file.data.size)}); no se muestra.</p>
          ) : (
            <pre className="h-full max-h-[70vh] overflow-auto bg-zinc-50 p-4 font-mono text-[12.5px] leading-5 dark:bg-zinc-950">
              {file.data.text}
            </pre>
          ))}
      </div>
    </div>
  );
}

function InstallsTab({ d, home }: { d: SkillDetail; home?: string }) {
  if (d.installs.length === 0) {
    return (
      <div className="text-sm text-zinc-500">
        <p>Esta skill todavía no está instalada en ningún proyecto registrado.</p>
        <p className="mt-2">
          Pídele a tu agente “trae la skill <strong>{d.name}</strong> de mi banco”, o usa{" "}
          <code className="rounded bg-zinc-100 px-1.5 py-0.5 dark:bg-zinc-800">{d.pull_command}</code>.
        </p>
      </div>
    );
  }
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-zinc-200 text-left text-xs text-zinc-500 dark:border-zinc-800">
          <th className="py-2 font-medium">Ubicación</th>
          <th className="py-2 font-medium">Agentes</th>
          <th className="py-2 font-medium">Estado</th>
          <th className="py-2" />
        </tr>
      </thead>
      <tbody>
        {d.installs.map((i) => (
          <tr key={i.id} className="border-b border-zinc-100 last:border-0 dark:border-zinc-800">
            <td className="py-2.5 pr-4">
              <div className="selectable font-mono text-xs">{prettyPath(i.path, home)}</div>
              <div className="text-[11px] text-zinc-400">
                {i.scope === "global" ? "global" : "proyecto"} · {relativeTime(i.installed_at)}
              </div>
            </td>
            <td className="py-2.5 pr-4 text-xs">{i.agents.map((a) => AGENT_NAMES[a] ?? a).join(", ")}</td>
            <td className="py-2.5 pr-4">
              <StatusBadge status={i.status} label={i.status_es} />
            </td>
            <td className="py-2.5 text-right">
              {i.status !== "missing" && (
                <button
                  type="button"
                  title="Abrir carpeta"
                  onClick={() => api.openPath(i.path)}
                  className="rounded p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800"
                >
                  <FolderOpen className="size-4" />
                </button>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default function SkillPage() {
  const { name = "" } = useParams();
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>("vista");
  const [copied, setCopied] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const detail = useQuery({ queryKey: ["skill", name], queryFn: () => api.skillDetail(name) });
  const app = useQuery({ queryKey: ["app"], queryFn: api.appInfo });

  if (detail.isLoading) return <Spinner />;
  if (detail.error) {
    return (
      <div>
        <div className="px-6 pt-4">
          <Link to="/" className="inline-flex items-center gap-1 text-sm text-zinc-500 hover:text-zinc-800">
            <ArrowLeft className="size-4" /> Biblioteca
          </Link>
        </div>
        <ErrorBox error={detail.error} />
      </div>
    );
  }
  const d = detail.data!;
  const errors = d.diagnostics.filter((x) => x.severity === "error").length;
  const warnings = d.diagnostics.filter((x) => x.severity === "warning").length;
  const outdated = d.installs.filter((i) => i.status === "outdated").length;

  const copyPull = () => {
    navigator.clipboard.writeText(d.pull_command);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const tabs: [Tab, string, number?][] = [
    ["vista", "Vista"],
    ["archivos", "Archivos", d.files.length],
    ["instalaciones", "Instalaciones", d.installs.length],
    ["validacion", "Validación", errors + warnings],
  ];

  return (
    <div className="flex h-full flex-col">
      <header className="border-b border-zinc-200 px-6 pt-4 dark:border-zinc-800">
        <button
          type="button"
          onClick={() => navigate(-1)}
          className="inline-flex items-center gap-1 text-sm text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
        >
          <ArrowLeft className="size-4" /> Volver
        </button>
        <div className="mt-3 flex items-start gap-4">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="selectable font-mono text-2xl font-semibold tracking-tight">{d.name}</h1>
              {!d.valid && <Badge tone="red">inválida</Badge>}
              {d.valid && warnings > 0 && <Badge tone="amber">{plural(warnings, "aviso", "avisos")}</Badge>}
              {d.claude_code_fields.length > 0 && <Badge tone="violet">solo Claude Code</Badge>}
              {outdated > 0 && <Badge tone="amber">{plural(outdated, "copia desactualizada", "copias desactualizadas")}</Badge>}
            </div>
            <p
              className={`selectable mt-1.5 max-w-3xl text-sm leading-relaxed text-zinc-600 dark:text-zinc-400 ${
                expanded ? "max-h-48 overflow-y-auto" : "line-clamp-3"
              }`}
            >
              {d.description}
            </p>
            {(d.description?.length ?? 0) > 260 && (
              <button
                type="button"
                onClick={() => setExpanded(!expanded)}
                className="mt-0.5 text-xs font-medium text-brand-600 hover:underline dark:text-brand-500"
              >
                {expanded ? "Ver menos" : "Ver más"}
              </button>
            )}
            <div className="mt-2.5 flex flex-wrap items-center gap-1.5 text-xs text-zinc-500">
              {d.tags.map((t) => (
                <TagChip key={t} tag={t} onClick={() => navigate(`/?tag=${encodeURIComponent(t)}`)} />
              ))}
              <span title={fullDate(d.updated_at)}>Actualizada {relativeTime(d.updated_at)}</span>
              <span>·</span>
              <span className="font-mono" title={d.content_hash}>
                {shortHash(d.content_hash)}
              </span>
              {d.meta.source?.agent && (
                <>
                  <span>·</span>
                  <span>subida desde {AGENT_NAMES[d.meta.source.agent] ?? d.meta.source.agent}</span>
                </>
              )}
            </div>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button onClick={copyPull} title="Copia el comando para traerla a un proyecto">
              {copied ? <Check className="size-4 text-emerald-500" /> : <Terminal className="size-4" />}
              {copied ? "¡Copiado!" : "Copiar comando pull"}
            </Button>
            <Button onClick={() => api.openPath(d.path)}>
              <FolderOpen className="size-4" /> Abrir carpeta
            </Button>
          </div>
        </div>
        <nav className="mt-4 flex gap-1">
          {tabs.map(([key, label, count]) => (
            <button
              type="button"
              key={key}
              onClick={() => setTab(key)}
              className={`-mb-px border-b-2 px-3 pb-2.5 text-sm font-medium transition ${
                tab === key
                  ? "border-brand-600 text-brand-700 dark:text-brand-100"
                  : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
              }`}
            >
              {label}
              {count !== undefined && count > 0 && <span className="ml-1.5 text-xs text-zinc-400">{count}</span>}
            </button>
          ))}
        </nav>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
        {tab === "vista" && (
          <div className="mx-auto max-w-4xl space-y-6">
            <section className="rounded-xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
              <h2 className="mb-2 text-xs font-semibold tracking-wider text-zinc-400 uppercase">Frontmatter</h2>
              <FrontmatterTable fm={d.frontmatter} />
            </section>
            <article className="prose-skill rounded-xl border border-zinc-200 bg-white p-6 dark:border-zinc-800 dark:bg-zinc-900">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{d.body ?? ""}</ReactMarkdown>
            </article>
          </div>
        )}
        {tab === "archivos" && <FilesTab d={d} />}
        {tab === "instalaciones" && <InstallsTab d={d} home={app.data?.home} />}
        {tab === "validacion" && (
          <div className="max-w-3xl">
            <DiagnosticsList diagnostics={d.diagnostics} />
          </div>
        )}
      </div>
    </div>
  );
}
