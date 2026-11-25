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
  agent: AgentKey;
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

export type AgentKey = "claude" | "cursor" | "antigravity";
export type ConnState = "not_installed" | "needs_guide" | "ready" | "connected";

export interface AgentStatus {
  agent: AgentKey;
  name: string;
  present: boolean;
  guide: "current" | "outdated" | "missing" | "foreign";
  state: ConnState;
  connected_at?: string;
  last_seen?: string;
  last_command?: string;
  last_skill?: string;
  last_project?: string;
  count: number;
  project_dir: string;
  global_dir: string;
}

export interface ActivityEvent {
  at: string;
  agent: AgentKey;
  command: string;
  skill?: string;
  project?: string;
  ok: boolean;
}

export interface Destination {
  dir: string;
  agents: AgentKey[];
}

export interface PullItem {
  skill: string;
  path: string;
  agents: AgentKey[];
  action: "installed" | "updated" | "unchanged" | "conflict";
  reason?: "symlink" | "modified" | "unregistered" | "not_a_dir" | "forced";
  diff?: { files: { path: string; status: string; added: number; removed: number }[] };
}

export interface PullOutcome {
  scope: "project" | "global";
  project_root?: string;
  agents: AgentKey[];
  destinations: Destination[];
  items: PullItem[];
  warnings: Diagnostic[];
  dry_run: boolean;
}

export interface InstallPreview extends PullOutcome {
  conflicts: boolean;
  duplicated_for: string[];
}

export interface GuideResult {
  dir: string;
  agents: AgentKey[];
  action: string;
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
  agentsStatus: () => invoke<AgentStatus[]>("agents_status"),
  activityList: (limit = 50) => invoke<ActivityEvent[]>("activity_list", { limit }),
  agentInstallGuide: (agent: AgentKey) => invoke<GuideResult[]>("agent_install_guide", { agent }),
  agentDisconnect: (agent: AgentKey) =>
    invoke<{ removed: GuideResult[]; forgotten: boolean; still_shared_with: string[] }>("agent_disconnect", { agent }),
  installPreview: (name: string, scope: "global" | "project", project: string | null, agents: AgentKey[]) =>
    invoke<InstallPreview>("install_preview", { name, scope, project, agents }),
  installApply: (name: string, scope: "global" | "project", project: string | null, agents: AgentKey[], force: boolean) =>
    invoke<PullOutcome>("install_apply", { name, scope, project, agents, force }),
  recentProjects: () => invoke<string[]>("recent_projects"),
};

/** Abrevia una ruta reemplazando el HOME por ~. */
export function prettyPath(path: string, home?: string): string {
  if (home && path.startsWith(home)) return "~" + path.slice(home.length);
  return path;
}
