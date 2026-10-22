import React, { useState } from "react";
import { Check, Loader2, X } from "lucide-react";
import { api } from "../api";

const STATUS = {
  pending: ["Aguardando você", "card-v2-chip-accent"],
  running: ["Baixando", ""],
  done: ["Concluído", "card-v2-chip-ok"],
  rejected: ["Recusado", ""],
  error: ["Erro", "is-error"],
};

// Pedidos de download feitos pelos agentes (Claude, Codex, Gemini) pela MCP git-search.
// Nada baixa sem o clique em Aprovar.
export default function AgentRequestsView({ requests, onChanged, onNotice }) {
  const [busyId, setBusyId] = useState(null);

  async function resolve(req, approve) {
    setBusyId(req.id);
    try {
      const result = await api.resolveAgentRequest(req.id, approve);
      onNotice?.(
        result.status === "error" ? "error" : "success",
        approve ? "Pedido aprovado" : "Pedido recusado",
        result.message || req.target,
      );
    } catch (err) {
      onNotice?.("error", "Falha ao resolver pedido", String(err));
    } finally {
      setBusyId(null);
      onChanged?.();
    }
  }

  if (!requests.length) {
    return (
      <div className="glass-panel catalog-v2-empty">
        Nenhum pedido dos agentes ainda. Pela MCP <code>git-search</code>, os
        agentes podem pedir downloads de repositórios e modelos; eles aparecem
        aqui para você aprovar.
      </div>
    );
  }

  return (
    <section className="agent-requests">
      <p className="catalog-v2-status">
        <span className="catalog-v2-notice">
          Skills pedidas pelos agentes instalam direto no ecossistema.
          Repositórios e modelos só baixam depois do seu clique em Aprovar.
        </span>
      </p>
      <div className="agent-requests-list">
        {requests.map((req) => {
          const [label, tone] = STATUS[req.status] || [req.status, ""];
          const pending = req.status === "pending";
          return (
            <article
              key={req.id}
              className="repo-card-luxury card-v2 agent-request"
            >
              <div className="card-v2-top">
                <div className="card-v2-tags">
                  <span className="rank-tag-luxury">#{req.id}</span>
                  <span className="card-v2-chip">
                    {req.kind === "dataset" ? "Dataset" : req.kind === "model" ? "Modelo" : "Repositório"}
                  </span>
                  {(req.kind === "model" || req.kind === "dataset") && (
                    <span className="card-v2-chip">
                      {req.kind === "dataset"
                        ? (req.full ? "Completo com dados" : "Metadados")
                        : (req.full ? "Completo com pesos" : "Sem pesos")}
                    </span>
                  )}
                  <span className={`card-v2-chip ${tone}`}>{label}</span>
                </div>
                <span className="summary-muted">{req.created_at}</span>
              </div>
              <h3 className="card-v2-title">{req.target}</h3>
              <p className="summary-muted" style={{ margin: "6px 0 10px" }}>
                Pedido por <strong>{req.agent || "agente"}</strong>
              </p>
              {req.reason && (
                <div className="summary-block" style={{ marginBottom: 12 }}>
                  <span className="summary-label">Motivo do agente</span>
                  <p className="summary-text">{req.reason}</p>
                </div>
              )}
              {req.message && <p className="card-v2-note">{req.message}</p>}
              {(pending || req.status === "error") && (
                <div className="card-footer-luxury card-v2-footer">
                  <button
                    type="button"
                    className="btn-download-cyber"
                    disabled={busyId === req.id}
                    onClick={() => resolve(req, true)}
                  >
                    {busyId === req.id ? (
                      <Loader2 size={14} className="animate-spin" />
                    ) : (
                      <Check size={14} />
                    )}
                    <span>
                      {req.status === "error" ? "Tentar novamente" : "Aprovar e baixar"}
                    </span>
                  </button>
                  <button
                    type="button"
                    className="btn-details-glass"
                    disabled={busyId === req.id}
                    onClick={() => resolve(req, false)}
                  >
                    <X size={14} />
                    {req.status === "error" ? "Remover" : "Recusar"}
                  </button>
                </div>
              )}
            </article>
          );
        })}
      </div>
    </section>
  );
}
