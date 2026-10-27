import React, { useEffect, useId, useState } from "react";
import {
  Bookmark,
  BookOpen,
  Check,
  Download,
  ExternalLink,
  FolderOpen,
  Info,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { api } from "../api";
import { cardDetails } from "./cardDetails";
import OwnerAvatar from "./OwnerAvatar";

const CATEGORY_LABELS = {
  ia_agentes: "IA e Agentes",
  frontend_ui: "Frontend e UI",
  backend_api: "Backend e APIs",
  devops_cli: "DevOps e CLI",
  security: "Segurança",
  data_eng: "Dados",
  mobile: "Mobile",
  geral: "Geral",
};
const categoryLabel = (value) => {
  const text = String(value);
  if (CATEGORY_LABELS[text]) return CATEGORY_LABELS[text];
  const spaced = text.replaceAll("_", " ");
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
};

const compact = new Intl.NumberFormat("pt-BR", {
  notation: "compact",
  maximumFractionDigits: 1,
});

const formatBytes = (n) => {
  if (!n) return "0 B";
  const unit = Math.min(3, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** unit).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} ${["B", "KB", "MB", "GB"][unit]}`;
};

export default function ContentCard({
  item,
  record,
  rank,
  onRetry,
  onDownload,
  onOpenFolder,
  onToggleFavorite,
  downloadJob,
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [full, setFull] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [modelMeta, setModelMeta] = useState(null);
  const [installingSkill, setInstallingSkill] = useState(false);
  const detailsId = useId();
  const repo = item.repo;
  const model = item.model;
  const skill = item.skill;
  const isSkillRepo = Boolean(
    repo &&
      !skill &&
      (repo.repo_id?.toLowerCase().includes("skill") ||
        repo.description?.toLowerCase().includes("skill.md") ||
        repo.description?.toLowerCase().includes("agent skill") ||
        repo.description?.toLowerCase().includes("claude skill") ||
        repo.description?.toLowerCase().includes("skills system")),
  );

  useEffect(() => {
    if (!expanded || !model || modelMeta) return;
    let active = true;
    api
      .modelFiles(model.id)
      .then((info) => {
        if (active) setModelMeta(info);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [expanded, model, modelMeta]);

  async function installRepoAsSkill() {
    if (!repo) return;
    setInstallingSkill(true);
    setError("");
    setNotice("");
    try {
      const result = await api.installEcosystemSkill({
        id: `${repo.repo_id}::${repo.name}`,
        name: repo.name,
        repo: repo.repo_id,
        branch: "HEAD",
        path: "",
        description: repo.description || "",
        stars: repo.stars || 0,
        installs: 0,
        sources: ["GitHub"],
        github_url: repo.url || "",
        is_downloaded: false,
        downloaded_path: "",
      });
      if (result?.folder_path) setLocalPath(result.folder_path);
      setNotice(result?.message || "Skill instalada no ecossistema.");
    } catch (err) {
      setError(String(err));
    } finally {
      setInstallingSkill(false);
    }
  }
  const jobActive = downloadJob && !downloadJob.finished;
  const jobComplete = downloadJob?.finished && downloadJob.success;
  const jobFailure = downloadJob?.finished && !downloadJob.success;
  const downloading = busy || jobActive;
  const path =
    (jobComplete && downloadJob.folder_path) ||
    localPath ||
    repo?.downloaded_path ||
    skill?.downloaded_path ||
    model?.downloaded_path;
  const downloaded = Boolean(
    path || repo?.is_downloaded || skill?.is_downloaded || model?.is_downloaded,
  );
  const owner =
    repo?.owner || model?.author || skill?.repo?.split("/")[0] || "";
  const sourceLabel =
    { hf: "Hugging Face", github: "GitHub" }[item.source] ||
    item.source ||
    (model ? "Hugging Face" : "GitHub");
  const kindLabel =
    item.kind === "dataset"
      ? "Dataset"
      : item.kind === "mcp" || item.category === "Servidor MCP"
        ? "Servidor MCP"
        : model
          ? "Modelo"
          : skill
            ? "Skill"
            : "Repositório";
  const category = model?.task_label || item.category;

  // Dois resumos, sempre nesta ordem: a descricao traduzida e, embaixo, o resumo curto da IA.
  const ready = record?.status === "ready";
  const translated = item.description_pt || (ready && record.description_pt) || "";
  const original = item.description || repo?.description || "";
  const topText = translated || original;
  const aiText = ready && record.summary !== topText ? record.summary : "";
  const aiPending =
    !record || record.status === "pending" || record.status === "running";

  const metrics = [
    [repo?.stars_today, "hoje", "hot", "+"],
    [repo?.stars_today ? 0 : repo?.stars_per_day, "est./dia", "hot", "+"],
    [repo?.stars ?? skill?.stars, "estrelas"],
    [repo?.forks, "forks"],
    [model?.likes, "curtidas"],
    [model?.downloads, "downloads"],
    [skill?.installs, "instalações"],
  ].filter(
    ([value]) =>
      typeof value === "number" && Number.isFinite(value) && value > 0,
  );
  const { facts, texts, lists, installCmd } = cardDetails(item, record, [
    topText,
    aiText,
  ]);

  async function download() {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const result = await onDownload?.(full);
      if (result?.success === false)
        throw new Error(result.message || "Não foi possível baixar.");
      if (result?.success && result.folder_path)
        setLocalPath(result.folder_path);
      if (typeof result === "string") setNotice(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  function toggleFromBody(event) {
    if (
      event.target.closest("button, a, select, input, label") ||
      window.getSelection?.().toString()
    )
      return;
    setExpanded((value) => !value);
  }

  return (
    <article
      className={`repo-card-luxury card-v2 ${rank === 1 ? "rank-gold-glow" : ""} ${downloaded ? "is-downloaded" : ""} ${expanded ? "is-expanded" : ""}`}
    >
      <div className="card-v2-top">
        <div className="card-v2-tags">
          {rank && (
            <span
              className={`rank-tag-luxury ${rank <= 3 ? `rank-${rank}` : ""}`}
            >
              #{rank}
            </span>
          )}
          <span className="card-v2-chip">{kindLabel}</span>
          {category && (
            <span className="card-v2-chip card-v2-chip-accent">
              {categoryLabel(category)}
            </span>
          )}
          {downloaded && (
            <span className="card-v2-chip card-v2-chip-downloaded">
              <Check size={11} aria-hidden="true" />
              Na máquina
            </span>
          )}
        </div>
        <div className="quick-action-dock">
          {onToggleFavorite && (
            <button
              type="button"
              className={`dock-icon-btn fav-toggle ${repo?.is_favorite ? "is-fav" : ""}`}
              aria-label={
                repo?.is_favorite
                  ? "Remover dos favoritos"
                  : "Adicionar aos favoritos"
              }
              title={
                repo?.is_favorite
                  ? "Remover dos favoritos"
                  : "Adicionar aos favoritos"
              }
              aria-pressed={Boolean(repo?.is_favorite)}
              onClick={() =>
                Promise.resolve()
                  .then(onToggleFavorite)
                  .catch((err) => setError(String(err)))
              }
            >
              <Bookmark
                size={14}
                fill={repo?.is_favorite ? "currentColor" : "none"}
              />
            </button>
          )}
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="dock-icon-btn"
            aria-label={`Abrir ${item.title} no ${sourceLabel}`}
            title={`Abrir no ${sourceLabel}`}
          >
            <ExternalLink size={14} />
          </a>
        </div>
      </div>

      {/* Atalho de mouse para o botão Detalhes abaixo, que continua sendo o controle acessível. */}
      <div className="card-v2-body" onClick={toggleFromBody}>
        <div className="card-v2-identity">
          <OwnerAvatar owner={owner} source={model ? "hf" : "github"} />
          {/* So o nome do projeto: o dono aparece apenas na foto ao lado. */}
          <h3 className="card-v2-title" title={item.title}>
            {repo?.name || model?.name || skill?.name || item.title}
          </h3>
        </div>

        {metrics.length > 0 && (
          <div className="card-v2-metrics">
            {metrics.map(([value, label, tone, prefix]) => (
              <span
                key={label}
                className={`card-v2-metric ${tone === "hot" ? "is-hot" : ""}`}
                title={`${value.toLocaleString("pt-BR")} ${label}`}
              >
                <strong>
                  {prefix || ""}
                  {compact.format(value)}
                </strong>{" "}
                {label}
              </span>
            ))}
          </div>
        )}

        <section aria-label="Resumo" className="card-v2-summary">
          {topText && (
            <div className="summary-block">
              <span className="summary-label">
                {translated ? "Resumo traduzido" : "Descrição original"}
              </span>
              <p className="summary-text">{topText}</p>
            </div>
          )}
          <div className="summary-block summary-ai">
            <span className="summary-label">
              Resumo da IA
              {ready && record.model ? (
                <span className="summary-model"> · {record.model}</span>
              ) : null}
            </span>
            {aiText && <p className="summary-text">{aiText}</p>}
            {ready && !aiText && (
              <p className="summary-muted">Igual à descrição acima.</p>
            )}
            {aiPending && (
              <p role="status" className="summary-muted">
                Resumindo…
              </p>
            )}
            {record?.status === "error" && (
              <p role="status" className="summary-muted">
                Resumo indisponível.
              </p>
            )}
            {record?.status === "error" && onRetry && (
              <button
                type="button"
                className="btn-details-glass"
                style={{ marginTop: 8 }}
                onClick={() =>
                  Promise.resolve()
                    .then(onRetry)
                    .catch((err) => setError(String(err)))
                }
              >
                <RefreshCw size={13} />
                Tentar novamente
              </button>
            )}
          </div>
        </section>

        {expanded && (
          <section
            id={detailsId}
            aria-label="Detalhes"
            className="card-v2-details"
          >
            {texts.map(([label, value]) => (
              <div key={label}>
                <h4 className="summary-label">{label}</h4>
                <p className="summary-text">{value}</p>
              </div>
            ))}
            {lists.map(([label, values]) => (
              <div key={label}>
                <h4 className="summary-label">{label}</h4>
                <ul className="card-v2-list">
                  {values.map((value, index) => (
                    <li key={index}>{value}</li>
                  ))}
                </ul>
              </div>
            ))}
            {installCmd && (
              <div>
                <h4 className="summary-label">Instalação</h4>
                <code className="card-v2-code">{installCmd}</code>
              </div>
            )}
            {facts.length > 0 && (
              <dl className="card-v2-facts">
                {modelMeta?.vram_hint && (
                  <>
                    <dt>Hardware / VRAM</dt>
                    <dd>{modelMeta.vram_hint}</dd>
                  </>
                )}
                {modelMeta?.full_bytes > 0 && (
                  <>
                    <dt>Tamanho total</dt>
                    <dd>{formatBytes(modelMeta.full_bytes)} ({modelMeta.full_count} arquivos)</dd>
                  </>
                )}
                {modelMeta?.quantizations?.length > 0 && (
                  <>
                    <dt>Quantizações GGUF</dt>
                    <dd>{modelMeta.quantizations.slice(0, 4).map(([name, sz]) => `${name} (${formatBytes(sz)})`).join(" · ")}</dd>
                  </>
                )}
                {facts.map(([label, value]) => (
                  <React.Fragment key={label}>
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  </React.Fragment>
                ))}
              </dl>
            )}
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              className="btn-details-glass"
              style={{ justifySelf: "start" }}
            >
              <ExternalLink size={13} />
              Abrir no {sourceLabel}
            </a>
          </section>
        )}
      </div>

      {model && (
        <label className="card-v2-select">
          Download
          <select
            className="input-field"
            value={full ? "full" : "light"}
            disabled={downloading}
            onChange={(event) => setFull(event.target.value === "full")}
          >
            {item.kind === "dataset" ? (
              <>
                <option value="light">Metadados e documentação</option>
                <option value="full">Completo com dados (parquet/tabelas)</option>
              </>
            ) : (
              <>
                <option value="light">Arquivos sem pesos</option>
                <option value="full">
                  Completo com pesos (pode ocupar vários GB)
                </option>
              </>
            )}
          </select>
        </label>
      )}
      <div className="card-footer-luxury card-v2-footer">
        <button
          type="button"
          className={downloaded ? "btn-download-cyber is-downloaded" : "btn-download-cyber"}
          disabled={downloading || (!model && downloaded) || !onDownload}
          onClick={download}
        >
          {downloading ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Download size={14} />
          )}
          <span>
            {downloading
              ? "Baixando…"
              : !model && downloaded
                ? "Baixado"
                : downloaded && model
                  ? (full ? (item.kind === "dataset" ? "Baixar dados completos" : "Baixar pesos") : "Baixado")
                  : "Baixar"}
          </span>
        </button>
        <button
          type="button"
          className="btn-details-glass"
          aria-expanded={expanded}
          aria-controls={detailsId}
          onClick={() => setExpanded((value) => !value)}
        >
          <Info size={14} />
          {expanded ? "Fechar detalhes" : "Detalhes"}
        </button>
        {isSkillRepo && (
          <button
            type="button"
            className="btn-details-glass"
            disabled={installingSkill}
            onClick={installRepoAsSkill}
            title="Instalar SKILL.md deste repositório direto no ecossistema"
          >
            {installingSkill ? <Loader2 size={14} className="animate-spin" /> : <BookOpen size={14} />}
            Instalar Skill
          </button>
        )}
        {path && onOpenFolder && (
          <button
            type="button"
            className="btn-open-folder-luxury"
            onClick={() =>
              Promise.resolve()
                .then(() => onOpenFolder(path))
                .catch((err) => setError(String(err)))
            }
          >
            <FolderOpen size={14} />
            Abrir pasta
          </button>
        )}
      </div>
      {notice && (
        <p role="status" className="card-v2-note">
          {notice}
        </p>
      )}
      {jobActive && downloadJob && (
        <div className="card-download-progress-box">
          <div className="card-download-progress-header">
            <span className="card-download-progress-file">
              <Loader2 size={12} className="animate-spin" />
              {downloadJob.current_file || "Baixando arquivos…"}
            </span>
            <span className="card-download-progress-bytes">
              {formatBytes(downloadJob.done_bytes)}
              {downloadJob.total_bytes > 0
                ? ` de ${formatBytes(downloadJob.total_bytes)} (${Math.min(100, Math.round((downloadJob.done_bytes / downloadJob.total_bytes) * 100))}%)`
                : ""}
            </span>
          </div>
          {downloadJob.total_bytes > 0 ? (
            <div className="card-download-bar">
              <div
                className="card-download-fill"
                style={{
                  width: `${Math.min(100, Math.max(3, Math.round((downloadJob.done_bytes / downloadJob.total_bytes) * 100)))}%`,
                }}
              />
            </div>
          ) : (
            <div className="card-download-bar is-indeterminate">
              <div className="card-download-fill" />
            </div>
          )}
        </div>
      )}
      {jobComplete && !downloading && (
        <div className="card-download-status is-success">
          <Check size={14} />
          <span>Download concluído com sucesso.</span>
        </div>
      )}
      {jobFailure && (
        <div className="card-download-status is-error">
          <span>{downloadJob.message || "Não foi possível concluir o download."}</span>
        </div>
      )}
      {error && !jobActive && (
        <div className="card-download-status is-error">
          <span>{error}</span>
        </div>
      )}
    </article>
  );
}
