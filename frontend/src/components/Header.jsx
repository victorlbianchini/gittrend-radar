import React from 'react';
import { RefreshCw, FolderGit2, Cpu, Terminal } from 'lucide-react';
import GitRadarLogo from './GitRadarLogo';

export default function Header({ aiStatus, _syncStatus, onTriggerSync, isSyncing, onOpenReposDir, githubDir = 'E:\\REPOSITORIOS' }) {
  const isOllama = aiStatus?.ollama_online;
  const currentModel = aiStatus?.default_model || 'sem modelo de texto';

  return (
    <header className="header-bar-luxury">
      {/* Brand Identity with High-Tech Git Branch Logo */}
      <div className="brand-identity-container">
        <GitRadarLogo size={48} animated={true} />

        <div className="brand-text-block">
          <div className="brand-title-row">
            <h1 className="brand-title-luxury">
              <span className="brand-name-primary">GitTrend</span>
              <span className="brand-name-accent">Radar</span>
            </h1>
            <span className="brand-version-badge">2026 PRO</span>
          </div>

          <div className="brand-meta-row">
            <div className="meta-item">
              <span className="meta-dot neon-cyan" />
              <span className="meta-label">Radar Inteligente do GitHub</span>
            </div>
            <span className="meta-separator">•</span>
            <button type="button" className="meta-item path-item" onClick={onOpenReposDir} title={`Clique para abrir ${githubDir}`} style={{ background: 'none', border: 0, cursor: 'pointer' }}>
              <Terminal size={11} className="meta-icon" />
              <span className="meta-path">{githubDir}</span>
            </button>
          </div>
        </div>
      </div>

      {/* Header Actions & Controls */}
      <div className="header-controls-group">
        {/* AI Engine Status Pill */}
        <div 
          className={`ai-status-capsule ${isOllama ? 'is-ollama' : 'is-heuristic'}`}
          title={isOllama ? `Ollama Local Ativo (${currentModel})` : 'Inicie o Ollama para traduzir e resumir os itens com IA'}
        >
          <div className="capsule-glow-dot" />
          <Cpu size={13} className="capsule-icon" />
          <span className="capsule-text">
            {isOllama ? 'Resumos rápidos' : 'Resumos pausados'}
          </span>
          <span className="capsule-subtext">PT-BR</span>
        </div>

        {/* Quick Open Local Directory */}
        <button 
          type="button"
          className="btn-luxury-secondary"
          onClick={onOpenReposDir}
          title={`Abrir ${githubDir} no Windows Explorer`}
        >
          <FolderGit2 size={15} className="btn-icon" />
          <span>{githubDir}</span>
        </button>

        {/* Sync Now Action */}
        <button 
          type="button"
          className="btn-luxury-primary"
          onClick={onTriggerSync}
          disabled={isSyncing}
          title="Sincronizar tendências do GitHub e atualizar inteligência"
        >
          <RefreshCw size={14} className={`btn-icon ${isSyncing ? 'animate-spin' : ''}`} />
          <span>{isSyncing ? 'Sincronizando...' : 'Atualizar Trends'}</span>
        </button>
      </div>
    </header>
  );
}
