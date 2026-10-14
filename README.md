# GitTrend Radar 🚀

> Radar diario inteligente de tendencias do GitHub, com atualizacao automatica, explicacoes em termos simples, exemplos praticos de codigo e guia de decisao para saber exatamente quando selecionar cada ferramenta.

---

## 💡 O que e o GitTrend Radar?

O **GitTrend Radar** e uma ferramenta autonoma criada para desenvolvedores e arquitetos que querem acompanhar os repositorios mais quentes do GitHub todos os dias, sem perder tempo decifrando documentacoes confusas.

Ao abrir o programa, ele:
1. **Atualiza automaticamente** os repositorios em alta (Trending Hoje, Destaques da Semana e do Mes).
2. **Resume em termos simples** o que cada ferramenta faz e qual dor real ela resolve.
3. **Extrai exemplos praticos de codigo** (quickstart e comandos de instalacao imediata).
4. **Gera uma matriz de decisao** ("Quando Selecionar" vs "Quando Evitar").
5. **Categoriza por area**: IA & Agentes Autonomos, Frontend, Backend, DevOps, Seguranca e Engenharia de Dados.
6. **Permite favoritar** os melhores projetos para estudar ou usar na empresa.

---

## 🛠️ Como Iniciar em 1 Clique

### No Windows:
- Dê um duplo clique no arquivo `iniciar.bat` (ou execute `iniciar.ps1` no PowerShell).
- O sistema inicia o servidor local em `http://127.0.0.1:8765` e abre automaticamente no **Brave Browser**.

### Pelo Terminal:
```bash
python run.py
```

---

## ⏰ Atualizacao Diaria Automatica

O GitTrend Radar possui 3 camadas de atualizacao:

1. **Auto-Update ao Abrir**: Ao iniciar a aplicacao, ele verifica se os dados de hoje ja foram baixados. Se nao foram, busca em segundo plano automaticamente.
2. **Botao "Atualizar Agora"**: No topo da interface, permite forçar uma nova sincronizacao a qualquer momento.
3. **Agendador do Windows (Opcional)**: Para atualizar todos os dias as 08:00 AM em segundo plano:
   ```powershell
   powershell -ExecutionPolicy Bypass -File setup_agendador.ps1
   ```

---

## 🧠 Arquitetura Tecnica

- **Backend**: FastAPI + Uvicorn de alta performance.
- **Banco de Dados**: SQLite local (`git_trending.db`) para historico de tendencias, cache de analises e favoritos.
- **Scraper & Parser**: `selectolax` + `httpx` (respeita a regra ZERO DOM no servidor, processando HTML em milissegundos).
- **Motor de Analise**:
  - Heuristico & Semantico (instantaneo, sub-segundo, sem custos ou tokens).
  - Inteligencia Artificial Local via Ollama (`qwen3:8b`, `qwen2.5-coder:7b`) ou LLM compativel quando solicitado.
- **Frontend**: Vue 3 Reativo (100% livre de manipulacao direta de DOM), com design system Techno-Futurist 2026, dark mode nativo, glassmorphism e cards bento grid.

---

## 📂 Estrutura do Projeto

```
GIT_SEARCH/
├── git_radar/
│   ├── db.py                 # Persistencia SQLite e consultas
│   ├── scraper.py            # Coletor de dados do GitHub Trending
│   ├── analyzer.py           # Motor de explicacao simples e guia de escolha
│   ├── scheduler.py          # Coordenador de atualizacao diaria
│   ├── server.py             # API REST FastAPI e rotas
│   └── models.py             # Modelos de dados Pydantic
├── static/
│   ├── index.html            # Interface web reativa (Vue 3)
│   ├── css/style.css         # Design system Techno-Futurist 2026
│   └── js/app.js             # Estado reativo e interacoes
├── run.py                    # Launcher principal (abre no Brave)
├── update_now.py             # Script CLI de atualizacao direta
├── setup_agendador.ps1       # Configurador da tarefa agendada do Windows
├── iniciar.bat               # Atalho duplo-clique para Windows
├── iniciar.ps1               # Launcher PowerShell
└── requirements.txt          # Dependencias Python
```

---

Desenvolvido para máxima agilidade e clareza técnica no ecossistema Bianchini.
