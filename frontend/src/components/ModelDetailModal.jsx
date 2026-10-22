import React, { useEffect, useRef, useState } from "react";
import { Download, ExternalLink, X } from "lucide-react";
import { api } from "../api";
import useSummaries from '../hooks/useSummaries';

function bytes(value) {
  if (!value) return "tamanho não informado";
  const unit = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** unit).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} ${["B", "KB", "MB", "GB", "TB"][unit]}`;
}

export default function ModelDetailModal({ model, onClose, onNotice }) {
  const [explanation, setExplanation] = useState(null);
  const [files, setFiles] = useState(null);
  const [explanationError, setExplanationError] = useState("");
  const [filesError, setFilesError] = useState("");
  const [downloadBusy, setDownloadBusy] = useState(false);
  const [downloadError, setDownloadError] = useState("");
  const [notice, setNotice] = useState("");
  const dialog = useRef(null);
  const summaries = useSummaries([{ key: `hf:${model.id}` }]);
  const aiSummary = summaries.records[`hf:${model.id}`];

  useEffect(() => {
    let active = true;
    api.explainModel(model).then((result) => { if (active) setExplanation(result); }).catch((err) => { if (active) setExplanationError(String(err)); });
    api.modelFiles(model.id).then((result) => { if (active) setFiles(result); }).catch((err) => { if (active) setFilesError(String(err)); });
    return () => { active = false; };
  }, [model]);

  useEffect(() => {
    const previous = document.activeElement;
    dialog.current?.focus();
    function handleKey(event) {
      if (event.key === "Escape") onClose();
      if (event.key !== "Tab") return;
      const elements = [...dialog.current.querySelectorAll('button:not(:disabled), a[href], [tabindex="0"]')];
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) {
        event.preventDefault(); last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault(); first?.focus();
      }
    }
    document.addEventListener("keydown", handleKey);
    return () => { document.removeEventListener("keydown", handleKey); previous?.focus(); };
  }, [onClose]);

  async function download(full) {
    setDownloadBusy(true);
    setDownloadError("");
    setNotice("");
    try {
      await api.downloadModel(model.id, full);
      const message = `Download ${full ? "completo" : "leve"} iniciado; acompanhe em Downloads.`;
      setNotice(message);
      onNotice?.(message);
    } catch (err) {
      setDownloadError(String(err));
    } finally {
      setDownloadBusy(false);
    }
  }

  return <div className="modal-overlay" onClick={onClose}>
    <div ref={dialog} role="dialog" aria-modal="true" aria-labelledby="model-detail-title" tabIndex={-1} className="modal-content" onClick={(event) => event.stopPropagation()} style={{ maxWidth: 850 }}>
      <div className="modal-header">
        <div><h3 id="model-detail-title" className="modal-title" style={{ overflowWrap: "anywhere" }}>{model.id}</h3><p style={{ color: "var(--text-muted)", marginTop: 8 }}>{model.task_label}</p></div>
        <button type="button" className="modal-close-btn" aria-label="Fechar detalhes do modelo" onClick={onClose}><X size={20} /></button>
      </div>
      <a href={model.url} className="btn-details-glass"><ExternalLink size={14} /> Abrir no Hugging Face</a>
      {!explanation && !explanationError && <p role="status" style={{ marginTop: 20 }}>Lendo informações do modelo...</p>}
      {explanationError && <p role="alert" style={{ marginTop: 20 }}>Não foi possível explicar o modelo: {explanationError}</p>}
      {explanation && <div style={{ lineHeight: 1.65, overflowWrap: "anywhere" }}>
        {(aiSummary?.summary || explanation.summary) && <p style={{ marginTop: 20, whiteSpace: "pre-wrap" }}>{aiSummary?.summary || explanation.summary}</p>}
        {explanation.why_trending && <section style={{ marginTop: 20 }}><h4>Por que está em alta</h4><p>{explanation.why_trending}</p></section>}
        {[["Principais recursos", explanation.key_features], ["Quando escolher", explanation.when_to_select], ["Quando evitar", explanation.when_to_avoid]].filter(([, items]) => items?.length).map(([title, items]) => <section key={title} style={{ marginTop: 20 }}><h4>{title}</h4><ul style={{ paddingLeft: 20 }}>{items.map((item, i) => <li key={i}>{item}</li>)}</ul></section>)}
        {explanation.verdict && <p style={{ marginTop: 20 }}>{explanation.verdict}</p>}
        {explanation.license && <p>Licença: {explanation.license}</p>}
        {explanation.base_model && <p>Modelo base: {explanation.base_model}</p>}
        {explanation.languages?.length > 0 && <p>Idiomas: {explanation.languages.join(", ")}</p>}
        {[["Instalação", explanation.install_cmd], ["Exemplo do model card", explanation.code_example]].filter(([, value]) => value).map(([title, value]) => <section key={title} style={{ marginTop: 20 }}><h4>{title}</h4><pre style={{ overflowX: "auto", padding: 16, background: "var(--bg-base)", borderRadius: 8, fontSize: 12 }}>{value}</pre></section>)}
      </div>}
      <section style={{ marginTop: 24, borderTop: "1px solid var(--border-glass)", paddingTop: 20 }}>
        <h4>Arquivos e download</h4>
        {!files && !filesError && <p role="status">Consultando arquivos e tamanhos...</p>}
        {filesError && <p role="alert">{filesError}</p>}
        {files && <>
          <p style={{ margin: "12px 0", color: "var(--text-muted)" }}>Leve: documentação, código e configuração, sem pesos. Completo: inclui os pesos e pode ocupar vários GB.</p>
          {files.gated && <p role="alert">Este modelo exige acesso autorizado no Hugging Face.</p>}
          <div style={{ display: "flex", flexWrap: "wrap", gap: 12, marginTop: 12 }}>
            <button type="button" className="btn btn-secondary" disabled={downloadBusy || files.light_count === 0 || files.gated} onClick={() => download(false)}><Download size={15} /> Baixar leve · {bytes(files.light_bytes)} ({files.light_count} arquivos)</button>
            <button type="button" className="btn btn-secondary" disabled={downloadBusy || files.full_count === 0 || files.gated} onClick={() => download(true)}><Download size={15} /> Baixar completo com pesos · {bytes(files.full_bytes)} ({files.full_count} arquivos)</button>
          </div>
          <details style={{ marginTop: 16 }}><summary style={{ cursor: "pointer" }}>Ver {files.files.length} arquivos</summary><ul style={{ maxHeight: 230, overflowY: "auto", paddingLeft: 20, fontSize: 12, lineHeight: 1.8 }}>{files.files.map(([name, size]) => <li key={name} style={{ overflowWrap: "anywhere" }}>{name} — {bytes(size)}</li>)}</ul></details>
        </>}
        {downloadBusy && <p role="status">Iniciando download...</p>}
        {notice && <p role="status" style={{ marginTop: 12 }}>{notice}</p>}
        {downloadError && <p role="alert" style={{ marginTop: 12 }}>{downloadError}</p>}
      </section>
    </div>
  </div>;
}
