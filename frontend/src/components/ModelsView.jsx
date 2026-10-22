import React, { useMemo, useRef, useState } from "react";
import {
  Search,
  Heart,
  Download,
  TrendingUp,
  ExternalLink,
  RefreshCw,
  Loader2,
} from "lucide-react";

import { api } from "../api";
import ModelDetailModal from "./ModelDetailModal";

const GROUPS = [
  { id: "all", label: "Todos" },
  { id: "texto", label: "Texto e LLMs", color: "#c084fc" },
  { id: "visao", label: "Visão e multimodal", color: "#38bdf8" },
  { id: "imagem", label: "Imagem e 3D", color: "#f472b6" },
  { id: "video", label: "Vídeo", color: "#fb923c" },
  { id: "audio", label: "Voz e áudio", color: "#34d399" },
  { id: "embeddings", label: "Embeddings e busca", color: "#818cf8" },
  { id: "outros", label: "Outros", color: "#94a3b8" },
];
const groupOf = (id) =>
  GROUPS.find((g) => g.id === id) || GROUPS[GROUPS.length - 1];

const compact = new Intl.NumberFormat("pt-BR", {
  notation: "compact",
  maximumFractionDigits: 1,
});

function ModelCard({ model, rank, onDetails }) {
  const g = groupOf(model.group);
  return (
    <div className={`repo-card-luxury ${rank === 1 ? "rank-gold-glow" : ""}`}>
      <div className="card-top-bar-luxury">
        <div className="top-pills-row">
          <span
            className={`rank-tag-luxury ${rank === 1 ? "rank-1" : rank === 2 ? "rank-2" : rank === 3 ? "rank-3" : ""}`}
          >
            #{rank} {rank === 1 ? "TOP" : ""}
          </span>
          <span
            className="lang-capsule-luxury"
            style={{
              color: g.color,
              borderColor: `${g.color}55`,
              backgroundColor: `${g.color}1f`,
            }}
          >
            <span
              className="lang-mini-dot"
              style={{ backgroundColor: g.color }}
            />
            {model.task_label}
          </span>
        </div>
        {model.is_new && <span className="model-new-badge">Novo</span>}
      </div>

      <div className="card-identity-luxury">
        {model.author && (
          <div className="owner-chip">
            <span className="owner-handle">@{model.author}</span>
          </div>
        )}
        <div className="title-row-luxury">
          <a
            href={model.url}
            className="repo-title-luxury-link"
            title={`Abrir ${model.id} no Hugging Face`}
          >
            <h3 className="repo-title-luxury-text">{model.name}</h3>
          </a>
        </div>
      </div>

      <div className="card-metrics-strip">
        <div
          className="metric-pill highlight-trend-luxury"
          title="Pontuação de tendência do Hugging Face"
        >
          <TrendingUp size={12} className="metric-icon" />
          <span>{compact.format(model.trending_score)} em alta</span>
        </div>
        <div
          className="metric-pill"
          title={`${model.likes.toLocaleString("pt-BR")} curtidas`}
        >
          <Heart
            size={12}
            style={{ color: "#f472b6" }}
            className="metric-icon"
          />
          <span>{compact.format(model.likes)}</span>
        </div>
        <div
          className="metric-pill"
          title={`${model.downloads.toLocaleString("pt-BR")} downloads no último mês`}
        >
          <Download
            size={12}
            style={{ color: "#34d399" }}
            className="metric-icon"
          />
          <span>{compact.format(model.downloads)}</span>
        </div>
      </div>

      <div className="model-created">
        Publicado em{" "}
        {model.created_at
          ? new Date(`${model.created_at}T12:00:00`).toLocaleDateString("pt-BR")
          : "data não informada"}
      </div>

      <div className="card-footer-luxury">
        <button type="button" className="btn-details-glass" onClick={() => onDetails(model)} style={{ flex: 1, justifyContent: "center" }}>
          <ExternalLink size={13} /><span>Detalhes e download</span>
        </button>
      </div>
    </div>
  );
}

export default function ModelsView({ models, loading, error, onRefresh, onNotice, onOpenFolder }) {
  const [group, setGroup] = useState("all");
  const [query, setQuery] = useState("");
  const [onlyNew, setOnlyNew] = useState(false);
  const [searchPage, setSearchPage] = useState(null);
  const [searchedQuery, setSearchedQuery] = useState("");
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState("");
  const [selected, setSelected] = useState(null);
  const request = useRef(0);
  const source = searchPage?.models || models;
  const currentError = searchPage ? searchError : (searchError || error);
  const busy = searchPage ? searchLoading : (searchLoading || loading);

  async function searchMore(cursor = null) {
    const token = ++request.current;
    const term = cursor ? searchedQuery : query.trim();
    setSearchLoading(true);
    setSearchError("");
    if (!cursor) {
      setSearchPage({ models: [], next_cursor: null });
      setSearchedQuery(term);
      setGroup("all");
    }
    try {
      const page = await api.hfSearch(term, cursor);
      if (token !== request.current) return;
      setSearchPage((previous) => ({ ...page, models: cursor
        ? [...new Map([...previous.models, ...page.models].map((m) => [m.id, m])).values()]
        : page.models }));
    } catch (err) {
      if (token === request.current) setSearchError(String(err));
    } finally {
      if (token === request.current) setSearchLoading(false);
    }
  }

  function showTrending() {
    request.current += 1;
    setSearchPage(null);
    setSearchError("");
    setSearchLoading(false);
    setQuery("");
    setGroup("all");
  }

  const counts = useMemo(() => {
    const c = {};
    for (const m of source) c[m.group] = (c[m.group] || 0) + 1;
    return c;
  }, [source]);

  const visible = useMemo(() => {
    const q = searchPage ? "" : query.trim().toLowerCase();
    return source
      .filter((m) => group === "all" || m.group === group)
      .filter((m) => !onlyNew || m.is_new)
      .filter(
        (m) =>
          !q ||
          m.id.toLowerCase().includes(q) ||
          m.task_label.toLowerCase().includes(q),
      );
  }, [source, group, query, onlyNew, searchPage]);

  return (
    <div style={{ marginTop: "10px" }}>
      <div className="explorer-filter-bar models-filter-bar">
        <div
          style={{
            display: "flex",
            gap: "14px",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <form onSubmit={(e) => { e.preventDefault(); searchMore(); }} style={{ display: "flex", gap: 8, flex: "1 1 300px" }}>
          <div style={{ position: "relative", flex: 1 }}>
            <Search
              size={17}
              style={{
                position: "absolute",
                left: 15,
                top: "50%",
                transform: "translateY(-50%)",
                color: "var(--text-dim)",
              }}
            />
            <input
              type="text"
              className="input-field"
              style={{ paddingLeft: "44px" }}
              aria-label="Buscar modelos no Hugging Face"
              placeholder="Buscar em todo o Hugging Face..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <button type="submit" className="btn btn-secondary" disabled={searchLoading}>Buscar</button>
          </form>
          {searchPage && <button type="button" className="btn btn-secondary" onClick={showTrending}>Voltar aos modelos em alta</button>}
          <button
            type="button"
            className={`tab-btn ${onlyNew ? "active" : ""}`}
            onClick={() => setOnlyNew((v) => !v)}
          >
            Só lançados nas últimas 2 semanas
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={searchPage ? () => searchMore() : onRefresh}
            disabled={busy}
          >
            {busy ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
            <span>Atualizar</span>
          </button>
        </div>
        <div
          className="tabs-container"
          style={{ marginTop: "16px", flexWrap: "wrap" }}
        >
          {GROUPS.filter((g) => g.id === "all" || counts[g.id]).map((g) => (
            <button
              key={g.id}
              type="button"
              className={`tab-btn ${group === g.id ? "active" : ""}`}
              onClick={() => setGroup(g.id)}
            >
              {g.color && (
                <span
                  className="lang-mini-dot"
                  style={{ backgroundColor: g.color }}
                />
              )}
              {g.label}
              <span className="chip-count">
                {g.id === "all" ? source.length : counts[g.id]}
              </span>
            </button>
          ))}
        </div>
      </div>

      <div className="models-source-line">
        {searchPage ? `Busca global no Hugging Face${searchedQuery ? ` por “${searchedQuery}”` : ""}. Filtros aplicados aos resultados carregados.` : "Fonte: modelos em alta no Hugging Face, atualizados a cada hora. Use Buscar para pesquisar todo o catálogo."}
      </div>

      {currentError && source.length === 0 ? (
        <div
          className="glass-panel"
          style={{ padding: "60px 20px", textAlign: "center" }}
        >
          <h4
            style={{ fontSize: "17px", fontWeight: 700, marginBottom: "8px" }}
          >
            Não foi possível buscar os modelos
          </h4>
          <p style={{ fontSize: "13px", color: "var(--text-muted)" }}>
            {currentError}
          </p>
        </div>
      ) : busy && source.length === 0 ? (
        <div
          className="glass-panel"
          style={{ padding: "80px 20px", textAlign: "center" }}
        >
          <Loader2
            size={32}
            className="animate-spin"
            style={{ color: "var(--accent-indigo)", margin: "0 auto 14px" }}
          />
          <h4 style={{ fontSize: "17px", fontWeight: 700 }}>
            {searchPage ? "Buscando modelos no Hugging Face..." : "Buscando modelos em alta..."}
          </h4>
        </div>
      ) : visible.length === 0 ? (
        <div
          className="glass-panel"
          style={{ padding: "60px 20px", textAlign: "center" }}
        >
          <h4 style={{ fontSize: "17px", fontWeight: 700 }}>
            Nenhum modelo com esses filtros
          </h4>
        </div>
      ) : (
        <div className="repo-grid-luxury">
          {visible.map((m, i) => (
            <ModelCard key={m.id} model={m} rank={i + 1} onDetails={setSelected} />
          ))}
        </div>
      )}
      {currentError && source.length > 0 && <p role="alert">{currentError}</p>}
      {searchPage?.next_cursor && <button type="button" className="btn btn-secondary" disabled={searchLoading} onClick={() => searchMore(searchPage.next_cursor)} style={{ marginTop: 20 }}>
        {searchLoading ? "Carregando..." : "Carregar mais modelos"}
      </button>}
      {selected && <ModelDetailModal key={selected.id} model={selected} onClose={() => setSelected(null)} onNotice={onNotice} onOpenFolder={onOpenFolder} />}
    </div>
  );
}
