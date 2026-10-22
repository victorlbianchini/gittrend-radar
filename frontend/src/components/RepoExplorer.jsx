import React from 'react';
import { Search, FolderGit2, Layers } from 'lucide-react';
import RepoCard from './RepoCard';

export default function RepoExplorer({
  repos,
  searchQuery,
  onSearchChange,
  selectedCategory,
  onSelectCategory,
  selectedLanguage,
  onSelectLanguage,
  availableLanguages,
  sortBy,
  onSortChange,
  onCloneClick,
  onSelectRepo,
  onOpenFolder,
  onCopyNotice,
  onToggleFavorite
}) {
  const categories = [
    { id: 'all', label: 'Todos os Repositórios' },
    { id: 'ia_agentes', label: 'IA e Agentes' },
    { id: 'frontend_ui', label: 'Frontend e UI' },
    { id: 'backend_api', label: 'Backend e APIs' },
    { id: 'devops_cli', label: 'DevOps e CLI' },
    { id: 'data_eng', label: 'Engenharia de Dados' },
    { id: 'security', label: 'Segurança' },
    { id: 'mobile', label: 'Mobile' },
    { id: 'geral', label: 'Geral' }
  ];

  return (
    <div style={{ marginTop: '10px' }}>
      {/* Search and Filters Bar */}
      <div style={{
        background: 'rgba(11, 15, 25, 0.75)',
        backdropFilter: 'blur(20px)',
        border: '1px solid var(--border-glass)',
        borderTop: '1px solid var(--border-card-highlight)',
        borderRadius: 'var(--radius-xl)',
        padding: '20px 26px',
        marginBottom: '28px',
        boxShadow: 'var(--shadow-card)'
      }}>
        <div style={{ display: 'flex', gap: '14px', flexWrap: 'wrap', alignItems: 'center' }}>
          {/* Text Search Input */}
          <div style={{ position: 'relative', flex: '1 1 300px' }}>
            <Search 
              size={17} 
              style={{ position: 'absolute', left: 15, top: '50%', transform: 'translateY(-50%)', color: 'var(--text-dim)' }} 
            />
            <input
              type="text"
              className="input-field"
              style={{ paddingLeft: '44px' }}
              placeholder="Buscar repositório por nome, autor, descrição ou tópico..."
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
            />
          </div>

          {/* Language Filter */}
          <div style={{ minWidth: '160px' }}>
            <select
              className="input-field"
              value={selectedLanguage}
              onChange={(e) => onSelectLanguage(e.target.value)}
              style={{ cursor: 'pointer' }}
            >
              <option value="all">Todas as Linguagens</option>
              {availableLanguages.map((lang) => (
                <option key={lang} value={lang}>{lang}</option>
              ))}
            </select>
          </div>

          {/* Sort By Filter */}
          <div style={{ minWidth: '170px' }}>
            <select
              className="input-field"
              value={sortBy}
              onChange={(e) => onSortChange(e.target.value)}
              style={{ cursor: 'pointer' }}
            >
              <option value="stars_today">Mais Estrelas Hoje</option>
              <option value="stars">Mais Estrelas Total</option>
              <option value="name">Ordem Alfabetica</option>
            </select>
          </div>
        </div>

        {/* Category Pills */}
        <div className="tabs-container" style={{ marginTop: '18px' }}>
          {categories.map((cat) => (
            <button
              key={cat.id}
              type="button"
              className={`tab-btn ${selectedCategory === cat.id ? 'active' : ''}`}
              onClick={() => onSelectCategory(cat.id)}
            >
              <span>{cat.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Counter bar */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px', padding: '0 4px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Layers size={16} style={{ color: 'var(--accent-indigo)' }} />
          <span style={{ fontSize: '14px', color: 'var(--text-muted)' }}>
            Exibindo <strong style={{ color: '#ffffff' }}>{repos.length}</strong> repositórios
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', color: 'var(--text-dim)' }}>
          <FolderGit2 size={15} style={{ color: 'var(--accent-emerald)' }} />
          <span>Destino: <strong>pasta GitHub configurada</strong></span>
        </div>
      </div>

      {/* Grid of Results */}
      {repos.length === 0 ? (
        <div style={{
          background: 'rgba(11, 15, 25, 0.6)',
          border: '1px solid var(--border-glass)',
          borderRadius: 'var(--radius-xl)',
          padding: '80px 20px',
          textAlign: 'center'
        }}>
          <Search size={40} style={{ color: 'var(--text-dim)', margin: '0 auto 16px' }} />
          <h4 style={{ fontSize: '18px', fontWeight: 700, marginBottom: '8px' }}>
            Nenhum repositório encontrado
          </h4>
          <p style={{ fontSize: '13px', color: 'var(--text-muted)' }}>
            Tente buscar com outros termos ou altere o filtro de categoria/linguagem.
          </p>
        </div>
      ) : (
        <div className="repo-grid">
          {repos.map((repo, idx) => (
            <RepoCard
              key={repo.repo_id}
              repo={repo}
              rank={idx + 1}
              onCloneClick={onCloneClick}
              onSelectRepo={onSelectRepo}
              onOpenFolder={onOpenFolder}
              onCopyNotice={onCopyNotice}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </div>
      )}
    </div>
  );
}

