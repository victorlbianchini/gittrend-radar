import React, { useEffect, useRef, useState } from 'react';
import { HardDrive, Loader2, RefreshCw, Search, Sparkles } from 'lucide-react';
import { api } from '../api';
import useSummaries from '../hooks/useSummaries';
import { summaryKey } from '../hooks/summaryState';
import ContentCard from './ContentCard';

const EMPTY = [];
const labels = {
  github: 'repositórios',
  mcp: 'servidores MCP',
  models: 'modelos de IA',
  datasets: 'datasets de IA',
  skills: 'skills',
};

// The parent keys this component by domain/mode, so each catalog owns its own
// cursor history and asynchronous lifecycle.
export default function CatalogView({ domain, mode, cardProps = {}, onNotice, onOpenFolder }) {
  const [source, setSource] = useState(
    domain === 'models' || domain === 'datasets'
      ? 'hf'
      : domain === 'github' || domain === 'mcp'
        ? 'github'
        : 'all'
  );
  const [query, setQuery] = useState('');
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('all');
  const [position, setPosition] = useState({ page: 1, cursors: [null] });
  const [revision, setRevision] = useState(0);
  const [result, setResult] = useState(null);
  const [requestState, setRequestState] = useState({ key: '', error: '' });
  const [error, setError] = useState('');
  const [downloadJobs, setDownloadJobs] = useState({});
  const [downloadError, setDownloadError] = useState('');
  const [modelRequests, setModelRequests] = useState(0);
  const [downloadFilter, setDownloadFilter] = useState('all');
  const [organizeBy, setOrganizeBy] = useState('unDownloadedFirst');
  const alive = useRef(true);
  const requestKey = JSON.stringify([domain, mode, source, search, category, position.page, position.cursors[position.page - 1], revision]);
  const items = result?.requestKey === requestKey ? result.page.items : EMPTY;
  const page = result?.requestKey === requestKey ? result.page : null;
  const busy = requestState.key !== requestKey;
  const requestError = requestState.key === requestKey ? requestState.error : '';
  const summaries = useSummaries(items);
  const categories = result?.page.categories || EMPTY;
  const categoryOptions = category !== 'all' && !categories.some((item) => item.id === category)
    ? [...categories, { id: category, label: category.replaceAll('_', ' ') }] : categories;

  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    let active = true;
    let timer;
    async function poll() {
      try {
        const jobs = await api.downloads();
        if (!active) return;
        setDownloadJobs(Object.fromEntries(jobs.map((job) => [job.id, job])));
        setDownloadError('');
        if (modelRequests > 0 || jobs.some((job) => !job.finished)) {
          timer = setTimeout(poll, 1000);
        } else {
          timer = setTimeout(poll, 3000);
        }
      } catch (err) {
        if (!active) return;
        setDownloadError(String(err));
        timer = setTimeout(poll, 5000);
      }
    }
    poll();
    return () => { active = false; clearTimeout(timer); };
  }, [domain, source, modelRequests]);
  useEffect(() => {
    let active = true;
    const [currentDomain, currentMode, currentSource, currentQuery, currentCategory, currentPage, cursor] = JSON.parse(requestKey);
    api.catalog({ domain: currentDomain, mode: currentMode, source: currentSource, query: currentQuery, page: currentPage, cursor, category: currentCategory })
      .then((data) => { if (active) { setResult({ requestKey, page: data }); setRequestState({ key: requestKey, error: '' }); } })
      .catch((err) => { if (active) setRequestState({ key: requestKey, error: String(err) }); });
    return () => { active = false; };
  }, [requestKey]);

  function resetPosition() { setPosition({ page: 1, cursors: [null] }); }
  async function retry(keys) {
    try { await summaries.retry(keys); } catch (err) { if (alive.current) setError(String(err)); }
  }
  async function download(item, full = false) {
    if (item.repo) {
      const data = await cardProps.onCloneClick?.(item.repo);
      if (data?.success && alive.current) {
        setResult((current) => current && ({
          ...current,
          page: {
            ...current.page,
            items: current.page.items.map((entry) =>
              entry.key === item.key
                ? {
                    ...entry,
                    repo: {
                      ...entry.repo,
                      is_downloaded: true,
                      downloaded_path: data.folder_path,
                    },
                  }
                : entry
            ),
          },
        }));
      }
      return data;
    }
    if (item.model) {
      setModelRequests((count) => count + 1);
      try {
        await api.downloadModel(item.model.id, full);
        onNotice?.('Download iniciado.');
      } finally {
        if (alive.current) setModelRequests((count) => count - 1);
      }
      return;
    }
    const data = await api.downloadSkill(item.skill);
    if (!data.success) throw new Error(data.message || 'Não foi possível baixar a skill.');
    if (!alive.current) return;
    setResult((current) => current && ({ ...current, page: { ...current.page, items: current.page.items.map((entry) => entry.key === item.key ? { ...entry, skill: { ...entry.skill, is_downloaded: true, downloaded_path: data.folder_path } } : entry) } }));
    onNotice?.(data.message || 'Skill baixada.');
  }
  async function toggleFavorite(repo) {
    const value = await cardProps.onToggleFavorite?.(repo);
    if (!alive.current || typeof value !== 'boolean') return value;
    setResult((current) => current && ({ ...current, page: { ...current.page, items: current.page.items.map((item) => item.repo?.repo_id === repo.repo_id ? { ...item, repo: { ...item.repo, is_favorite: value } } : item) } }));
    return value;
  }

  const isItemDownloaded = (item) => Boolean(
    item.repo?.is_downloaded ||
    item.skill?.is_downloaded ||
    item.model?.is_downloaded ||
    (item.model && downloadJobs[`hf:${item.model.id}`]?.finished && downloadJobs[`hf:${item.model.id}`]?.success)
  );

  let filteredItems = items;
  if (downloadFilter === 'unDownloaded') {
    filteredItems = filteredItems.filter((item) => !isItemDownloaded(item));
  } else if (downloadFilter === 'downloaded') {
    filteredItems = filteredItems.filter((item) => isItemDownloaded(item));
  }

  let displayItems = filteredItems;
  if (organizeBy === 'unDownloadedFirst') {
    displayItems = [...filteredItems].sort((a, b) => {
      const aDl = isItemDownloaded(a);
      const bDl = isItemDownloaded(b);
      if (aDl === bDl) return 0;
      return aDl ? 1 : -1;
    });
  } else if (organizeBy === 'downloadedFirst') {
    displayItems = [...filteredItems].sort((a, b) => {
      const aDl = isItemDownloaded(a);
      const bDl = isItemDownloaded(b);
      if (aDl === bDl) return 0;
      return aDl ? -1 : 1;
    });
  }

  const downloadedCountOnPage = items.filter(isItemDownloaded).length;
  const newCountOnPage = items.length - downloadedCountOnPage;

  const progress = summaries.progress;
  const pct = progress.total ? Math.round((progress.ready / progress.total) * 100) : 0;
  const retryErrors = () => retry(items.filter((item) => (summaries.records[summaryKey(item)] || item.summary)?.status === 'error').map(summaryKey));

  return <div className="catalog-v2">
    <form className="catalog-v2-filters" onSubmit={(event) => { event.preventDefault(); setSearch(query.trim()); resetPosition(); setRevision((value) => value + 1); }}>
      <label className="catalog-v2-search">
        <span className="catalog-v2-label">Buscar {labels[domain]}</span>
        <span className="catalog-v2-input-wrap">
          <Search size={16} aria-hidden="true" />
          <input className="input-field" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Nome, tema ou repositório…" />
        </span>
      </label>
      {(domain === 'models' || domain === 'datasets') && <label className="catalog-v2-field"><span className="catalog-v2-label">Fonte</span><select className="input-field" value={source} onChange={(event) => { setSource(event.target.value); setCategory('all'); resetPosition(); }}><option value="hf">Hugging Face</option><option value="github">GitHub</option></select></label>}
      <label className="catalog-v2-field"><span className="catalog-v2-label">Categoria</span><select className="input-field" value={category} onChange={(event) => { setCategory(event.target.value); resetPosition(); }}><option value="all">Todas</option>{categoryOptions.filter((item) => item.id !== 'all').map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
      <label className="catalog-v2-field">
        <span className="catalog-v2-label">Download</span>
        <select className="input-field" value={downloadFilter} onChange={(e) => setDownloadFilter(e.target.value)}>
          <option value="all">Todos</option>
          <option value="unDownloaded">Apenas não baixados</option>
          <option value="downloaded">Apenas já baixados</option>
        </select>
      </label>
      <label className="catalog-v2-field">
        <span className="catalog-v2-label">Organizar</span>
        <select className="input-field" value={organizeBy} onChange={(e) => setOrganizeBy(e.target.value)}>
          <option value="unDownloadedFirst">Não baixados em cima</option>
          <option value="source">Ordem padrão</option>
          <option value="downloadedFirst">Baixados em cima</option>
        </select>
      </label>
      <div className="catalog-v2-actions">
        <button type="submit" className="btn btn-primary" disabled={busy}><Search size={15} />Buscar</button>
        <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => setRevision((value) => value + 1)} aria-label="Atualizar catálogo" title="Atualizar"><RefreshCw size={15} /></button>
      </div>
    </form>

    <div className="catalog-v2-status" role="status">
      <span className="catalog-v2-notice">
        {page?.notice || 'Consultando a fonte selecionada…'}
        {items.length > 0 && (
          <> · <strong>{downloadedCountOnPage}</strong> baixados / <strong>{newCountOnPage}</strong> novos nesta página</>
        )}
      </span>
      {items.length > 0 && <span className="catalog-v2-progress">
        <span className="catalog-v2-progress-text">Resumos da IA <strong>{progress.ready}/{progress.total}</strong>{progress.running > 0 ? ` · ${progress.running} processando` : ''}{progress.pending > 0 ? ` · ${progress.pending} na fila` : ''}{progress.error > 0 ? ` · ${progress.error} com erro` : ''}</span>
        <span className="catalog-v2-bar" aria-hidden="true"><i style={{ width: `${pct}%` }} /></span>
        {progress.error > 0 && <button type="button" className="btn-details-glass" onClick={retryErrors}>Repetir com erro</button>}
      </span>}
    </div>

    {(requestError || error || summaries.error || downloadError) && <p className="glass-panel catalog-v2-alert" role="alert">{requestError || error || summaries.error || downloadError}</p>}
    {busy ? <p role="status" className="glass-panel catalog-v2-empty"><Loader2 size={20} className="animate-spin" />Carregando {labels[domain]}…</p> : displayItems.length === 0 ? <p className="glass-panel catalog-v2-empty">{requestError ? 'A consulta falhou. Use Atualizar para tentar novamente.' : downloadFilter === 'downloaded' ? 'Nenhum item baixado nesta página do catálogo.' : downloadFilter === 'unDownloaded' ? 'Todos os itens desta página já foram baixados!' : 'Nenhum resultado nesta categoria e período.'}</p> : <div className="repo-grid-luxury">
      {displayItems.map((item, index) => {
        const record = summaries.records[summaryKey(item)] || item.summary;
        const rank = mode === 'tops' ? (position.page - 1) * 50 + index + 1 : undefined;
        const isDl = isItemDownloaded(item);
        const prevIsDl = index > 0 ? isItemDownloaded(displayItems[index - 1]) : null;

        let divider = null;
        if (downloadFilter === 'all' && downloadedCountOnPage > 0 && newCountOnPage > 0) {
          if (organizeBy === 'unDownloadedFirst') {
            if (index === 0 && !isDl) {
              divider = (
                <div key="group-new" className="catalog-group-divider">
                  <span className="catalog-group-title"><Sparkles size={15} /> Novos / Disponíveis para baixar</span>
                  <span className="catalog-group-count">{newCountOnPage} itens</span>
                </div>
              );
            } else if (isDl && prevIsDl === false) {
              divider = (
                <div key="group-downloaded" className="catalog-group-divider is-downloaded-group">
                  <span className="catalog-group-title"><HardDrive size={15} /> Já baixados na máquina</span>
                  <span className="catalog-group-count">{downloadedCountOnPage} itens</span>
                </div>
              );
            }
          } else if (organizeBy === 'downloadedFirst') {
            if (index === 0 && isDl) {
              divider = (
                <div key="group-downloaded" className="catalog-group-divider is-downloaded-group">
                  <span className="catalog-group-title"><HardDrive size={15} /> Já baixados na máquina</span>
                  <span className="catalog-group-count">{downloadedCountOnPage} itens</span>
                </div>
              );
            } else if (!isDl && prevIsDl === true) {
              divider = (
                <div key="group-new" className="catalog-group-divider">
                  <span className="catalog-group-title"><Sparkles size={15} /> Novos / Disponíveis para baixar</span>
                  <span className="catalog-group-count">{newCountOnPage} itens</span>
                </div>
              );
            }
          }
        }

        return (
          <React.Fragment key={item.key}>
            {divider}
            <ContentCard key={item.key} item={item} record={record} rank={rank} downloadJob={item.model ? downloadJobs[`hf:${item.model.id}`] : undefined}
              onRetry={() => retry([summaryKey(item)])} onDownload={(full) => download(item, full)}
              onOpenFolder={onOpenFolder} onToggleFavorite={item.repo && cardProps.onToggleFavorite ? () => toggleFavorite(item.repo) : undefined} />
          </React.Fragment>
        );
      })}
    </div>}
    <nav aria-label="Paginação do catálogo" className="catalog-v2-pager">
      <button type="button" className="btn btn-secondary" disabled={busy || position.page === 1} onClick={() => setPosition((current) => ({ ...current, page: current.page - 1 }))}>Anterior</button>
      <span>Página <strong>{position.page}</strong>{page?.total != null ? ` · ${page.total.toLocaleString('pt-BR')} resultados na fonte` : ''}</span>
      <button type="button" className="btn btn-secondary" disabled={busy || !page?.has_more} onClick={() => setPosition((current) => ({ page: current.page + 1, cursors: [...current.cursors.slice(0, current.page), page.next_cursor || null] }))}>Próxima</button>
    </nav>
  </div>;
}
