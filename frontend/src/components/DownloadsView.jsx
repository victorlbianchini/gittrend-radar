import { useEffect, useState } from 'react';
import { AlertCircle, Check, FolderOpen, Loader2 } from 'lucide-react';
import { api } from '../api';

const bytes = (n) => {
  if (!n) return '0 B';
  const unit = Math.min(3, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** unit).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} ${['B', 'KB', 'MB', 'GB'][unit]}`;
};

export default function DownloadsView({ onOpenFolder }) {
  const [jobs, setJobs] = useState([]);
  const [error, setError] = useState('');

  useEffect(() => {
    let active = true;
    let timer;
    const poll = async () => {
      try {
        const result = await api.downloads();
        if (active) {
          setJobs(result);
          setError('');
        }
      } catch (err) {
        if (active) setError(String(err));
      }
      if (active) timer = setTimeout(poll, 1000);
    };
    poll();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, []);

  return (
    <section className="glass-panel" style={{ padding: 24, marginTop: 12 }} aria-label="Downloads da sessão">
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
        <div>
          <h2 style={{ margin: 0, fontSize: '1.25rem', fontWeight: 600 }}>Downloads da Sessão</h2>
          <p style={{ color: 'var(--text-muted)', margin: '4px 0 0 0', fontSize: '0.875rem' }}>
            Modelos, datasets e skills baixados ou em andamento. Mantenha o app aberto até concluir.
          </p>
        </div>
      </div>

      {error && (
        <div className="card-download-status is-error" style={{ marginBottom: 16 }}>
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      )}

      {!jobs.length && (
        <div className="glass-panel catalog-v2-empty" style={{ margin: '16px 0', textAlign: 'center', padding: '32px 16px' }}>
          Nenhum download iniciado nesta sessão.
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
        {jobs.map((job) => {
          const pct = job.total_bytes > 0 ? Math.min(100, Math.round((job.done_bytes / job.total_bytes) * 100)) : 0;
          return (
            <article
              key={job.id}
              className={`repo-card-luxury ${job.finished && job.success ? 'is-downloaded' : ''}`}
              style={{ padding: 18, border: '1px solid var(--border-subtle)', borderRadius: 12 }}
            >
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap', marginBottom: 10 }}>
                <h3 style={{ margin: 0, fontSize: '1.05rem', fontWeight: 600, color: 'var(--text-primary)' }}>
                  {job.label}
                </h3>

                <div>
                  {job.finished ? (
                    job.success ? (
                      <span className="card-v2-chip card-v2-chip-downloaded">
                        <Check size={11} aria-hidden="true" />
                        Concluído
                      </span>
                    ) : (
                      <span className="card-v2-chip" style={{ background: 'rgba(239, 68, 68, 0.15)', color: '#f87171', borderColor: 'rgba(239, 68, 68, 0.3)' }}>
                        <AlertCircle size={11} aria-hidden="true" />
                        Falhou
                      </span>
                    )
                  ) : (
                    <span className="card-v2-chip card-v2-chip-accent">
                      <Loader2 size={11} className="animate-spin" aria-hidden="true" />
                      Baixando
                    </span>
                  )}
                </div>
              </div>

              {!job.finished && (
                <div className="card-download-progress-box" style={{ margin: '10px 0' }}>
                  <div className="card-download-progress-header">
                    <span className="card-download-progress-file">
                      <Loader2 size={12} className="animate-spin" />
                      {job.current_file || 'Transferindo dados…'}
                    </span>
                    <span className="card-download-progress-bytes">
                      {bytes(job.done_bytes)}
                      {job.total_bytes > 0 ? ` de ${bytes(job.total_bytes)} (${pct}%)` : ''}
                    </span>
                  </div>
                  {job.total_bytes > 0 ? (
                    <div className="card-download-bar">
                      <div className="card-download-fill" style={{ width: `${Math.max(3, pct)}%` }} />
                    </div>
                  ) : (
                    <div className="card-download-bar is-indeterminate">
                      <div className="card-download-fill" />
                    </div>
                  )}
                </div>
              )}

              {job.finished && job.success && (
                <div className="card-download-status is-success" style={{ margin: '8px 0 12px 0' }}>
                  <Check size={14} />
                  <span>Download concluído com sucesso ({bytes(job.done_bytes)} salvos).</span>
                </div>
              )}

              {job.finished && !job.success && (
                <div className="card-download-status is-error" style={{ margin: '8px 0 12px 0' }}>
                  <AlertCircle size={14} />
                  <span>{job.message || 'Falha ao concluir o download.'}</span>
                </div>
              )}

              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, marginTop: 8, flexWrap: 'wrap' }}>
                <p style={{ margin: 0, fontSize: '0.8rem', color: 'var(--text-muted)', overflowWrap: 'anywhere' }}>
                  {job.folder_path}
                </p>
                {job.finished && job.success && onOpenFolder && (
                  <button
                    type="button"
                    className="btn-open-folder-luxury"
                    onClick={() => onOpenFolder(job.folder_path)}
                  >
                    <FolderOpen size={13} />
                    Abrir pasta
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}

