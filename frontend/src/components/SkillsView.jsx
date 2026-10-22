import React, { useEffect, useRef, useState } from "react";
import { BookOpen, Download, ExternalLink, FolderOpen, Loader2, Search, X } from "lucide-react";
import { api } from "../api";

const compact = new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 });
const readable = (value) => typeof value === "string" ? value : JSON.stringify(value, null, 2);

export default function SkillsView() {
  const [query, setQuery] = useState("");
  const [skills, setSkills] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [detail, setDetail] = useState(null);
  const [detailBusy, setDetailBusy] = useState(false);
  const [downloadBusy, setDownloadBusy] = useState(null);
  const requestId = useRef(0);
  const detailRequest = useRef(0);

  async function search(value) {
    const request = ++requestId.current;
    setLoading(true);
    setError("");
    try {
      const result = await api.skillsSearch(value.trim());
      if (request === requestId.current) setSkills(result);
    } catch (err) { if (request === requestId.current) setError(String(err)); }
    finally { if (request === requestId.current) setLoading(false); }
  }

  useEffect(() => {
    const searchCounter = requestId;
    const detailCounter = detailRequest;
    let active = true;
    Promise.resolve().then(() => { if (active) search(""); });
    return () => { active = false; searchCounter.current++; detailCounter.current++; };
  }, []);

  async function explain(skill) {
    const request = ++detailRequest.current;
    setDetail({ skill, explanation: null });
    setDetailBusy(true);
    setError("");
    try {
      const result = await api.explainSkill(skill);
      if (request === detailRequest.current) setDetail(result);
    } catch (err) { if (request === detailRequest.current) setError(String(err)); }
    finally { if (request === detailRequest.current) setDetailBusy(false); }
  }

  async function download(skill) {
    setDownloadBusy(skill.id);
    setError("");
    try {
      const result = await api.downloadSkill(skill);
      if (!result.success) throw new Error(result.message || "Não foi possível baixar a skill.");
      const path = result.folder_path;
      const update = (item) => item.id === skill.id ? { ...item, is_downloaded: true, downloaded_path: path } : item;
      setSkills((current) => current.map(update));
      setDetail((current) => current ? { ...current, skill: update(current.skill) } : current);
    } catch (err) { setError(String(err)); }
    finally { setDownloadBusy(null); }
  }

  async function openFolder(path) {
    try {
      const result = await api.openFolder(path);
      if (!result.success) throw new Error(result.message || "Não foi possível abrir a pasta.");
    }
    catch (err) { setError(String(err)); }
  }

  const explanation = detail?.explanation;
  return (
    <div style={{ marginTop: 10 }}>
      <form className="explorer-filter-bar" onSubmit={(event) => { event.preventDefault(); search(query); }} style={{ display: "flex", gap: 12, alignItems: "end" }}>
        <div style={{ flex: 1 }}>
          <label htmlFor="skills-search">Buscar skills</label>
          <input id="skills-search" className="input-field" style={{ marginTop: 8 }} placeholder="Nome, assunto ou repositório…" value={query} onChange={(event) => setQuery(event.target.value)} />
        </div>
        <button type="submit" className="btn btn-secondary" disabled={loading}>{loading ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}Buscar</button>
      </form>
      <p className="models-source-line">Fontes: skills.sh e catálogos públicos da Anthropic e OpenAI no GitHub. Consulte os arquivos antes de baixar para a pasta configurada. A busca cobre essas fontes; não representa toda a internet.</p>
      {error && <p className="glass-panel" role="alert" style={{ padding: 20, color: "#fca5a5", overflowWrap: "anywhere" }}>{error}</p>}
      {detail && (
        <section className="glass-panel" aria-labelledby="skill-detail-title" style={{ padding: 24, marginBottom: 24, overflowWrap: "anywhere" }}>
          <div className="modal-header">
            <h2 className="modal-title" id="skill-detail-title">{detail.skill.name}</h2>
            <button className="modal-close-btn" type="button" aria-label="Fechar detalhes da skill" onClick={() => { detailRequest.current++; setDetail(null); setDetailBusy(false); }}><X size={18} /></button>
          </div>
          {detailBusy && <p role="status">Lendo arquivos da skill…</p>}
          {explanation && <>
            <p style={{ whiteSpace: "pre-wrap", lineHeight: 1.7 }}>{explanation.summary}</p>
            {[["Quando usar", explanation.when_to_use], ["Seções", explanation.sections], ["Arquivos", explanation.files]].map(([title, content]) => content?.length > 0 && (
              <div key={title} style={{ marginTop: 20 }}>
                <h3>{title}</h3>
                {Array.isArray(content) ? <ul style={{ paddingLeft: 20, marginTop: 8 }}>{content.map((item, index) => <li key={index} style={{ whiteSpace: "pre-wrap", marginBottom: 6 }}>{readable(item)}</li>)}</ul> : <p style={{ whiteSpace: "pre-wrap" }}>{readable(content)}</p>}
              </div>
            ))}
            {explanation.preview && <details style={{ marginTop: 20 }}><summary>Prévia do conteúdo</summary><pre className="code-block" style={{ whiteSpace: "pre-wrap", marginTop: 12 }}>{readable(explanation.preview)}</pre></details>}
            {explanation.install_hint && <p style={{ marginTop: 20, whiteSpace: "pre-wrap", color: "var(--text-muted)" }}>{explanation.install_hint}</p>}
          </>}
        </section>
      )}
      {loading ? <p role="status" style={{ padding: 30 }}>Buscando skills…</p> : skills.length === 0 ? <p className="glass-panel" style={{ padding: 30 }}>Nenhuma skill encontrada.</p> : (
        <div className="repo-grid-luxury">
          {skills.map((skill) => (
            <article className="repo-card-luxury" key={skill.id}>
              <div className="card-identity-luxury">
                <div className="owner-chip"><span className="owner-handle">{skill.repo}</span></div>
                <h3 className="repo-title-luxury-text">{skill.name}</h3>
              </div>
              <p style={{ color: "var(--text-muted)", lineHeight: 1.6, margin: "16px 0", overflowWrap: "anywhere" }}>{skill.description || "Descrição não informada pela fonte."}</p>
              <div className="card-metrics-strip">
                {skill.stars != null && <span className="metric-pill">{compact.format(skill.stars)} estrelas no repositório</span>}
                {skill.installs != null && <span className="metric-pill"><Download size={12} />{compact.format(skill.installs)} instalações</span>}
              </div>
              {skill.sources?.length > 0 && <p style={{ color: "var(--text-dim)", fontSize: 12, marginBottom: 16 }}>Fontes: {skill.sources.join(", ")}</p>}
              <div className="card-footer-luxury" style={{ flexWrap: "wrap", gap: 8 }}>
                <button type="button" className="btn-details-glass" onClick={() => explain(skill)}><BookOpen size={14} />Detalhes</button>
                <button type="button" className="btn-details-glass" disabled={downloadBusy !== null || skill.is_downloaded} onClick={() => download(skill)}>{downloadBusy === skill.id ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}{skill.is_downloaded ? "Baixada" : "Baixar"}</button>
                {skill.is_downloaded && skill.downloaded_path && <button type="button" className="btn-details-glass" onClick={() => openFolder(skill.downloaded_path)}><FolderOpen size={14} />Abrir pasta</button>}
                {skill.github_url && <a className="btn-details-glass" href={skill.github_url}><ExternalLink size={14} />GitHub</a>}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
