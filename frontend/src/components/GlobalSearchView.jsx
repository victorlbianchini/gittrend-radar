import React, { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Loader2, Search } from "lucide-react";
import { api } from "../api";
import RepoCard from "./RepoCard";

export default function GlobalSearchView({ cardProps = {}, onError }) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("stars");
  const [applied, setApplied] = useState({ query: "", sort: "stars" });
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const requestId = useRef(0);
  const errorHandler = useRef(onError);
  useEffect(() => { errorHandler.current = onError; }, [onError]);

  const search = useCallback(async (searchQuery, searchSort, page = 1) => {
    const request = ++requestId.current;
    setLoading(true);
    setError("");
    setResult(null);
    try {
      const response = await api.githubSearch(searchQuery.trim(), searchSort, page);
      if (request !== requestId.current) return;
      setResult(response);
      setApplied({ query: searchQuery.trim(), sort: searchSort });
    } catch (err) {
      if (request !== requestId.current) return;
      const message = String(err);
      setError(message);
      errorHandler.current?.(message);
    } finally {
      if (request === requestId.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    const counter = requestId;
    let active = true;
    Promise.resolve().then(() => { if (active) search("", "stars"); });
    return () => { active = false; counter.current++; };
  }, [search]);

  const pages = result ? Math.max(1, Math.min(10, Math.ceil(result.total / 100))) : 1;

  return (
    <section style={{ marginTop: 10 }} aria-label="Busca global do GitHub">
      <form className="explorer-filter-bar" onSubmit={(event) => { event.preventDefault(); search(query, sort); }} style={{ display: "flex", gap: 14, flexWrap: "wrap", alignItems: "end" }}>
        <div style={{ flex: "1 1 300px" }}>
          <label htmlFor="global-github-query">Buscar no GitHub</label>
          <input id="global-github-query" className="input-field" style={{ marginTop: 8 }} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Nome, assunto ou filtros, como language:rust" aria-describedby="global-github-hint" />
        </div>
        <div style={{ minWidth: 190 }}>
          <label htmlFor="global-github-sort">Ordenação</label>
          <select id="global-github-sort" className="input-field" style={{ marginTop: 8 }} value={sort} onChange={(event) => setSort(event.target.value)}>
            <option value="stars">Mais estrelas</option>
            <option value="updated">Atualizados recentemente</option>
            <option value="best">Melhor correspondência</option>
          </select>
        </div>
        <button type="submit" className="btn btn-secondary" disabled={loading}>{loading ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}Buscar</button>
      </form>
      <p className="models-source-line" id="global-github-hint">Busca em todo o GitHub, com até 100 repositórios por página. Deixe a consulta vazia para explorar os repositórios por estrelas.</p>
      {error && <p role="alert" className="glass-panel" style={{ padding: 20, color: "#fca5a5", overflowWrap: "anywhere" }}>{error}</p>}
      <div role="status" aria-live="polite" style={{ margin: "16px 0", color: "var(--text-muted)" }}>
        {loading ? "Buscando repositórios no GitHub…" : result ? `${result.total.toLocaleString("pt-BR")} resultados. Página ${result.page} de ${pages}.` : ""}
      </div>
      {result?.total > 1000 && <p className="glass-panel" style={{ padding: 16, marginBottom: 20, color: "#E5B623" }}>O GitHub disponibiliza apenas os primeiros 1.000 resultados de cada busca. Refine a consulta para encontrar outros repositórios.</p>}
      {result && (result.repositories.length ? <>
        <div className="repo-grid-luxury">
          {result.repositories.map((repo, index) => <RepoCard {...cardProps} onToggleFavorite={async (target) => {
            const isFavorite = await cardProps.onToggleFavorite?.(target);
            if (typeof isFavorite === 'boolean') setResult((current) => ({ ...current, repositories: current.repositories.map((item) => item.repo_id === target.repo_id ? { ...item, is_favorite: isFavorite } : item) }));
          }} key={repo.repo_id} repo={repo} rank={(result.page - 1) * 100 + index + 1} />)}
        </div>
        <nav aria-label="Páginas dos resultados do GitHub" style={{ display: "flex", justifyContent: "center", gap: 16, alignItems: "center", margin: "28px 0" }}>
          <button className="btn btn-secondary" type="button" disabled={loading || result.page <= 1} onClick={() => search(applied.query, applied.sort, result.page - 1)}><ChevronLeft size={16} />Anterior</button>
          <span aria-current="page">Página {result.page} de {pages}</span>
          <button className="btn btn-secondary" type="button" disabled={loading || !result.has_more || result.page >= pages || result.page >= 10} onClick={() => search(applied.query, applied.sort, result.page + 1)}>Próxima<ChevronRight size={16} /></button>
        </nav>
      </> : <p className="glass-panel" style={{ padding: 30 }}>Nenhum repositório encontrado para esta consulta.</p>)}
    </section>
  );
}
