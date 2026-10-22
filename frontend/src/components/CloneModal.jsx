import { api } from '../api';
import React, { useState } from 'react';
import { X, Download, FolderCheck, Loader2, Folder } from 'lucide-react';

export default function CloneModal({ repo, isOpen, onClose, onCloneSuccess, onOpenFolder, baseDir }) {
  // Hooks antes do return condicional; o App remonta o modal por repo (key), entao o estado nasce certo
  const [folderName, setFolderName] = useState(repo?.pretty_name || repo?.name || '');
  const [isCloning, setIsCloning] = useState(false);
  const [cloneResult, setCloneResult] = useState(null);
  const [errorMsg, setErrorMsg] = useState('');

  if (!isOpen || !repo) return null;

  const targetPath = `${baseDir || 'E:\\REPOSITORIOS'}\\${folderName || 'repositorio'}`;

  const handleStartClone = async () => {
    if (!folderName.trim()) {
      setErrorMsg('Por favor, informe um nome para a pasta.');
      return;
    }

    setIsCloning(true);
    setErrorMsg('');

    try {
      const data = await api.cloneRepo(repo.url, repo.name, folderName.trim());
      if (data.success) {
        setCloneResult(data);
        if (onCloneSuccess) {
          onCloneSuccess(repo.repo_id, data.folder_name, data.folder_path);
        }
      } else {
        setErrorMsg(data.message || 'Falha ao clonar repositório.');
      }
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsCloning(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <div style={{
              width: 36,
              height: 36,
              borderRadius: 8,
              background: 'rgba(16, 185, 129, 0.15)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: '#34d399'
            }}>
              <Download size={20} />
            </div>
            <div>
              <h3 className="modal-title">Baixar Repositório</h3>
              <p style={{ fontSize: '13px', color: 'var(--text-muted)' }}>
                Clonar diretamente para <strong>{baseDir || 'E:\\REPOSITORIOS'}</strong>
              </p>
            </div>
          </div>

          <button className="modal-close-btn" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        {/* Repo Information */}
        <div style={{
          background: 'rgba(255, 255, 255, 0.03)',
          border: '1px solid var(--border-subtle)',
          borderRadius: 'var(--radius-md)',
          padding: '14px 16px',
          marginBottom: '20px'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '6px' }}>
            <span style={{ fontSize: '15px', fontWeight: 700, color: 'var(--text-main)' }}>
              {repo.repo_id}
            </span>
            <span style={{ fontSize: '12px', color: 'var(--accent-cyan)' }}>
              {repo.language} • {repo.stars_today > 0 ? `+${repo.stars_today.toLocaleString('pt-BR')} hoje` : 'Sem dado de estrelas hoje'}
            </span>
          </div>
          <p style={{ fontSize: '13px', color: 'var(--text-muted)', lineHeight: '1.4' }}>
            {repo.description || 'Sem descrição fornecida.'}
          </p>
        </div>

        {/* Clone Form */}
        {!cloneResult ? (
          <div>
            <div style={{ marginBottom: '18px' }}>
              <label className="input-label" htmlFor="folder-input">
                Nome da pasta no diret?rio configurado:
              </label>
              <input
                id="folder-input"
                type="text"
                className="input-field"
                value={folderName}
                onChange={(e) => setFolderName(e.target.value)}
                placeholder="Ex: FastAPI, LangChain, AutoGPT..."
                disabled={isCloning}
                autoFocus
              />
              <div className="path-preview">
                Destino no disco: {targetPath}
              </div>
            </div>

            {errorMsg && (
              <div style={{
                background: 'rgba(244, 63, 94, 0.1)',
                border: '1px solid rgba(244, 63, 94, 0.3)',
                color: '#fb7185',
                padding: '10px 14px',
                borderRadius: 'var(--radius-md)',
                fontSize: '13px',
                marginBottom: '16px'
              }}>
                {errorMsg}
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '24px' }}>
              <button 
                type="button" 
                className="btn btn-secondary" 
                onClick={onClose}
                disabled={isCloning}
              >
                Cancelar
              </button>
              <button 
                type="button" 
                className="btn btn-download" 
                onClick={handleStartClone}
                disabled={isCloning}
              >
                {isCloning ? (
                  <>
                    <Loader2 size={16} className="animate-spin" />
                    <span>Baixando Repositório...</span>
                  </>
                ) : (
                  <>
                    <Download size={16} />
                    <span>Confirmar e Baixar</span>
                  </>
                )}
              </button>
            </div>
          </div>
        ) : (
          /* Success Screen */
          <div style={{ textAlign: 'center', padding: '10px 0 10px' }}>
            <div style={{
              width: 56,
              height: 56,
              borderRadius: '50%',
              background: 'rgba(16, 185, 129, 0.15)',
              color: '#34d399',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              margin: '0 auto 16px'
            }}>
              <FolderCheck size={30} />
            </div>

            <h4 style={{ fontSize: '18px', fontWeight: 700, marginBottom: '6px' }}>
              {cloneResult.already_exists ? 'Repositório Já Disponível!' : 'Repositório Baixado com Sucesso!'}
            </h4>
            <p style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '16px' }}>
              {cloneResult.message}
            </p>

            <div className="path-preview" style={{ marginBottom: '24px', textAlign: 'left' }}>
              {cloneResult.folder_path}
            </div>

            <div style={{ display: 'flex', justifyContent: 'center', gap: '12px' }}>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => onOpenFolder(cloneResult.folder_path)}
              >
                <Folder size={16} />
                <span>Abrir Pasta no Windows</span>
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={onClose}
              >
                Concluido
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
