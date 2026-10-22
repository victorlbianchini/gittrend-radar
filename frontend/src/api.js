import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ponytail: sem mock de proposito. Erro real do backend tem que aparecer na tela, nunca virar "sucesso" falso.
export const api = {
  catalog: (request) => invoke('get_catalog', { request }),
  summaryRecords: (keys) => invoke('get_summary_records', { keys }),
  summaryStatus: () => invoke('get_summary_status'),
  retrySummaries: (keys) => invoke('retry_summaries', { keys }),
  queueRepoSummaries: (repoIds) => invoke('queue_repo_summaries', { repoIds }),
  status: () => invoke("get_status"),
  sync: (force = true) => invoke("start_sync", { force }),
  trends: () => invoke("get_trends"),
  updateRepos: () => invoke("update_downloaded_repos"),
  stackAnalysis: () => invoke("analyze_local_stacks"),
  requestStackDownload: (kind, id, reason) => invoke("request_stack_download", { kind, id, reason }),
  fixLfs: () => invoke("fix_lfs"),
  agentRequests: () => invoke("list_agent_requests"),
  resolveAgentRequest: (id, approve) => invoke("resolve_agent_request", { id, approve }),
  library: () => invoke("get_library"),
  toggleFavorite: (repoId) => invoke("toggle_favorite", { repoId }),
  hfModels: (force = false) => invoke("get_hf_models", { force }),
  githubSearch: (query, sort = 'stars', page = 1) => invoke('search_github', { query, sort, page }),
  explainRepo: (repoId) => invoke('explain_repo', { repoId }),
  hfSearch: (query, cursor = null) => invoke('search_hf', { query, cursor }),
  explainModel: (model) => invoke('explain_model', { model }),
  modelFiles: (id) => invoke('model_files', { id }),
  downloadModel: (id, full = false) => invoke('download_model', { id, full }),
  skillsSearch: (query = '') => invoke('search_skills', { query }),
  explainSkill: (skill) => invoke('explain_skill', { skill }),
  downloadSkill: (skill) => invoke('download_skill', { skill }),
  downloads: () => invoke('get_downloads'),
  settings: () => invoke('get_settings'),
  saveSettings: (settings) => invoke('save_settings', settings),
  pickFolder: () => invoke('pick_folder'),
  categories: () => invoke("get_categories"),
  cloneRepo: (url, name, customName) =>
    invoke("clone_repo", { url, name, customName }),
  openFolder: (folderPath) => invoke("open_folder", { folderPath }),
  aiModels: () => invoke("ai_models"),
  analyzeOllama: (repoId) => invoke("analyze_ollama", { repoId }),
  setTitle: (title) =>
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {}),
};

// Links http(s) abrem no navegador padrao em vez de navegar dentro da janela do app
document.addEventListener("click", (e) => {
  const a = e.target.closest?.('a[href^="http"]');
  if (!a) return;
  e.preventDefault();
  invoke("open_url", { url: a.href }).catch((error) => {
    document.dispatchEvent(new CustomEvent('backend-error', { detail: String(error) }));
  });
});
