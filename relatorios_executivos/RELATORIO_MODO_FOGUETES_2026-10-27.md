# Relatorio Executivo: Modo Foguetes 7d no GitTrend Radar (GitSearch)

**Data:** 2026-10-27
**Projeto:** `E:/REPOSITORIOS/git_search` (`GitTrend Radar` v0.1.7)
**Stack:** Tauri 2 + Rust + SQLite + React 19 + MCP stdio

## 1. Resumo Executivo

Implementado o modo **Foguetes 7d** (`mode: "breakout"` no catalogo e `modo: "foguetes"` no servidor MCP `git_search_tops`), projetado para capturar projetos recem-nascidos com maior velocidade real de adocao antes de chegarem ao topo acumulado tradicional. A ordenacao foi ativada nos cinco dominios do aplicativo (`GitHub`, `Servidores MCP`, `Modelos de IA`, `Datasets de IA` e `Skills`), mantendo zero invencao de dados e substituindo o icone generico `Sparkles` por um emblema vetorial geometrico autoral (`RadarBadgeIcon`).

## 2. Antes vs Depois

| Dimensao | Antes | Depois |
|---|---|---|
| Modos de descoberta na UI | `Tops`, `Novidades`, `Explorar` | `Foguetes 7d`, `Tops`, `Novidades`, `Explorar` |
| Metrica de velocidade diaria | Sem calculo explicito de tracao por idade | Calculo real `stars_per_day = round(stars / max(1, dias_desde_criacao))` persistido e exibido como `+N est./dia` |
| Cobertura de coleta GitHub 7d | Apenas 15 repositorios (`stars:>50`) sem `created_at` salvo | Ate 40 repositorios (`stars:>=30`, janela movel de 7 dias UTC) com `created_at` real persistido no SQLite |
| Servidor MCP (`git_search_tops`) | Apenas `tops` e `novidades` | Suporte completo a `modo: "foguetes"` (com aliases `breakout` e `rockets`) e retorno de `stars_per_day` e `criado_em` |
| Iconografia e blindagem npm | Uso de `Sparkles` do `lucide-react` e ausencia de `.npmrc` | SVG autoral `RadarBadgeIcon` (zero estrelinhas genericas) e `frontend/.npmrc` blindado (`ignore-scripts=true`, `save-exact=true`) |

## 3. Arquivos Modificados

- `frontend/.npmrc` (novo, blindagem de cadeia de suprimentos Black October 2026)
- `frontend/src-tauri/src/db.rs` (migracao idempotente da coluna `created_at` em `repositories`, calculo real de `stars_per_day` e query `catalog_github` para `mode == "breakout"`)
- `frontend/src-tauri/src/github.rs` (expansao de `fetch_new_breakouts` para 40 itens na janela de 7 dias e persistencia de `created_at`)
- `frontend/src-tauri/src/catalog.rs` (suporte a `breakout` em GitHub, MCP 30d, Hugging Face Models 14d, Datasets 30d e Skills 7d + 2 novos testes unitarios)
- `frontend/src-tauri/src/mcp.rs` (exposicao de `modo: "foguetes"` no schema JSON-RPC e no payload de `git_search_tops`)
- `frontend/src/App.jsx` (suporte ao sub-modo `breakout` na navegacao lateral)
- `frontend/src/components/CatalogView.jsx` (pill `Foguetes 7d` na barra de modos e substituicao de `Sparkles` por `RadarBadgeIcon`)
- `frontend/src/components/ContentCard.jsx` (badge dourado `+{stars_per_day} est./dia` quando disponivel)
- `frontend/tests/content-card.test.js` (teste automatizado do badge de tracao diaria)

## 4. Evidencias de Testes Reais e Build

1. **Auditoria de repositorio (`shield_repository.py`):** `OK` (`frontend/.npmrc` criado e validado).
2. **Lint React (`npx oxlint src`):** `Found 0 warnings and 0 errors` em 15 arquivos.
3. **Testes Frontend (`npm test`):** `4 passed, 0 failed`.
4. **Testes Backend Rust (`cargo test --lib`):** `45 passed; 0 failed` (incluindo `github_breakout_ranks_recent_velocity_first` e `mcp_breakout_filters_recent_servers_by_velocity`).
5. **Build Tauri Release (`npx tauri build`):** `exit 0`. Instalador NSIS gerado com `4.55 MiB` (dentro do teto de 10 MB) e binario instalado atualizado em `%LOCALAPPDATA%\GitTrend Radar\gittrend-radar.exe` (com backup previo).
6. **Smoke Test MCP Ao Vivo (`gittrend-radar.exe --mcp`):** validado em producao local retornando foguetes reais de 2026-10-27:
   - `designfuchen/word-atlas` (`+1.209 est./dia`, 2.418 estrelas, TypeScript)
   - `JustAResearcher/gpu-unlock-ada` (`+1.053 est./dia`, 4.212 estrelas, C)
   - `rddmwmddr/android-ad-cleaner` (`+1.045 est./dia`, 3.135 estrelas, Python)
   - `t3dotgg/tstack` (`+640 est./dia`, 1.919 estrelas, TypeScript)
   - `wesbos/3d-model-anything` (`+364 est./dia`, 2.186 estrelas, Swift)

## 5. Limites e Pendencias

- Interacao manual por clique na janela GUI permanece `NOT_TESTED` em terminal headless de agente (conforme regra do `AGENTS.md` para nao travar `EBWebView` em sessao oculta). O binario em `%LOCALAPPDATA%\GitTrend Radar\gittrend-radar.exe` ja contem `frontend/dist` embutido via `npx tauri build`.
