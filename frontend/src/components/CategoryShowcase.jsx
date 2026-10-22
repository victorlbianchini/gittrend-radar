import React from 'react';
import { 
  Brain, Layout, Server, Terminal, Database, Boxes, ShieldCheck, Smartphone
} from 'lucide-react';
import RepoCard from './RepoCard';

const categoryMeta = {
  ia_agentes: {
    icon: Brain,
    color: '#c084fc',
    bg: 'rgba(168, 85, 247, 0.12)',
    border: 'rgba(168, 85, 247, 0.35)',
    glow: 'rgba(168, 85, 247, 0.25)',
    tag: 'AGENTS & LLMs'
  },
  frontend_ui: {
    icon: Layout,
    color: '#38bdf8',
    bg: 'rgba(56, 189, 248, 0.12)',
    border: 'rgba(56, 189, 248, 0.35)',
    glow: 'rgba(56, 189, 248, 0.25)',
    tag: 'UI & DESIGN SYSTEMS'
  },
  backend_api: {
    icon: Server,
    color: '#34d399',
    bg: 'rgba(52, 211, 153, 0.12)',
    border: 'rgba(52, 211, 153, 0.35)',
    glow: 'rgba(52, 211, 153, 0.25)',
    tag: 'HIGH PERF & RUNTIMES'
  },
  devops_cli: {
    icon: Terminal,
    color: '#fb923c',
    bg: 'rgba(251, 146, 60, 0.12)',
    border: 'rgba(251, 146, 60, 0.35)',
    glow: 'rgba(251, 146, 60, 0.25)',
    tag: 'CLI & INFRASTRUCTURE'
  },
  data_eng: {
    icon: Database,
    color: '#818cf8',
    bg: 'rgba(129, 140, 248, 0.12)',
    border: 'rgba(129, 140, 248, 0.35)',
    glow: 'rgba(129, 140, 248, 0.25)',
    tag: 'DATA & STORAGE'
  },
  security: {
    icon: ShieldCheck,
    color: '#f87171',
    bg: 'rgba(248, 113, 113, 0.12)',
    border: 'rgba(248, 113, 113, 0.35)',
    glow: 'rgba(248, 113, 113, 0.25)',
    tag: 'SECURITY & PRIVACY'
  },
  mobile: {
    icon: Smartphone,
    color: '#4ade80',
    bg: 'rgba(74, 222, 128, 0.12)',
    border: 'rgba(74, 222, 128, 0.35)',
    glow: 'rgba(74, 222, 128, 0.25)',
    tag: 'iOS & ANDROID'
  },
  geral: {
    icon: Boxes,
    color: '#facc15',
    bg: 'rgba(250, 204, 21, 0.12)',
    border: 'rgba(250, 204, 21, 0.35)',
    glow: 'rgba(250, 204, 21, 0.25)',
    tag: 'ECOSYSTEM & UTILS'
  }
};

export default function CategoryShowcase({ 
  categories, 
  onCloneClick, 
  onSelectRepo, 
  onOpenFolder,
  onCopyNotice,
  onToggleFavorite
}) {
  if (!categories || categories.length === 0) return null;

  return (
    <div className="category-showcase-flow">
      {categories.map((catGroup) => {
        const meta = categoryMeta[catGroup.category] || categoryMeta.geral;
        const IconComponent = meta.icon;
        const repos = catGroup.top_repositories || [];
        if (repos.length === 0) return null;

        return (
          <section key={catGroup.category} className="category-deck-section">
            {/* Section Header with Holographic Category Emblem */}
            <div className="category-section-banner">
              <div className="category-header-main">
                <div 
                  className="category-emblem"
                  style={{
                    backgroundColor: meta.bg,
                    borderColor: meta.border,
                    color: meta.color,
                    boxShadow: `0 0 20px ${meta.glow}`
                  }}
                >
                  <IconComponent size={20} />
                </div>
                <div>
                  <div className="category-title-flex">
                    <h2 className="category-heading">{catGroup.title}</h2>
                    <span 
                      className="category-micro-tag"
                      style={{ color: meta.color, borderColor: meta.border, backgroundColor: meta.bg }}
                    >
                      {meta.tag}
                    </span>
                  </div>
                  <div className="category-breadcrumbs">
                    <span>Top {repos.length} em alta monitorados</span>
                    <span className="bread-dot">•</span>
                    <span className="bread-target">Download para a pasta GitHub configurada</span>
                  </div>
                </div>
              </div>

              <div className="category-radar-tag">
                <div className="radar-live-indicator" />
                <span>IA Explica Ativa</span>
              </div>
            </div>

            {/* Repositories 4-Card Luxury Grid */}
            <div className="repo-grid-luxury">
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

            {/* Subtle Divider Glow */}
            <div className="section-bottom-separator" />
          </section>
        );
      })}
    </div>
  );
}
