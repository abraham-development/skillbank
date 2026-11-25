// Formatos en español.

const rtf = new Intl.RelativeTimeFormat("es", { numeric: "auto" });

export function relativeTime(iso: string): string {
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  const abs = Math.abs(diff);
  if (abs < 60) return "hace un momento";
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  if (abs < 86400 * 30) return rtf.format(Math.round(diff / 86400), "day");
  return new Date(iso).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" });
}

export function fullDate(iso: string): string {
  return new Date(iso).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" });
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function shortHash(hash: string): string {
  return hash.replace(/^sha256:/, "").slice(0, 12);
}

export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

export const AGENT_NAMES: Record<string, string> = {
  claude: "Claude Code",
  cursor: "Cursor",
  antigravity: "Antigravity",
};

/** Verbo en pasado para el historial ("Claude Code subió pdf-tools"). */
export const COMMAND_VERBS: Record<string, string> = {
  connect: "se conectó",
  push: "subió",
  pull: "trajo",
  list: "listó el banco",
  search: "buscó en el banco",
  show: "consultó",
  path: "consultó la ruta de",
  validate: "validó",
  remove: "eliminó",
  tag: "etiquetó",
  rename: "renombró",
  new: "creó",
  export: "exportó",
  import: "importó",
  installed: "consultó las instalaciones",
  outdated: "revisó actualizaciones",
  scan: "escaneó sus skills",
  doctor: "ejecutó el diagnóstico",
  agents: "revisó los agentes",
};
