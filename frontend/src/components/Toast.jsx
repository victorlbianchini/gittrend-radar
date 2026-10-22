import React from 'react';
import { CheckCircle2, AlertCircle, Info, X } from 'lucide-react';

export default function Toast({ toasts, onDismiss }) {
  if (!toasts || toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((t) => {
        const isSuccess = t.type === 'success';
        const isError = t.type === 'error';

        return (
          <div key={t.id} className={`toast ${t.type}`}>
            {isSuccess && <CheckCircle2 size={18} style={{ color: '#34d399', flexShrink: 0 }} />}
            {isError && <AlertCircle size={18} style={{ color: '#fb7185', flexShrink: 0 }} />}
            {!isSuccess && !isError && <Info size={18} style={{ color: '#38bdf8', flexShrink: 0 }} />}

            <div style={{ flex: 1 }}>
              {t.title && <div style={{ fontWeight: 700, marginBottom: '2px' }}>{t.title}</div>}
              <div>{t.message}</div>
            </div>

            <button
              onClick={() => onDismiss(t.id)}
              style={{
                background: 'none',
                border: 'none',
                color: 'var(--text-muted)',
                cursor: 'pointer',
                padding: '2px',
                display: 'flex',
                alignItems: 'center'
              }}
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
