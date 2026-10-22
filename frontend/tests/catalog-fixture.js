import React from 'react';
import { createRoot } from 'react-dom/client';
import '../src/index.css';

// This fixture is an explicit browser-only test entry point. Production entry
// points do not import it. Unknown/native commands fail rather than simulating
// successful side effects.
const calls = [];
const records = new Map();
const polls = new Map();
let activeKeys = [];
const log = document.querySelector('[data-testid="fixture-request-log"]');
function track(command, args) {
  const entry = command === 'get_catalog' ? { command, request: args.request } : { command };
  calls.push(entry);
  log.textContent = calls.slice(-12).map((call) => JSON.stringify(call)).join('\n');
}

function catalog(request) {
  const { domain, mode, source, page = 1, query = '', category = 'all', cursor = null } = request;
  if (!['github', 'models', 'skills'].includes(domain)) throw new Error('Fixture: domínio desconhecido');
  if (!['tops', 'news', 'explore'].includes(mode)) throw new Error('Fixture: seção desconhecida');
  if (domain === 'github' && source !== 'github') throw new Error('Fixture: fonte GitHub incorreta');
  if (domain === 'models' && !['hf', 'github'].includes(source)) throw new Error('Fixture: fonte de modelos incorreta');
  if (domain === 'skills' && source !== 'all') throw new Error('Fixture: fonte de skills incorreta');
  if (domain === 'models' && source === 'hf' && page > 1 && cursor !== 'fixture-next') throw new Error('Fixture: cursor HF não encaminhado');
  const items = [1, 2].map((index) => {
    const suffix = `${domain}-${source}-${mode}-p${page}-${index}`;
    const title = `Fixture ${suffix}${query ? ` busca ${query}` : ''}`;
    const repoId = `fixture/${suffix}`;
    const kind = domain === 'skills' ? 'skill' : domain === 'models' && source === 'hf' ? 'hf' : 'github';
    const key = `${kind}:${repoId}`;
    const summary = { key, status: 'pending', description_pt: '', summary: '', model: 'test-model', error: '' };
    records.set(key, summary);
    polls.set(key, 0);
    const repo = kind === 'github' ? {
      repo_id: repoId, owner: 'fixture', name: title, url: `https://github.com/${repoId}`,
      description: 'Synthetic English repository description.', language: 'Python', stars: 1234, stars_today: mode === 'tops' ? 12 : 0,
      forks: 23, category: 'ia_agentes', first_seen: null, is_downloaded: false, downloaded_path: null,
      is_favorite: false, has_analysis: false, simple_summary: '', why_trending: '',
    } : null;
    const model = kind === 'hf' ? {
      id: repoId, name: title, author: 'fixture', url: `https://huggingface.co/${repoId}`,
      task_label: 'Texto e LLMs', group: 'texto', likes: 34, downloads: 567, trending_score: 78,
      created_at: '2026-10-15', is_new: mode === 'news', description: 'Synthetic English model description.',
    } : null;
    const skill = kind === 'skill' ? {
      id: `${repoId}::example-skill`, name: title, repo: repoId, path: 'skills/example-skill',
      description: 'Synthetic English skill description.', github_url: `https://github.com/${repoId}/tree/main/skills/example-skill`,
      sources: ['Fixture GitHub'], stars: 123, installs: 45, is_downloaded: false, downloaded_path: null,
    } : null;
    return { key, id: repoId, kind, title, url: repo?.url || model?.url || skill?.github_url,
      description: 'Synthetic English catalog description.', category: category === 'all' ? 'ia_agentes' : category,
      source: source === 'hf' ? 'Hugging Face (fixture)' : 'GitHub (fixture)',
      first_seen: null, published_at: mode === 'news' ? '2026-10-15' : null, repo, model, skill, summary };
  });
  activeKeys = items.map((item) => item.key);
  return { items, page, has_more: page === 1, next_cursor: page === 1 && source === 'hf' ? 'fixture-next' : null,
    total: 4, notice: `DADOS SINTÉTICOS · ${domain}/${mode}/${source} · página ${page} · categoria ${category}${query ? ` · busca ${query}` : ''}`,
    categories: [{ id: 'ia_agentes', label: 'IA e agentes (fixture)' }, { id: 'dev_tools', label: 'Ferramentas (fixture)' }] };
}

window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  async invoke(command, args = {}) {
    track(command, args);
    if (command === 'plugin:window|set_title') return null;
    if (command === 'get_status') return { is_syncing: false, last_sync: null, total_repos: 0 };
    if (command === 'get_settings') return { github_dir: 'E:\\FIXTURE\\GitHub', models_dir: 'E:\\FIXTURE\\Models', skills_dir: 'E:\\FIXTURE\\Skills', has_github_token: false };
    if (command === 'get_library') return { favorites: [], downloaded: [] };
    if (command === 'ai_models') return { ollama_online: true, default_model: 'test-model', models: ['test-model'] };
    if (command === 'get_catalog') return catalog(args.request);
    if (command === 'get_summary_records') return args.keys.map((key) => {
      const original = records.get(key);
      if (!original) throw new Error(`Fixture: chave de resumo desconhecida ${key}`);
      const count = (polls.get(key) || 0) + 1;
      polls.set(key, count);
      const record = count >= 2 ? { ...original, status: 'ready', description_pt: `Descrição verificada em português para ${key}.`, summary: `Resumo verificado em português para ${key}. Este conteúdo demonstra a tradução e o resumo progressivos da interface.` } : { ...original, status: 'running' };
      records.set(key, record);
      return record;
    });
    if (command === 'get_summary_status') {
      const values = activeKeys.map((key) => records.get(key));
      return { pending: values.filter((record) => record.status === 'pending').length,
        running: values.filter((record) => record.status === 'running').length,
        ready: values.filter((record) => record.status === 'ready').length, errors: 0, model: 'test-model' };
    }
    throw new Error(`Fixture não implementa o comando ${command}; nenhuma ação real foi executada.`);
  },
};

const { default: App } = await import('../src/App.jsx');
createRoot(document.getElementById('root')).render(React.createElement(App));
