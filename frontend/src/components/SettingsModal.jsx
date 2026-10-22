import React, { useEffect, useRef, useState } from "react";
import { FolderOpen, Loader2, X } from "lucide-react";
import { api } from "../api";

const DIRECTORIES = [
  ["githubDir", "Repositórios GitHub", "github_dir"],
  ["hfDir", "Modelos Hugging Face", "hf_dir"],
  ["skillsDir", "Skills", "skills_dir"],
];

export default function SettingsModal({ isOpen, onClose, onSaved }) {
  const [form, setForm] = useState({ githubDir: "", hfDir: "", skillsDir: "" });
  const [token, setToken] = useState("");
  const [removeToken, setRemoveToken] = useState(false);
  const [tokenHint, setTokenHint] = useState("");
  const [loading, setLoading] = useState(true);
  const [ready, setReady] = useState(false);
  const [saving, setSaving] = useState(false);
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState("");
  const panel = useRef(null);

  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    const previousFocus = document.activeElement;
    panel.current?.focus();
    Promise.resolve().then(() => {
      if (!active) return null;
      setLoading(true);
      setReady(false);
      setError("");
      setToken("");
      setRemoveToken(false);
      return api.settings();
    }).then((settings) => {
      if (!active) return;
      setReady(true);
      setForm(Object.fromEntries(DIRECTORIES.map(([key, , source]) => [key, settings[source] || ""])));
      setTokenHint(settings.token_hint || "Nenhum token configurado");
    }).catch((err) => { if (active) setError(String(err)); })
      .finally(() => { if (active) setLoading(false); });
    return () => {
      active = false;
      previousFocus?.focus?.();
    };
  }, [isOpen]);

  if (!isOpen) return null;
  const busy = loading || saving || picking;

  async function pickDirectory(key) {
    setPicking(true);
    setError("");
    try {
      const path = await api.pickFolder();
      if (path) setForm((current) => ({ ...current, [key]: path }));
    } catch (err) { setError(String(err)); }
    finally { setPicking(false); }
  }

  async function save(event) {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      const settings = await api.saveSettings({ ...form, token: removeToken ? "" : token.trim() || null });
      onSaved?.(settings);
      onClose();
    } catch (err) { setError(String(err)); }
    finally { setSaving(false); }
  }

  function handleKeys(event) {
    if (event.key === "Escape" && !busy) { event.stopPropagation(); onClose(); }
    if (event.key !== "Tab") return;
    const controls = panel.current.querySelectorAll('button:not(:disabled), input:not(:disabled), [tabindex="0"]');
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (!first) { event.preventDefault(); return; }
    if (event.shiftKey && (document.activeElement === first || document.activeElement === panel.current)) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus();
    }
  }

  return (
    <div className="modal-overlay">
      <div className="modal-content" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabIndex={-1} ref={panel} onKeyDown={handleKeys}>
        <div className="modal-header">
          <h2 className="modal-title" id="settings-title">Configurações</h2>
          <button className="modal-close-btn" type="button" aria-label="Fechar configurações" onClick={onClose} disabled={busy}><X size={20} /></button>
        </div>
        {loading ? <p role="status">Carregando configurações…</p> : ready && (
          <form onSubmit={save} style={{ display: "grid", gap: 20 }}>
            {DIRECTORIES.map(([key, label]) => (
              <div key={key}>
                <label htmlFor={`settings-${key}`}>{label}</label>
                <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
                  <input className="input-field" id={`settings-${key}`} required value={form[key]} disabled={busy} onChange={(event) => setForm({ ...form, [key]: event.target.value })} />
                  <button className="btn btn-secondary" type="button" aria-label={`Escolher pasta: ${label}`} onClick={() => pickDirectory(key)} disabled={busy}><FolderOpen size={18} /></button>
                </div>
              </div>
            ))}
            <div>
              <label htmlFor="settings-token">Token do GitHub</label>
              <p id="settings-token-hint" style={{ color: "var(--text-muted)", margin: "8px 0" }}>{tokenHint}. Deixe em branco para manter o token atual.</p>
              <input className="input-field" id="settings-token" type="password" autoComplete="new-password" value={token} aria-describedby="settings-token-hint" disabled={busy || removeToken} onChange={(event) => setToken(event.target.value)} />
              <label style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <input type="checkbox" checked={removeToken} disabled={busy} onChange={(event) => setRemoveToken(event.target.checked)} />Remover token salvo
              </label>
            </div>
            <button className="btn btn-primary" type="submit" disabled={busy}>{saving && <Loader2 size={16} className="animate-spin" />}Salvar configurações</button>
          </form>
        )}
        {error && <p role="alert" style={{ color: "#fca5a5", marginTop: 16, overflowWrap: "anywhere" }}>{error}</p>}
      </div>
    </div>
  );
}
