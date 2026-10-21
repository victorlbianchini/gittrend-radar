import React, { useMemo, useState } from 'react';
import { BookOpen, Boxes, Cpu, Database, FolderGit2, FolderOpen, HardDrive, Layers, List, RefreshCw, Search } from 'lucide-react';
import ContentCard from './ContentCard';
import { api } from '../api';
import useSummaries from '../hooks/useSummaries';
import { summaryKey } from '../hooks/summaryState';

function isMcp(repo) {
  return (
    repo.category === 'Servidor MCP' ||
    repo.repo_id.toLowerCase().includes('mcp') ||
    repo.description?.toLowerCase().includes('model context protocol') ||
    repo.description?.toLowerCase().includes('mcp server')
  );
}

function matches(title, desc, id, cat, q) {
  if (!q) return true;
  return (
    title?.toLowerCase().includes(q) ||
    desc?.toLowerCase().includes(q) ||
    id?.toLowerCase().includes(q) ||
    cat?.toLowerCase().includes(q)
  );
}

function mapRepo(repo) {
  return {
    key: `github:${repo.repo_id}`,
    id: repo.repo_id,
    kind: 'github',
    title: repo.name,
    description: repo.description,
    description_pt: repo.analysis?.description_pt || '',
    url: repo.url,
    source: 'GitHub',
    category: repo.language,
    repo,
  };
}

function mapMcp(repo) {
  return {
    key: `mcp:${repo.repo_id}`,
    id: repo.repo_id,
    kind: 'mcp',
    title: repo.name,
    description: repo.description,
    description_pt: repo.analysis?.description_pt || '',
    url: repo.url,
    source: 'GitHub',
    category: 'Servidor MCP',
    repo,
  };
}

function mapModel(model) {
  return {
    key: `hf:${model.id}`,
    id: model.id,
    kind: 'hf',
    title: model.name,
    description: model.task_label || 'Modelo baixado',
    description_pt: model.summary || '',
    url: model.url,
    source: 'Hugging Face',
    category: model.group,
    model,
  };
}

function mapDataset(dataset) {
  return {
    key: `dataset:${dataset.id}`,
    id: dataset.id,
    kind: 'dataset',
    title: dataset.name,
    description: dataset.task_label || 'Dataset baixado',
    description_pt: dataset.summary || '',
    url: dataset.url,
    source: 'Hugging Face',
    category: dataset.group,
    model: dataset,
  };
}

function mapSkill(skill) {
  return {
    key: `skill:${skill.id}`,
    id: skill.id,
    kind: 'skill',
    title: skill.name,
    description: skill.description,
    description_pt: '',
    url: skill.github_url,
    source: skill.sources?.join(', ') || 'Local',
    category: 'skills',
    skill,
  };
}

export default function DownloadedView({
  library,
  cardProps = {},
  onOpenFolder,
  directories,
  onNotice,
  onLibraryChanged,
}) {
  const [activeCategory, setActiveCategory] = useState('all');
  const [search, setSearch] = useState('');
  const [organize, setOrganize] = useState('grouped'); // 'grouped' | 'list'
  const [updating, setUpdating] = useState(false);

  const allRepos = library?.downloaded;
  const models = library?.downloaded_models;
  const datasets = library?.downloaded_datasets;
  const skills = library?.downloaded_skills;

  async function updateAll() {
    setUpdating(true);
    try {
      const results = await api.updateRepos();
      const updated = results.filter((r) => r.status === 'updated').length;
      const upToDate = results.filter((r) => r.status === 'up_to_date').length;
      const errors = results.filter((r) => r.status === 'error');
      onNotice?.(errors.length ? 'error' : 'success', 'Atualização em lote', `${updated} atualizado(s), ${upToDate} em dia, ${errors.length} com erro.`);
      if (errors.length) {
        errors.slice(0, 3).forEach((r) => onNotice?.('error', r.folder, r.message));
      }
      onLibraryChanged?.();
    } catch (err) {
      onNotice?.('error', 'Falha ao atualizar', String(err));
    } finally {
      setUpdating(false);
    }
  }

  const query = search.trim().toLowerCase();

  const filteredRepos = useMemo(() => {
    const list = (allRepos || []).filter((r) => !isMcp(r));
    return list
      .filter((r) => matches(r.name, r.description, r.repo_id, r.language, query))
      .map(mapRepo);
  }, [allRepos, query]);

  const filteredMcp = useMemo(() => {
    const list = (allRepos || []).filter(isMcp);
    return list
      .filter((r) => matches(r.name, r.description, r.repo_id, 'Servidor MCP', query))
      .map(mapMcp);
  }, [allRepos, query]);

  const filteredModels = useMemo(() => {
    return (models || [])
      .filter((m) => matches(m.name, m.task_label, m.id, m.group, query))
      .map(mapModel);
  }, [models, query]);

  const filteredDatasets = useMemo(() => {
    return (datasets || [])
      .filter((d) => matches(d.name, d.task_label, d.id, d.group, query))
      .map(mapDataset);
  }, [datasets, query]);

  const filteredSkills = useMemo(() => {
    return (skills || [])
      .filter((s) => matches(s.name, s.description, s.id, 'skills', query))
      .map(mapSkill);
  }, [skills, query]);

  const totalReposCount = (allRepos || []).filter((r) => !isMcp(r)).length;
  const totalMcpCount = (allRepos || []).filter(isMcp).length;
  const totalModelsCount = (models || []).length;
  const totalDatasetsCount = (datasets || []).length;
  const totalSkillsCount = (skills || []).length;

  const totalCount = library?.total_downloaded ?? (
    (allRepos || []).length + totalModelsCount + totalDatasetsCount + totalSkillsCount
  );

  // Unified list for summaries
  const allFiltered = useMemo(() => {
    if (activeCategory === 'github') return filteredRepos;
    if (activeCategory === 'mcp') return filteredMcp;
    if (activeCategory === 'models') return filteredModels;
    if (activeCategory === 'datasets') return filteredDatasets;
    if (activeCategory === 'skills') return filteredSkills;
    return [...filteredRepos, ...filteredMcp, ...filteredModels, ...filteredDatasets, ...filteredSkills];
  }, [activeCategory, filteredRepos, filteredMcp, filteredModels, filteredDatasets, filteredSkills]);

  const summaries = useSummaries(allFiltered);

  const renderCard = (item) => {
    const record = summaries.records[summaryKey(item)] || item.summary || null;
    return (
      <ContentCard
        key={item.key}
        item={item}
        record={record}
        onDownload={item.repo ? () => cardProps.onCloneClick?.(item.repo) : undefined}
        onOpenFolder={onOpenFolder}
        onToggleFavorite={item.repo && cardProps.onToggleFavorite ? () => cardProps.onToggleFavorite(item.repo) : undefined}
      />
    );
  };

  return (
    <section className="downloaded-hub" aria-label="Todos os itens baixados">
      {/* Header com Filtros de Categoria, Busca e Acoes de Pasta */}
      <div className="downloaded-hub-header">
        <div className="downloaded-categories-pills" role="tablist" aria-label="Categorias de baixados">
          <button
            type="button"
            role="tab"
            aria-selected={activeCategory === 'all'}
            className={`downloaded-cat-btn ${activeCategory === 'all' ? 'is-active' : ''}`}
            onClick={() => setActiveCategory('all')}
          >
            <HardDrive size={15} />
            Todos
            <span className="downloaded-cat-badge">{totalCount}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={activeCategory === 'github'}
            className={`downloaded-cat-btn ${activeCategory === 'github' ? 'is-active' : ''}`}
            onClick={() => setActiveCategory('github')}
          >
            <FolderGit2 size={15} />
            GitHub
            <span className="downloaded-cat-badge">{totalReposCount}</span>
          </button>
          {totalMcpCount > 0 && (
            <button
              type="button"
              role="tab"
              aria-selected={activeCategory === 'mcp'}
              className={`downloaded-cat-btn ${activeCategory === 'mcp' ? 'is-active' : ''}`}
              onClick={() => setActiveCategory('mcp')}
            >
              <Cpu size={15} />
              Servidores MCP
              <span className="downloaded-cat-badge">{totalMcpCount}</span>
            </button>
          )}
          <button
            type="button"
            role="tab"
            aria-selected={activeCategory === 'models'}
            className={`downloaded-cat-btn ${activeCategory === 'models' ? 'is-active' : ''}`}
            onClick={() => setActiveCategory('models')}
          >
            <Boxes size={15} />
            Modelos de IA
            <span className="downloaded-cat-badge">{totalModelsCount}</span>
          </button>
          {totalDatasetsCount > 0 && (
            <button
              type="button"
              role="tab"
              aria-selected={activeCategory === 'datasets'}
              className={`downloaded-cat-btn ${activeCategory === 'datasets' ? 'is-active' : ''}`}
              onClick={() => setActiveCategory('datasets')}
            >
              <Database size={15} />
              Datasets
              <span className="downloaded-cat-badge">{totalDatasetsCount}</span>
            </button>
          )}
          <button
            type="button"
            role="tab"
            aria-selected={activeCategory === 'skills'}
            className={`downloaded-cat-btn ${activeCategory === 'skills' ? 'is-active' : ''}`}
            onClick={() => setActiveCategory('skills')}
          >
            <BookOpen size={15} />
            Skills
            <span className="downloaded-cat-badge">{totalSkillsCount}</span>
          </button>
        </div>

        {/* Barra de Busca e Acoes */}
        <div className="downloaded-hub-controls">
          <div className="downloaded-search-wrap">
            <Search size={15} aria-hidden="true" />
            <input
              type="text"
              className="downloaded-search-input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filtrar por nome, termo ou autor..."
            />
          </div>

          {activeCategory === 'all' && (
            <div className="downloaded-view-toggle">
              <button
                type="button"
                className={`downloaded-toggle-btn ${organize === 'grouped' ? 'is-active' : ''}`}
                onClick={() => setOrganize('grouped')}
                title="Agrupar por categoria"
              >
                <Layers size={14} />
                Agrupado
              </button>
              <button
                type="button"
                className={`downloaded-toggle-btn ${organize === 'list' ? 'is-active' : ''}`}
                onClick={() => setOrganize('list')}
                title="Lista contínua"
              >
                <List size={14} />
                Lista
              </button>
            </div>
          )}

          <button
            type="button"
            className="btn btn-secondary"
            style={{ padding: '8px 14px', fontSize: 13 }}
            onClick={updateAll}
            disabled={updating}
            title="git pull --rebase em todos os repositórios baixados"
          >
            <RefreshCw size={15} className={updating ? 'animate-spin' : ''} />
            Atualizar tudo
          </button>

          {directories?.github_dir && onOpenFolder && (
            <button
              type="button"
              className="btn btn-secondary"
              style={{ padding: '8px 14px', fontSize: 13 }}
              onClick={() => onOpenFolder(directories.github_dir)}
              title="Abrir pasta local de repositórios"
            >
              <FolderOpen size={15} />
              Pasta Repositórios
            </button>
          )}
        </div>
      </div>

      {/* Status da lista */}
      <div className="catalog-v2-status" role="status">
        <span className="catalog-v2-notice">
          {totalCount === 0
            ? 'Nenhum item baixado encontrado ainda.'
            : `${totalCount} ${totalCount === 1 ? 'item baixado' : 'itens baixados'} no total (${totalReposCount} repositórios GitHub, ${totalMcpCount} servidores MCP, ${totalModelsCount} modelos HF, ${totalDatasetsCount} datasets, ${totalSkillsCount} skills). Todos destacados em verde com status Na máquina.`}
        </span>
      </div>

      {/* Conteudo Renderizado */}
      {totalCount === 0 ? (
        <div className="glass-panel catalog-v2-empty">
          Nenhum item baixado encontrado. Baixe repositórios, servidores MCP, modelos, datasets ou skills no catálogo para vê-los organizados aqui.
        </div>
      ) : activeCategory === 'all' && organize === 'grouped' ? (
        <div>
          {/* Seção GitHub */}
          {filteredRepos.length > 0 && (
            <section style={{ marginBottom: 32 }}>
              <div className="catalog-group-divider is-downloaded-group">
                <span className="catalog-group-title">
                  <FolderGit2 size={16} />
                  Repositórios GitHub
                </span>
                <span className="catalog-group-count">{filteredRepos.length} baixados</span>
              </div>
              <div className="repo-grid-luxury">
                {filteredRepos.map(renderCard)}
              </div>
            </section>
          )}

          {/* Seção Servidores MCP */}
          {filteredMcp.length > 0 && (
            <section style={{ marginBottom: 32 }}>
              <div className="catalog-group-divider is-downloaded-group">
                <span className="catalog-group-title">
                  <Cpu size={16} />
                  Servidores MCP
                </span>
                <span className="catalog-group-count">{filteredMcp.length} baixados</span>
              </div>
              <div className="repo-grid-luxury">
                {filteredMcp.map(renderCard)}
              </div>
            </section>
          )}

          {/* Seção Modelos */}
          {filteredModels.length > 0 && (
            <section style={{ marginBottom: 32 }}>
              <div className="catalog-group-divider is-downloaded-group">
                <span className="catalog-group-title">
                  <Boxes size={16} />
                  Modelos de IA (Hugging Face)
                </span>
                <span className="catalog-group-count">{filteredModels.length} baixados</span>
              </div>
              <div className="repo-grid-luxury">
                {filteredModels.map(renderCard)}
              </div>
            </section>
          )}

          {/* Seção Datasets */}
          {filteredDatasets.length > 0 && (
            <section style={{ marginBottom: 32 }}>
              <div className="catalog-group-divider is-downloaded-group">
                <span className="catalog-group-title">
                  <Database size={16} />
                  Datasets de IA (Hugging Face)
                </span>
                <span className="catalog-group-count">{filteredDatasets.length} baixados</span>
              </div>
              <div className="repo-grid-luxury">
                {filteredDatasets.map(renderCard)}
              </div>
            </section>
          )}

          {/* Seção Skills */}
          {filteredSkills.length > 0 && (
            <section style={{ marginBottom: 32 }}>
              <div className="catalog-group-divider is-downloaded-group">
                <span className="catalog-group-title">
                  <BookOpen size={16} />
                  Skills de Agentes
                </span>
                <span className="catalog-group-count">{filteredSkills.length} baixadas</span>
              </div>
              <div className="repo-grid-luxury">
                {filteredSkills.map(renderCard)}
              </div>
            </section>
          )}

          {filteredRepos.length === 0 && filteredMcp.length === 0 && filteredModels.length === 0 && filteredDatasets.length === 0 && filteredSkills.length === 0 && (
            <div className="glass-panel catalog-v2-empty">
              Nenhum item corresponde à busca &quot;{search}&quot;.
            </div>
          )}
        </div>
      ) : (
        <div className="repo-grid-luxury">
          {allFiltered.length === 0 ? (
            <div className="glass-panel catalog-v2-empty" style={{ gridColumn: '1 / -1' }}>
              Nenhum item corresponde à busca &quot;{search}&quot;.
            </div>
          ) : (
            allFiltered.map(renderCard)
          )}
        </div>
      )}
    </section>
  );
}
