import { useCallback, useEffect, useState } from 'react';
import { Lightbulb, RefreshCw, HardDriveDownload, CheckCircle2, FolderSearch, TriangleAlert, CircleCheck, FileWarning } from 'lucide-react';
import { api } from '../api';

const DOMAIN_COLORS = {
  'Voz e STT': 'violet',
  'Agentes de IA': 'emerald',
  'Rust e Desktop': 'amber',
  Go: 'cyan',
  Frontend: 'sky',
  'Python Core': 'yellow',
  'Infra Self-hosted': 'green',
  'Segurança e RE': 'red',
  'WhatsApp e Marketing': 'lime',
  'Vídeo e Criativo': 'orange',
  'Documentos e RAG': 'blue',
  'Trading e Finanças': 'gold',
};

export default function StackSuggestionsView({ onNotice, onChanged }) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [requesting, setRequesting] = useState(new Set());
  const [fixingLfs, setFixingLfs] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      setData(await api.stackAnalysis());
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    api.stackAnalysis()
      .then((result) => { if (active) setData(result); })
      .catch((err) => { if (active) setError(String(err)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  async function request(suggestion) {
    setRequesting((set) => new Set(set).add(suggestion.id));
    try {
      const id = await api.requestStackDownload(suggestion.kind, suggestion.id, `Sugestão de stack (${suggestion.domain}): ${suggestion.reason}`);
      onNotice('info', 'Pedido registrado', `${suggestion.id} entrou na fila de Pedidos dos agentes (id ${id}).`);
      setData((current) => current && {
        ...current,
        suggestions: current.suggestions.map((s) => (s.id === suggestion.id ? { ...s, already_requested: true } : s)),
      });
      if (onChanged) onChanged();
    } catch (err) {
      onNotice('error', 'Falha ao pedir download', String(err));
    } finally {
      setRequesting((set) => { const next = new Set(set); next.delete(suggestion.id); return next; });
    }
  }

  async function fixLfs() {
    setFixingLfs(true);
    try {
      const report = await api.fixLfs();
      onNotice(report.errors.length ? 'info' : 'success', 'Git LFS', report.message);
      await load();
    } catch (err) {
      onNotice('error', 'Falha ao corrigir Git LFS', String(err));
    } finally {
      setFixingLfs(false);
    }
  }

  if (loading) {
    return <section className="glass-panel catalog-v2-empty">
      <RefreshCw size={18} className="spin" /> Analisando a stack dos repositórios baixados...
    </section>;
  }
  if (error) {
    return <section className="glass-panel catalog-v2-empty" role="alert">
      {error}
      <div style={{ marginTop: 12 }}><button type="button" className="btn btn-primary" onClick={load}>Tentar de novo</button></div>
    </section>;
  }

  const { suggestions = [], lfs_folders: lfsFolders = [], scanned_folders: scanned, omitted_downloaded: omitted } = data || {};
  const requestedCount = suggestions.filter((s) => s.already_requested).length;

  return <section className="stack-suggestions">
    <div className="stack-summary glass-panel">
      <div className="stack-summary-item"><FolderSearch size={16} /><span><strong>{scanned}</strong> pastas analisadas no disco</span></div>
      <div className="stack-summary-item"><Lightbulb size={16} /><span><strong>{suggestions.length}</strong> sugestões do conhecimento do ecossistema</span></div>
      <div className="stack-summary-item"><CircleCheck size={16} /><span><strong>{requestedCount}</strong> já na fila de aprovação</span></div>
      {omitted > 0 && <div className="stack-summary-item"><CheckCircle2 size={16} /><span><strong>{omitted}</strong> sugestões omitidas por já estarem baixadas</span></div>}
      <button type="button" className="btn btn-secondary stack-reload" onClick={load} title="Reanalisar o disco"><RefreshCw size={14} /> Reanalisar</button>
    </div>

    {lfsFolders.length > 0 && <div className="stack-lfs glass-panel" role="status">
      <div className="stack-lfs-title"><TriangleAlert size={16} /> Git LFS pendente em {lfsFolders.length} pasta(s)</div>
      <p className="stack-lfs-text">
        Estes repositórios têm arquivos baixados como ponteiros (o git-lfs não estava instalado quando foram clonados): {lfsFolders.slice(0, 6).join(', ')}{lfsFolders.length > 6 ? ` e mais ${lfsFolders.length - 6}` : ''}.
      </p>
      <button type="button" className="btn btn-primary" onClick={fixLfs} disabled={fixingLfs}>
        {fixingLfs ? <RefreshCw size={14} className="spin" /> : <FileWarning size={14} />} Instalar Git LFS e corrigir
      </button>
    </div>}

    {!suggestions.length ? <div className="glass-panel catalog-v2-empty">Nenhuma sugestão nova: o disco já está completo para o conhecimento atual.</div> : <div className="stack-grid">
      {suggestions.map((s) => <article key={`${s.kind}:${s.id}`} className="stack-card glass-panel">
        <header className="stack-card-head">
          <span className={`stack-domain chip-domain-${DOMAIN_COLORS[s.domain] || 'gray'}`}>{s.domain}</span>
          <span className={`stack-kind ${s.kind === 'model' ? 'kind-model' : 'kind-repo'}`}>{s.kind === 'model' ? 'Modelo HF' : 'GitHub'}</span>
        </header>
        <h3 className="stack-card-title" title={s.id}>{s.name}</h3>
        <code className="stack-card-id">{s.id}</code>
        <p className="stack-card-reason">{s.reason}</p>
        {s.from_folders.length > 0 && <p className="stack-card-from">A partir de: <strong>{s.from_folders.slice(0, 3).join(', ')}</strong>{s.from_folders.length > 3 ? ` (+${s.from_folders.length - 3})` : ''}</p>}
        <footer className="stack-card-actions">
          {s.already_requested
            ? <span className="stack-requested"><CheckCircle2 size={14} /> Já pedido</span>
            : <button type="button" className="btn btn-primary" disabled={requesting.has(s.id)} onClick={() => request(s)}>
              <HardDriveDownload size={14} /> Pedir download
            </button>}
        </footer>
      </article>)}
    </div>}
  </section>;
}
