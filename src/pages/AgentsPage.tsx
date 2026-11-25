// Agentes: estado de conexión de Claude Code, Cursor y Antigravity con el banco, y su actividad.
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ask } from "@tauri-apps/plugin-dialog";
import { Check, Copy, Download, PlugZap, Unplug } from "lucide-react";
import { api, errorMessage, prettyPath, type AgentStatus } from "../lib/api";
import { AGENT_NAMES, COMMAND_VERBS, fullDate, relativeTime } from "../lib/format";
import { AgentAvatar, STATE_LABEL } from "../components/AgentAvatar";
import { Badge, Button, ErrorBox, Spinner } from "../components/ui";

const CONNECT_PHRASE = "conéctate con mi banco de skills";

const STATE_TONE = {
  connected: "green",
  ready: "sky",
  needs_guide: "amber",
  not_installed: "neutral",
} as const;

const GUIDE_LABEL: Record<string, string> = {
  current: "al día",
  outdated: "desactualizada",
  missing: "no instalada",
  foreign: "hay otra skill con su nombre",
};

function AgentCard({ a, home }: { a: AgentStatus; home?: string }) {
  const qc = useQueryClient();
  const [copied, setCopied] = useState(false);
  const [note, setNote] = useState("");
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["agents"] });
    qc.invalidateQueries({ queryKey: ["app"] });
  };
  const install = useMutation({
    mutationFn: () => api.agentInstallGuide(a.agent),
    onSuccess: () => {
      setNote("Guía instalada.");
      refresh();
    },
  });
  const disconnect = useMutation({
    mutationFn: () => api.agentDisconnect(a.agent),
    onSuccess: (r) => {
      setNote(
        r.still_shared_with.length > 0
          ? `Conexión olvidada. ${a.name} sigue viendo la guía porque la comparte con ${r.still_shared_with.join(", ")}.`
          : "Desconectado: se quitó la guía y se olvidó la conexión.",
      );
      refresh();
    },
  });

  const copyPhrase = () => {
    navigator.clipboard.writeText(CONNECT_PHRASE);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };
  const confirmDisconnect = async () => {
    const ok = await ask(
      `Se quitará la skill guía de ${a.name} y se olvidará su conexión. Tus skills y el banco no se tocan.`,
      { title: `¿Desconectar ${a.name}?`, kind: "warning", okLabel: "Desconectar", cancelLabel: "Cancelar" },
    );
    if (ok) disconnect.mutate();
  };

  const busy = install.isPending || disconnect.isPending;
  const error = install.error ?? disconnect.error;

  return (
    <div
      className={`flex flex-col rounded-xl border bg-white p-5 dark:bg-zinc-900 ${
        a.state === "connected" ? "border-emerald-300 dark:border-emerald-500/40" : "border-zinc-200 dark:border-zinc-800"
      }`}
    >
      <div className="flex items-center gap-3">
        <AgentAvatar agent={a.agent} state={a.state} size="lg" />
        <div className="min-w-0 flex-1">
          <div className="text-base font-semibold">{a.name}</div>
          <Badge tone={STATE_TONE[a.state]}>{STATE_LABEL[a.state]}</Badge>
        </div>
      </div>

      <dl className="mt-4 space-y-1.5 text-sm">
        <div className="flex gap-2">
          <dt className="w-24 shrink-0 text-zinc-500">Guía</dt>
          <dd>{GUIDE_LABEL[a.guide] ?? a.guide}</dd>
        </div>
        <div className="flex gap-2">
          <dt className="w-24 shrink-0 text-zinc-500">Global</dt>
          <dd className="selectable truncate font-mono text-xs leading-5">{prettyPath(a.global_dir, home)}</dd>
        </div>
        <div className="flex gap-2">
          <dt className="w-24 shrink-0 text-zinc-500">Proyecto</dt>
          <dd className="selectable font-mono text-xs leading-5">{a.project_dir}</dd>
        </div>
        {a.last_seen && (
          <div className="flex gap-2">
            <dt className="w-24 shrink-0 text-zinc-500">Actividad</dt>
            <dd title={fullDate(a.last_seen)}>
              {COMMAND_VERBS[a.last_command ?? ""] ?? a.last_command}
              {a.last_skill && a.last_command !== "connect" && <strong className="font-mono"> {a.last_skill}</strong>}
              {" · "}
              {relativeTime(a.last_seen)}
              {a.last_project && (
                <div className="truncate text-xs text-zinc-400">en {prettyPath(a.last_project, home)}</div>
              )}
            </dd>
          </div>
        )}
        {a.connected_at && (
          <div className="flex gap-2">
            <dt className="w-24 shrink-0 text-zinc-500">Conectado</dt>
            <dd>desde {fullDate(a.connected_at)}</dd>
          </div>
        )}
      </dl>

      {a.state === "not_installed" && (
        <p className="mt-4 rounded-lg bg-zinc-50 p-3 text-xs text-zinc-500 dark:bg-zinc-950">
          No se detectó {a.name} en esta computadora. Cuando lo instales, aparecerá aquí.
        </p>
      )}
      {(a.state === "ready" || a.state === "needs_guide") && (
        <ol className="mt-4 list-decimal space-y-1 rounded-lg bg-zinc-50 p-3 pl-7 text-xs text-zinc-600 dark:bg-zinc-950 dark:text-zinc-400">
          {a.state === "needs_guide" && <li>Instala la guía con el botón de abajo.</li>}
          <li>
            Abre {a.name} y dile: <strong>“{CONNECT_PHRASE}”</strong>.
          </li>
          <li>Este ícono se pondrá en verde en cuanto responda.</li>
        </ol>
      )}

      {note && <p className="mt-3 text-xs text-emerald-700 dark:text-emerald-400">{note}</p>}
      {error && <p className="mt-3 text-xs text-red-600">{errorMessage(error)}</p>}

      <div className="mt-auto flex flex-wrap gap-2 pt-4">
        {a.present && a.guide !== "current" && (
          <Button variant="primary" onClick={() => install.mutate()} disabled={busy}>
            <Download className="size-4" /> {a.guide === "missing" ? "Instalar guía" : "Actualizar guía"}
          </Button>
        )}
        {a.present && (
          <Button onClick={copyPhrase} title="Pégala en el chat de tu agente">
            {copied ? <Check className="size-4 text-emerald-500" /> : <Copy className="size-4" />}
            {copied ? "¡Copiada!" : "Copiar frase"}
          </Button>
        )}
        {(a.state === "connected" || a.guide === "current" || a.guide === "outdated") && (
          <Button variant="ghost" onClick={confirmDisconnect} disabled={busy}>
            <Unplug className="size-4" /> Desconectar
          </Button>
        )}
      </div>
    </div>
  );
}

export default function AgentsPage() {
  const agents = useQuery({ queryKey: ["agents"], queryFn: api.agentsStatus });
  const activity = useQuery({ queryKey: ["activity"], queryFn: () => api.activityList(50) });
  const app = useQuery({ queryKey: ["app"], queryFn: api.appInfo });
  const home = app.data?.home;

  if (agents.isLoading) return <Spinner />;
  if (agents.error) return <ErrorBox error={agents.error} />;
  const list = agents.data ?? [];
  const connected = list.filter((a) => a.state === "connected").length;

  return (
    <div className="h-full overflow-y-auto px-6 py-6">
      <div className="flex items-center gap-2">
        <PlugZap className="size-5 text-brand-600" />
        <h1 className="text-lg font-semibold">Agentes</h1>
        <span className="text-sm text-zinc-500">
          {connected} de {list.length} conectados
        </span>
      </div>
      <p className="mt-1 max-w-3xl text-sm text-zinc-500">
        Tus agentes se conectan con el banco a través del CLI <code>skillbank</code>. Un agente aparece
        conectado cuando saluda (“{CONNECT_PHRASE}”) o cuando sube o trae una skill.
      </p>

      <div className="mt-5 grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-4">
        {list.map((a) => (
          <AgentCard key={a.agent} a={a} home={home} />
        ))}
      </div>

      <h2 className="mt-8 mb-3 text-xs font-semibold tracking-wider text-zinc-400 uppercase">Actividad reciente</h2>
      {(activity.data ?? []).length === 0 ? (
        <p className="text-sm text-zinc-500">Todavía no hay actividad de ningún agente.</p>
      ) : (
        <ul className="divide-y divide-zinc-100 rounded-xl border border-zinc-200 bg-white dark:divide-zinc-800 dark:border-zinc-800 dark:bg-zinc-900">
          {(activity.data ?? []).map((e, i) => (
            <li key={i} className="flex items-center gap-3 px-4 py-2.5 text-sm">
              <AgentAvatar agent={e.agent} size="sm" />
              <div className="min-w-0 flex-1">
                <span className="font-medium">{AGENT_NAMES[e.agent]}</span>{" "}
                {COMMAND_VERBS[e.command] ?? e.command}
                {e.skill && e.command !== "connect" && <strong className="font-mono"> {e.skill}</strong>}
                {!e.ok && (
                  <span className="ml-2">
                    <Badge tone="red">falló</Badge>
                  </span>
                )}
                {e.project && <div className="truncate text-xs text-zinc-400">{prettyPath(e.project, home)}</div>}
              </div>
              <span className="shrink-0 text-xs text-zinc-400" title={fullDate(e.at)}>
                {relativeTime(e.at)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
