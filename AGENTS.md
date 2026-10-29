# GitTrend Radar

App desktop (Tauri 2 + Rust + React) que mostra as novidades diárias do GitHub Trending.

## Stack

- `frontend/src` - React 19 + Vite. Chama o backend via `invoke` em `src/api.js`.
- `frontend/src-tauri/src` - backend Rust:
  - `github.rs` coleta (Trending geral + 12 linguagens + repos criados nos últimos 7 dias), README
  - `analyze.rs` análise heurística, tradução PT-BR com cache, Ollama opcional
  - `db.rs` SQLite em `%APPDATA%\com.bianchini.gittrendradar\gittrend.db`
  - `local.rs` clone/abrir pasta em `E:\REPOSITORIOS` (URL só `https://github.com/<dono>/<repo>`)
  - `lib.rs` comandos Tauri, sync ao abrir e a cada 4h
  - `models.rs` modelos de IA em alta no Hugging Face (API pública, cache de 1h; autenticação com token HF e autoaceite de termos para modelos restritos via ask-access)
  - `settings.rs` três destinos configuráveis, token GitHub protegido em disco via DPAPI e detecção automática de token Hugging Face (HF_TOKEN, E:\REPOSITORIOS\AI_MODELS\token ou cache oficial)
  - `downloads.rs` jobs, streaming e publicação sem sobrescrever arquivos existentes
  - `skills.rs` skills.sh + catálogos GitHub Anthropic/OpenAI, explicação e download em staging
  - `catalog.rs` três áreas (GitHub/Modelos/Skills) × Tops/Novidades/Explorar, páginas de 50, observações persistentes e avisos de cobertura
  - `summaries.rs` fila Ollama automática em cada página, tradução/resumo PT-BR, cache 7 dias, dedup/retry, máximo 500 ativos; documentos limitados a 1 MiB e tratados como dados inertes
  - Busca global: `search_github`, `search_hf`, `search_skills`; resultados GitHub usam `period = 'search'`, nunca contam como novidade diária
  - Downloads HF: leve sem pesos / completo; arquivos existentes preservados. Skills: `<pasta>/<dono>/<repo>/<skill>`; staging só vira destino final quando completo
  - `mcp.rs` servidor MCP stdio (`gittrend-radar.exe --mcp`), registrado no multiplexador do ALFRED como `git-search`: `git_search_tops`, `git_search_buscar`, `git_search_instalar_skill`, `git_search_pedir_download`, `git_search_pedidos`, `git_search_baixados_recentes` (o que foi baixado, com data/hora = criação da pasta; `dias=1` = hoje)
  - Pedidos de agentes: tabela `agent_requests` (pending→running→done/error/rejected); aba "Pedidos dos agentes" no app aprova/recusa
  - Favoritos: tabela `favorites`; Favoritos/Baixados vêm de `db::all_latest` (todos os repos já vistos, não só o Trending do dia)

## Comandos

```bash
cd frontend
npm run tauri dev                         # desenvolvimento
npx tauri build                           # exe + instalador NSIS em src-tauri/target/release/bundle/nsis
cd src-tauri && cargo test                # testes offline
cargo test -- --ignored --nocapture       # teste com rede real (GitHub + tradução)
cargo test live_download_roundtrip -- --ignored --nocapture --test-threads=1 # isolado: altera settings globais só no processo de teste
npx oxlint src                            # lint do React
npm test                                  # estados/polling de resumos (frontend)
cargo test live_hf_and_skill_summaries -- --ignored --nocapture # IA real HF+skill, com Ollama existente
```

## Limites

- Não inventar dados (estrelas do dia, exemplos de código). Sem dado: mostrar vazio ou só `git clone`.
- Hooks React sempre antes de qualquer `return` condicional.
- Layout é o do Gemini (decisão do Victor 15/10). Não redesenhar sem pedido. Sem ícones de estrela/brilho genéricos (pedido do Victor).
- Busca tem limites reais de fonte; nunca afirmar varredura completa da internet. SkillsMP exige chave própria e não está habilitado.
- Fluxos nativos (seletor, browser, notificações, controles da janela) precisam de teste próprio; build/IPC backend não são prova de clique.
- Novidades: GitHub = primeira aparição no Trending hoje; HF = criação convertida à data local; Skills = descoberta local (não data de lançamento).
- Resumo automático usa Ollama real; nunca substituir falha por heurística. Prioridade rápida qwen3:0.6b → smollm2:1.7b → qwen3.5:4b; até 2 frases/55 palavras, cache versionado brief-pt-v2. Fila/cache sobrevivem ao reinício.
- Decisão Victor (16/10, substitui a de 15/10): todos os itens e biblioteca usam ContentCard. Rodapé = Baixar direto + Detalhes lado a lado (+ Abrir pasta). Clique no meio do card ou em Detalhes expande o card inline em largura total, mostrando só dados reais da fonte (`cardDetails.js`). Sem IA EXPLICA e sem modal “Detalhes e download”. Pesos de modelos exigem seleção explícita no próprio card; agendamento não equivale a download concluído.
- Decisão Victor (17/10): skills pedidas pela MCP instalam AUTOMATICAMENTE em `%USERPROFILE%\.agents\skills\<nome>` + `sync-indexes.py` (sem checagem, risco aceito por ele); nunca sobrescreve skill existente. Repositórios e modelos pela MCP só baixam após aprovação no app.
- Card (17/10): só foto do dono (sem nome), título só o nome do projeto, "Resumo traduzido" (`description_pt` do catálogo) e embaixo "Resumo da IA".
- Decisão Victor (18/10): Baixados contabiliza corretamente tudo que foi baixado nos domínios (Repositórios GitHub, Modelos HF, Datasets e Skills). Itens baixados mudam de cor (borda e brilho cyber emerald, chip 'Na máquina' e botão destacado). No Catálogo, organização com 'Não baixados no topo' e filtro de status (Todos / Apenas não baixados / Apenas já baixados). Na aba Baixados, agrupamento e filtragem por categoria (Todos, GitHub, Servidores MCP, Modelos de IA, Datasets, Skills).
- Novas Categorias (18/10): Adicionados Servidores MCP (repositórios de MCP no GitHub para agentes e Claude Desktop) e Datasets de IA (descoberta no Hugging Face com paginação por cursor, modalidades e filtros).
- Decisão Victor (18/10, Datasets e Navegação): Removida completamente a aba Central de comando. Datasets de IA corrigidos ponta a ponta: extração limpa de descrição a partir do payload do Hugging Face (removendo tabs e linhas vazias), tradução em português com cache e resumos via Ollama (kind 'dataset'); download de arquivos e metadados via /api/datasets com resolução direta e salvamento em subpasta dedicada de datasets.
- Build e execução GUI (25/10): ao atualizar `%LOCALAPPDATA%\GitTrend Radar\gittrend-radar.exe`, usar sempre o binário gerado por `npx tauri build` (que inclui `tauri/custom-protocol` e embute `frontend/dist`), nunca `cargo build --release` isolado. Nunca iniciar a janela GUI direto de terminais não interativos de agentes para não travar `%LOCALAPPDATA%\com.bianchini.gittrendradar\EBWebView` em desktop oculto; validar backend/SQLite via `--mcp`.

