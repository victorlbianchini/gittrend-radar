import { useCallback, useEffect, useState } from 'react';
import { Bell, BookOpen, Boxes, Bot, Compass, Cpu, Database, Download, FolderGit2, Bookmark, HardDrive, Lightbulb, Settings, TrendingUp } from 'lucide-react';
import Header from './components/Header';
import CatalogView from './components/CatalogView';
import SettingsModal from './components/SettingsModal';
import DownloadsView from './components/DownloadsView';
import DownloadedView from './components/DownloadedView';
import AgentRequestsView from './components/AgentRequestsView';
import StackSuggestionsView from './components/StackSuggestionsView';
import RepoCard from './components/RepoCard';
import Toast from './components/Toast';
import { api } from './api';
import useSummaries from './hooks/useSummaries';

const DOMAINS = [
  { id: 'github', label: 'GitHub', Icon: FolderGit2 },
  { id: 'mcp', label: 'Servidores MCP', Icon: Cpu },
  { id: 'models', label: 'Modelos de IA', Icon: Boxes },
  { id: 'datasets', label: 'Datasets de IA', Icon: Database },
  { id: 'skills', label: 'Skills', Icon: BookOpen },
];
const MODES = [
  { id: 'tops', label: 'Tops', Icon: TrendingUp },
  { id: 'news', label: 'Novidades de hoje', Icon: Bell },
  { id: 'explore', label: 'Explorar todos', Icon: Compass },
];

export default function App() {
  const [domain, setDomain] = useState('github');
  const [modes, setModes] = useState({ github: 'tops', mcp: 'tops', models: 'tops', datasets: 'tops', skills: 'tops' });
  const [utility, setUtility] = useState(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [directories, setDirectories] = useState(null);
  const [aiStatus, setAiStatus] = useState(null);
  const [syncStatus, setSyncStatus] = useState(null);
  const [isSyncing, setIsSyncing] = useState(false);
  const [library, setLibrary] = useState({ favorites: [], downloaded: [], downloaded_models: [], downloaded_datasets: [], downloaded_skills: [], total_downloaded: 0 });
  const [revision, setRevision] = useState(0);
  const [toasts, setToasts] = useState([]);
  const [agentRequests, setAgentRequests] = useState([]);

  const notice = useCallback((type, title, message) => {
    const id = `${Date.now()}-${Math.random()}`;
    setToasts((list) => [...list, { id, type, title, message }]);
    setTimeout(() => setToasts((list) => list.filter((item) => item.id !== id)), 6000);
  }, []);
  const refreshRequests = useCallback(async () => {
    try { setAgentRequests(await api.agentRequests()); } catch { /* app abre mesmo sem a tabela */ }
  }, []);
  const refreshLibrary = useCallback(async () => {
    try { setLibrary(await api.library()); }
    catch (err) { notice('error', 'Falha ao carregar biblioteca', String(err)); }
  }, [notice]);

  useEffect(() => {
    let active = true;
    let timer;
    let wasSyncing = false;
    let knownPending = null;
    const poll = async () => {
      try {
        const reqs = await api.agentRequests().catch(() => null);
        if (active && reqs) {
          setAgentRequests(reqs);
          const pending = reqs.filter((r) => r.status === 'pending').map((r) => r.id);
          if (knownPending && pending.some((id) => !knownPending.has(id))) notice('info', 'Novo pedido de agente', 'Um agente pediu um download. Veja em Pedidos dos agentes.');
          knownPending = new Set(pending);
        }
        const status = await api.status();
        if (!active) return;
        setSyncStatus(status);
        setIsSyncing(status.is_syncing);
        if (wasSyncing && !status.is_syncing) { setRevision((n) => n + 1); refreshLibrary(); }
        wasSyncing = status.is_syncing;
      } catch (err) { if (active) notice('error', 'Falha ao consultar sincronização', String(err)); }
      if (active) timer = setTimeout(poll, 5000);
    };
    poll();
    api.settings().then((value) => { if (active) setDirectories(value); }).catch((err) => notice('error', 'Configurações indisponíveis', String(err)));
    api.aiModels().then((value) => { if (active) setAiStatus(value); }).catch((err) => notice('error', 'Motor indisponível', String(err)));
    Promise.resolve().then(refreshLibrary);
    const linkError = (event) => notice('error', 'Falha ao abrir link', event.detail);
    document.addEventListener('backend-error', linkError);
    return () => { active = false; clearTimeout(timer); document.removeEventListener('backend-error', linkError); };
  }, [notice, refreshLibrary]);
  useEffect(() => { api.setTitle(`GitTrend Radar — ${DOMAINS.find((item) => item.id === domain).label}`); }, [domain]);

  async function sync() {
    setIsSyncing(true);
    try { await api.sync(true); notice('info', 'Sincronização iniciada', 'Os resumos de IA são processados em segundo plano, em lotes de 50.'); }
    catch (err) { setIsSyncing(false); notice('error', 'Falha ao sincronizar', String(err)); }
  }
  async function openFolder(path) {
    try { const result = await api.openFolder(path); if (!result.success) throw new Error(result.message); }
    catch (err) { notice('error', 'Falha ao abrir pasta', String(err)); }
  }
  async function favorite(repo) {
    try {
      const value = await api.toggleFavorite(repo.repo_id);
      await refreshLibrary();
      return value;
    } catch (err) { notice('error', 'Falha ao favoritar', String(err)); }
  }
  async function downloadRepo(repo) {
    try {
      const result = await api.cloneRepo(repo.url, repo.name, null);
      if (!result.success) throw new Error(result.message);
      notice('success', result.already_exists ? 'Já baixado' : 'Download concluído', result.message);
      await refreshLibrary();
      return result;
    } catch (err) { notice('error', 'Falha ao baixar', String(err)); throw err; }
  }
  const pendingCount = agentRequests.filter((r) => r.status === 'pending').length;
  const totalDownloaded = library?.total_downloaded ?? ((library?.downloaded?.length || 0) + (library?.downloaded_models?.length || 0) + (library?.downloaded_datasets?.length || 0) + (library?.downloaded_skills?.length || 0));
  const cardProps = { onCloneClick: downloadRepo, onOpenFolder: openFolder, onToggleFavorite: favorite };

  return <div className="app-container">
    <Header githubDir={directories?.github_dir} aiStatus={aiStatus} syncStatus={syncStatus} isSyncing={isSyncing} onTriggerSync={sync} onOpenReposDir={() => openFolder(directories?.github_dir || 'E:\\REPOSITORIOS')} />
    <nav aria-label="Categorias principais" className="main-view-switcher">
      <div className="switcher-pills-cluster">{DOMAINS.map(({ id, label, Icon }) => <button key={id} type="button" aria-current={!utility && domain === id ? 'page' : undefined} className={`switcher-tab-btn ${!utility && domain === id ? 'is-active' : ''}`} onClick={() => { setDomain(id); setUtility(null); }}><Icon size={17} />{label}</button>)}</div>
      <div className="switcher-pills-cluster">
        <button type="button" className={`switcher-tab-btn ${utility === 'favorites' ? 'is-active' : ''}`} onClick={() => setUtility('favorites')}><Bookmark size={15} />Favoritos ({library.favorites.length})</button>
        <button type="button" className={`switcher-tab-btn ${utility === 'downloaded' ? 'is-active' : ''}`} onClick={() => setUtility('downloaded')}><HardDrive size={15} />Baixados ({totalDownloaded})</button>
        <button type="button" className={`switcher-tab-btn ${utility === 'downloads' ? 'is-active' : ''}`} onClick={() => setUtility('downloads')}><Download size={15} />Downloads</button>
        <button type="button" className={`switcher-tab-btn ${utility === 'agents' ? 'is-active' : ''}`} onClick={() => { setUtility('agents'); refreshRequests(); }}><Bot size={15} />Pedidos dos agentes{pendingCount > 0 ? <span className="tab-badge">{pendingCount}</span> : null}</button>
        <button type="button" className={`switcher-tab-btn ${utility === 'suggest' ? 'is-active' : ''}`} onClick={() => setUtility('suggest')}><Lightbulb size={15} />Sugestões da máquina</button>
        <button type="button" className="switcher-tab-btn" onClick={() => setSettingsOpen(true)}><Settings size={15} />Configurações</button>
      </div>
    </nav>
    {!utility && <nav aria-label={`Seções de ${DOMAINS.find((item) => item.id === domain).label}`} className="tabs-container" style={{ margin: '12px 0 20px', flexWrap: 'wrap' }}>{MODES.map(({ id, label, Icon }) => <button key={id} type="button" aria-current={modes[domain] === id ? 'page' : undefined} className={`tab-btn ${modes[domain] === id ? 'active' : ''}`} onClick={() => setModes((current) => ({ ...current, [domain]: id }))}><Icon size={15} />{label}</button>)}</nav>}
    {utility === 'agents' ? <AgentRequestsView requests={agentRequests} onChanged={() => { refreshRequests(); refreshLibrary(); }} onNotice={notice} /> : utility === 'suggest' ? <StackSuggestionsView onNotice={notice} onChanged={refreshRequests} /> : utility === 'downloads' ? <DownloadsView onOpenFolder={openFolder} /> : utility === 'downloaded' ? <DownloadedView library={library} cardProps={cardProps} onOpenFolder={openFolder} directories={directories} onNotice={notice} onLibraryChanged={refreshLibrary} /> : utility === 'favorites' ? <LibraryGrid key="favorites" repos={library.favorites} cardProps={cardProps} /> : <CatalogView key={`${domain}-${modes[domain]}-${revision}`} domain={domain} mode={modes[domain]} cardProps={cardProps} onNotice={(message) => notice('info', 'Catálogo', message)} onOpenFolder={openFolder} />}
    <SettingsModal isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} onSaved={(value) => { setDirectories(value); refreshLibrary(); setRevision((n) => n + 1); notice('success', 'Configurações salvas', 'Os próximos downloads usarão as pastas escolhidas.'); }} />
    <Toast toasts={toasts} onDismiss={(id) => setToasts((list) => list.filter((item) => item.id !== id))} />
  </div>;
}

function LibraryGrid({ repos, cardProps }) {
  const [page, setPage] = useState(1);
  const currentPage = Math.min(page, Math.max(1, Math.ceil(repos.length / 50)));
  const visible = repos.slice((currentPage - 1) * 50, currentPage * 50);
  const signature = JSON.stringify(visible.map((repo) => repo.repo_id));
  const [failure, setFailure] = useState('');
  const summaryItems = visible.map((repo) => ({ key: `github:${repo.repo_id}` }));
  const summaries = useSummaries(summaryItems);
  useEffect(() => {
    let active = true;
    const ids = JSON.parse(signature);
    if (ids.length) api.queueRepoSummaries(ids).catch((err) => { if (active) setFailure(String(err)); });
    return () => { active = false; };
  }, [signature]);
  return <section>
    {(failure || summaries.error) && <p role="alert">{failure || summaries.error}</p>}
    {visible.length > 0 && <div className="catalog-v2-status" role="status" style={{ marginTop: 16 }}>
      <span className="catalog-v2-notice">{repos.length} {repos.length === 1 ? 'repositório' : 'repositórios'} nesta lista.</span>
      <span className="catalog-v2-progress"><span className="catalog-v2-progress-text">Resumos da IA <strong>{summaries.progress.ready}/{visible.length}</strong></span><span className="catalog-v2-bar" aria-hidden="true"><i style={{ width: `${Math.round((summaries.progress.ready / visible.length) * 100)}%` }} /></span></span>
    </div>}
    {!repos.length ? <div className="glass-panel catalog-v2-empty">Nenhum repositório nesta lista ainda.</div> : <div className="repo-grid-luxury">{visible.map((repo) => {
      const record = summaries.records[`github:${repo.repo_id}`];
      return <RepoCard key={repo.repo_id} repo={repo} summaryRecord={record} onRetry={() => summaries.retry([`github:${repo.repo_id}`]).catch((err) => setFailure(String(err)))} {...cardProps} />;
    })}</div>}
    {repos.length > 50 && <nav aria-label="Páginas da biblioteca" className="catalog-v2-pager"><button type="button" className="btn btn-secondary" disabled={currentPage === 1} onClick={() => setPage(currentPage - 1)}>Anterior</button><span>Página {currentPage}</span><button type="button" className="btn btn-secondary" disabled={currentPage * 50 >= repos.length} onClick={() => setPage(currentPage + 1)}>Próxima</button></nav>}
  </section>;
}
