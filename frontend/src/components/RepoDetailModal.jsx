import { api } from '../api';
import React, { useState } from 'react';
import useSummaries from '../hooks/useSummaries';
import { 
  X, ExternalLink, Download, Check, Copy, Code2, Bookmark,
  CheckCircle2, AlertTriangle, Cpu, Loader2, Terminal, Tag, Folder
} from 'lucide-react';

export default function RepoDetailModal({ 
  repo, 
  isOpen, 
  onClose, 
  onCloneClick, 
  onOpenFolder,
  onCopyNotice,
  onToggleFavorite
}) {
  // Hooks sempre antes do return condicional (regra do React)
  const [activeTab, setActiveTab] = useState('ia_overview');
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedClone, setCopiedClone] = useState(false);
  const [isReanalyzing, setIsReanalyzing] = useState(false);
  const [analysisData, setAnalysisData] = useState(repo?.analysis || null);
  const summaries = useSummaries(isOpen && repo ? [{ key: `github:${repo.repo_id}` }] : []);

  if (!isOpen || !repo) return null;

  const rawAnalysis = analysisData || repo.analysis || {};
  const record = summaries.records[`github:${repo.repo_id}`];
  const analysis = record?.status === 'ready' ? { ...rawAnalysis, simple_summary: record.summary, description_pt: record.description_pt, analysis_source: `ollama_${record.model}` } : rawAnalysis;

  const handleCopyCode = (text) => {
    if (!text) return;
    navigator.clipboard.writeText(text);
    setCopiedCode(true);
    if (onCopyNotice) onCopyNotice('Código copiado!');
    setTimeout(() => setCopiedCode(false), 2000);
  };

  const handleCopyClone = () => {
    const cmd = `git clone ${repo.url}.git`;
    navigator.clipboard.writeText(cmd);
    setCopiedClone(true);
    if (onCopyNotice) onCopyNotice('Comando git clone copiado!');
    setTimeout(() => setCopiedClone(false), 2000);
  };

  const handleReanalyzeWithOllama = async () => {
    setIsReanalyzing(true);
    try {
      const analysis = await api.analyzeOllama(repo.repo_id);
      setAnalysisData(analysis);
      if (onCopyNotice) onCopyNotice('Reanálise de IA concluída!');
    } catch (err) {
      if (onCopyNotice) onCopyNotice(`Falha na reanálise: ${err}`);
    } finally {
      setIsReanalyzing(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" style={{ maxWidth: 780 }} onClick={(e) => e.stopPropagation()}>
        {/* Modal Header */}
        <div className="modal-header">
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '6px' }}>
              <span style={{ fontSize: '13px', color: 'var(--text-muted)' }}>{repo.owner} /</span>
              <h3 className="modal-title" style={{ display: 'inline', color: 'var(--text-main)' }}>
                {repo.name}
              </h3>
              {repo.is_downloaded && (
                <span className="local-badge" style={{ marginLeft: 6 }}>
                  Na máquina
                </span>
              )}
            </div>
            <p style={{ fontSize: '13.5px', color: 'var(--text-muted)', lineHeight: '1.45' }}>
              {repo.description || 'Repositório GitHub em alta'}
            </p>
          </div>

          <button className="modal-close-btn" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        {/* Action & Links Bar */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          background: 'rgba(255, 255, 255, 0.025)',
          border: '1px solid var(--border-glass)',
          borderRadius: 'var(--radius-md)',
          padding: '12px 18px',
          marginBottom: '22px',
          flexWrap: 'wrap',
          gap: '12px'
        }}>
          {/* Metrics */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '16px', fontSize: '13px' }}>
            <span>Linguagem: <strong style={{ color: 'var(--accent-cyan)' }}>{repo.language}</strong></span>
            <span>Hoje: <strong style={{ color: '#38bdf8' }}>{repo.stars_today > 0 ? `+${repo.stars_today.toLocaleString('pt-BR')}` : 'Não informado'}</strong></span>
            <span>Total: <strong>{repo.stars?.toLocaleString()}</strong></span>
          </div>

          {/* Direct Link Buttons */}
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {/* Direct GitHub Link */}
            <a
              href={repo.url}
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-secondary"
              style={{ padding: '8px 14px', fontSize: '12px' }}
            >
              <ExternalLink size={14} />
              <span>Abrir no GitHub</span>
            </a>

            {/* Releases Link */}
            <a
              href={`${repo.url}/releases`}
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-secondary"
              style={{ padding: '8px 12px', fontSize: '12px' }}
              title="Ver versões lançadas no GitHub"
            >
              <Tag size={13} />
              <span>Releases</span>
            </a>

            {/* Copy Clone Command */}
            <button
              type="button"
              className="btn btn-secondary"
              style={{ padding: '8px 12px', fontSize: '12px' }}
              onClick={handleCopyClone}
              title="Copiar comando git clone"
            >
              {copiedClone ? <Check size={14} style={{ color: '#34d399' }} /> : <Terminal size={13} />}
              <span>{copiedClone ? 'Copiado!' : 'Clone Cmd'}</span>
            </button>

            {onToggleFavorite && (
              <button
                type="button"
                className={`btn btn-secondary fav-toggle ${repo.is_favorite ? 'is-fav' : ''}`}
                style={{ padding: '8px 14px', fontSize: '12px' }}
                onClick={() => onToggleFavorite(repo)}
              >
                <Bookmark size={13} fill={repo.is_favorite ? 'currentColor' : 'none'} />
                <span>{repo.is_favorite ? 'Nos favoritos' : 'Favoritar'}</span>
              </button>
            )}

            {/* Local or Download Action */}
            {repo.is_downloaded ? (
              <button
                type="button"
                className="btn btn-primary"
                style={{ padding: '8px 14px', fontSize: '12px' }}
                onClick={() => onOpenFolder(repo.downloaded_path)}
              >
                <Folder size={14} />
                <span>Abrir Pasta Local</span>
              </button>
            ) : (
              <button
                type="button"
                className="btn btn-download-glow"
                style={{ padding: '8px 16px', fontSize: '12px' }}
                onClick={() => onCloneClick(repo)}
              >
                <Download size={14} />
                <span>Baixar repositório</span>
              </button>
            )}
          </div>
        </div>

        {/* Navigation Tabs */}
        <div style={{ 
          display: 'flex', 
          gap: '8px', 
          borderBottom: '1px solid var(--border-glass)', 
          paddingBottom: '14px', 
          marginBottom: '22px' 
        }}>
          <button
            type="button"
            className={`tab-btn ${activeTab === 'ia_overview' ? 'active' : ''}`}
            onClick={() => setActiveTab('ia_overview')}
          >
            <span>Explicação com IA</span>
          </button>
          <button
            type="button"
            className={`tab-btn ${activeTab === 'code' ? 'active' : ''}`}
            onClick={() => setActiveTab('code')}
          >
            <Code2 size={14} />
            <span>Exemplo de Código</span>
          </button>
          <button
            type="button"
            className={`tab-btn ${activeTab === 'decision' ? 'active' : ''}`}
            onClick={() => setActiveTab('decision')}
          >
            <CheckCircle2 size={14} />
            <span>Quando Selecionar</span>
          </button>
        </div>

        {/* Tab 1: AI Explanation Overview */}
        {activeTab === 'ia_overview' && (
          <div>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '14px' }}>
              <div className="ai-powered-tag">
                <span>Análise Técnica com IA</span>
              </div>

              <button
                type="button"
                className="btn btn-secondary"
                style={{ padding: '7px 14px', fontSize: '12px' }}
                onClick={handleReanalyzeWithOllama}
                disabled={isReanalyzing}
              >
                {isReanalyzing ? (
                  <>
                    <Loader2 size={13} className="animate-spin" />
                    <span>Processando no Ollama...</span>
                  </>
                ) : (
                  <>
                    <Cpu size={13} />
                    <span>Reanalisar via Ollama Local</span>
                  </>
                )}
              </button>
            </div>

            <div style={{ marginBottom: '20px' }}>
              <h4 style={{ fontSize: '14px', fontWeight: 800, color: '#c084fc', marginBottom: '8px' }}>
                O que este projeto faz:
              </h4>
              <p style={{ fontSize: '14.5px', color: '#e2e8f0', lineHeight: '1.65', whiteSpace: 'pre-line' }}>
                {analysis.simple_summary || repo.simple_summary || repo.description || 'Análise em processamento.'}
              </p>
            </div>

            {analysis.why_trending && (
              <div style={{ marginBottom: '20px' }}>
                <h4 style={{ fontSize: '14px', fontWeight: 800, color: 'var(--accent-cyan)', marginBottom: '8px' }}>
                  Por que está em alta no GitHub:
                </h4>
                <p style={{ fontSize: '14px', color: 'var(--text-muted)', lineHeight: '1.55' }}>
                  {analysis.why_trending}
                </p>
              </div>
            )}

            {analysis.quick_verdict && (
              <div style={{
                background: 'rgba(99, 102, 241, 0.08)',
                border: '1px solid rgba(99, 102, 241, 0.28)',
                borderRadius: 'var(--radius-md)',
                padding: '14px 18px',
                marginTop: '18px'
              }}>
                <h5 style={{ fontSize: '11px', fontWeight: 800, color: '#a5b4fc', textTransform: 'uppercase', marginBottom: '4px', letterSpacing: '0.05em' }}>
                  Veredito da IA:
                </h5>
                <p style={{ fontSize: '13.5px', color: '#ffffff', fontWeight: 600 }}>
                  {analysis.quick_verdict}
                </p>
              </div>
            )}
          </div>
        )}

        {/* Tab 2: Code Example */}
        {activeTab === 'code' && (
          <div>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-muted)' }}>
                Comando / Exemplo de Execução:
              </span>
              <button
                type="button"
                className="btn btn-secondary"
                style={{ padding: '6px 12px', fontSize: '12px' }}
                onClick={() => handleCopyCode(analysis.code_example || analysis.install_cmd)}
              >
                {copiedCode ? <Check size={14} style={{ color: '#34d399' }} /> : <Copy size={14} />}
                <span>{copiedCode ? 'Copiado!' : 'Copiar'}</span>
              </button>
            </div>

            <pre className="code-block">
              {analysis.code_example || analysis.install_cmd || `git clone ${repo.url}\ncd ${repo.name}`}
            </pre>

            {analysis.install_cmd && (
              <div style={{ marginTop: '16px' }}>
                <span style={{ fontSize: '12px', color: 'var(--text-dim)' }}>Comando rápido:</span>
                <div style={{
                  background: '#04060a',
                  border: '1px solid var(--border-glass)',
                  padding: '10px 14px',
                  borderRadius: 'var(--radius-sm)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '12.5px',
                  marginTop: '6px',
                  color: 'var(--accent-cyan)'
                }}>
                  {analysis.install_cmd}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Tab 3: Decision Guide */}
        {activeTab === 'decision' && (
          <div>
            <div style={{ marginBottom: '22px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: '#34d399', marginBottom: '10px' }}>
                <CheckCircle2 size={17} />
                <h4 style={{ fontSize: '14.5px', fontWeight: 800 }}>Quando Selecionar:</h4>
              </div>
              <ul style={{ paddingLeft: '22px', fontSize: '13.5px', color: '#cbd5e1', lineHeight: '1.65' }}>
                {(analysis.when_to_select || [
                  'Você precisa acelerar o desenvolvimento com uma biblioteca testada pela comunidade.',
                  'Seu projeto demanda performance e código de alta qualidade.',
                  'Você quer testar uma tecnologia emergente antes que se torne padrão corporativo.'
                ]).map((item, idx) => (
                  <li key={idx} style={{ marginBottom: '8px' }}>{item}</li>
                ))}
              </ul>
            </div>

            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: '#fb7185', marginBottom: '10px' }}>
                <AlertTriangle size={17} />
                <h4 style={{ fontSize: '14.5px', fontWeight: 800 }}>Quando Evitar:</h4>
              </div>
              <ul style={{ paddingLeft: '22px', fontSize: '13.5px', color: '#94a3b8', lineHeight: '1.65' }}>
                {(analysis.when_to_avoid || [
                  'Se seu projeto possui requisitos corporativos estritos de suporte de longo prazo.',
                  'Se o escopo é muito básico e funções nativas da linguagem já atendem.'
                ]).map((item, idx) => (
                  <li key={idx} style={{ marginBottom: '8px' }}>{item}</li>
                ))}
              </ul>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
