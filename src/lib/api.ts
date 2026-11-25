// Tipos y llamadas a los comandos de Tauri (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";

export type Severity = "error" | "warning" | "info";

export interface Diagnostic {
  code: string;
  severity: Severity;
  message: string;
  file: string;
  line?: number;
  field?: string;
  tag?: string;
  hint?: string;
}

export interface SkillSummary {
  name: string;
  description: string | null;
  tags: string[];
  created_at: string;
  updated_at: string;
  content_hash: string;
  valid: boolean;
  errors: number;
  warnings: number;
  claude_code_fields: string[];
  path: string;
}

export interface FileInfo {
  path: string;
  size: number;
  executable: boolean;
}

export type InstallStatus = "up_to_date" | "outdated" | "modified" | "diverged" | "missing" | "orphan";

export interface InstallView {
  id: string;
  skill: string;
  scope: "project" | "global";
  agents: string[];
  project_root?: string;
  path: string;
  installed_hash: string;
  installed_at: string;
  origin: string;
  status: InstallStatus;
  status_es: string;
}

export interface SkillMeta {
  tags: string[];
  source?: { kind: string; path?: string; agent?: string; host?: string };
  notes: string;
}

export interface SkillDetail extends SkillSummary {
  frontmatter: Record<string, unknown> | null;
  body?: string;
  files: FileInfo[];
  diagnostics: Diagnostic[];
  meta: SkillMeta;
  installs: InstallView[];
  pull_command: string;
}

export interface SearchHit {
  name: string;
  description: string | null;
  tags: string[];
  score: number;
  matched_in: string[];
  snippet?: string;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface GuideState {
  agent: string;
  name: string;
  present: boolean;
  state: "current" | "outdated" | "missing" | "foreign";
}

export interface AppInfo {
  version: string;
  bank_path: string;
  config_path: string;
  home: string;
  skills: number;
  cli_on_path: string | null;
  guide: GuideState[];
}

/** Error que devuelve el núcleo (CoreError serializado). */
export interface CoreError {
  kind: string;
  message: string;
  hint?: string;
  data?: unknown;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CoreError).message);
  return String(e);
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  bankList: () => invoke<SkillSummary[]>("bank_list"),
  bankTags: () => invoke<TagCount[]>("bank_tags"),
  bankSearch: (query: string, tag?: string) => invoke<SearchHit[]>("bank_search", { query, tag: tag ?? null }),
  skillDetail: (name: string) => invoke<SkillDetail>("skill_detail", { name }),
  skillReadFile: (name: string, path: string) =>
    invoke<{ path: string; size: number; text: string | null; binary: boolean }>("skill_read_file", { name, path }),
  installsList: (skill?: string) => invoke<InstallView[]>("installs_list", { skill: skill ?? null }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
};

/** Abrevia una ruta reemplazando el HOME por ~. */
export function prettyPath(path: string, home?: string): string {
  if (home && path.startsWith(home)) return "~" + path.slice(home.length);
  return path;
}
