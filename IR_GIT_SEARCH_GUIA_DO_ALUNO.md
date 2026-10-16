# INSTRUTIVO DE REPOSITORIO (IR)
## GitTrend Radar (Git Search) v0.1.3
### Formacao Vibe Coder | Bianchini Academy

---

## 1. Identificacao e Visao Geral

O **GitTrend Radar** (conhecido no ecossistema como **Git Search**) e uma estacao de trabalho para desenvolvedores e Vibe Coders. O aplicativo rastreia diariamente as maiores tendencias do GitHub, modelos de inteligencia artificial no Hugging Face e skills de agentes autonomos, gerando resumos tecnicos sintetizados em portugues via IA local (Ollama) e oferecendo download direto com um unico clique.

* **Versao:** 0.1.3 (Release Oficial de Outubro de 2026)
* **Arquitetura:** Tauri 2 (Rust) + React 19 + Vite + SQLite + Ollama Local
* **Tamanho do Instalador:** Apenas 4,39 MiB (Instalador NSIS nativo para Windows x64)
* **Objetivo de Aprendizado:** Ensinar como construir aplicacoes desktop ultra-rapidas, que consom menos de 60 MB de memoria RAM e operam modelos de linguagem locais em segundo plano.

---

## 2. O Metodo CRIAR Aplicado ao Projeto

Na Bianchini Academy, todo projeto de software e guiado pelo **Metodo CRIAR**:

1. **Contexto:** Todo dia surgem centenas de novas bibliotecas no GitHub e no Hugging Face. Analisar repositorios em ingles lendo READMEs longos gera fadiga e perda de tempo.
2. **Realidade:** Solucoes baseadas em Electron consomem muita memoria e solucoes em Python costumam ser dificeis de distribuir para usuarios finais. Alem disso, chamar APIs pagas de IA para milhares de repositorios geraria custos desnecessarios de tokens.
3. **Imersao:** O projeto combina a velocidade nativa do Rust para scraping, banco de dados e gerenciamento de arquivos com a flexibilidade do React 19 no frontend, intermediados pelo runtime seguro do Tauri 2.
4. **Aplicacao:** O aluno aprende a estruturar comandos assincronos com `#[tauri::command]`, disparar eventos com `Emitter`, consumir bancos relacionais embarcados com `rusqlite` e consultar LLMs locais via API HTTP streaming.
5. **Resultado:** Uma ferramenta compacta, autonoma e produtiva que roda no computador do aluno, baixa ferramentas prontas para `E:\REPOSITORIOS` e organiza o radar de estudos diario.

---

## 3. Guia Rapido de Instalacao para o Aluno

### Opcao A: Instalacao Direta via Executavel (Mais Rapida)

Se o aluno deseja apenas utilizar o aplicativo de imediato:

1. Localize o instalador gerado no diretorio do projeto:
   `frontend/src-tauri/target/release/bundle/nsis/GitTrend Radar_0.1.3_x64-setup.exe`
2. De um duplo clique no arquivo para iniciar a instalacao automatica.
3. Apos a conclusao, o icone estara disponivel no Menu Iniciar do Windows. Clique com o botao direito e selecione **Fixar na barra de tarefas**.

### Opcao B: Execucao em Modo de Desenvolvimento (Para Estudar e Codar)

Se o aluno quer abrir o codigo, alterar telas e compilar:

#### Pre-requisitos:
* **Node.js:** Versao 20 ou superior (Recomendado: Node 22 ou 24).
* **Rust & Cargo:** Versao 1.85 ou superior instalada via `rustup`.
* **Ollama (Opcional, mas recomendado para IA local):** Instalado com o modelo `qwen3:0.6b` ou `smollm2:1.7b` puxado via comando `ollama run qwen3:0.6b`.
* **Git e Git LFS:** Instalados para que o download de modelos pesados funcione corretamente (`winget install GitHub.GitLFS`).

#### Passo a Passo no Terminal:

```powershell
# 1. Acesse o diretorio frontend
cd "E:\REPOSITORIOS\GIT_SEARCH\frontend"

# 2. Instale as dependencias do React e Vite
npm install

# 3. Inicie o ambiente de desenvolvimento do Tauri 2
npx tauri dev
```

O Tauri compilara o backend Rust e abrira a janela nativa com Hot Module Replacement (HMR) ativo. Qualquer alteracao no React sera refletida instantaneamente na tela.

---

## 4. Como o Git Search Funciona por Dentro

### A. As Tres Vitrines Principais

1. **GitHub:**
   - **Tops:** Exibe a ultima coleta do GitHub Trending filtrada por estrelas acumuladas.
   - **Novidades de Hoje:** Mostra exclusivamente repositorios que entraram no ranking pela primeira vez na data atual.
   - **Explorar Todos:** Permite pesquisar qualquer termo, linguagem (`language:rust`, `language:python`) ou topico (`topic:mcp`).
   - **Acao Baixar:** Clona o repositorio diretamente para a pasta configurada.

2. **Modelos de IA:**
   - Monitora modelos em alta no Hugging Face e no GitHub.
   - Permite baixar o cartao do modelo com ou sem os pesos binarios, economizando espaco em disco quando o objetivo for apenas inspecionar a arquitetura.

3. **Skills de Agentes:**
   - Busca catalogos publicos de skills (Anthropic, OpenAI e skills.sh).
   - Baixa a pasta organizada no formato `<autor>/<repo>/<skill>` para ser utilizada em agentes como Antigravity, Claude Code e Codex.

### B. Resumo Inteligente com IA Local

O aplicativo nao envia os dados para APIs na nuvem. Em vez disso, ele utiliza uma fila controlada que consulta o **Ollama** rodando localmente:

* **Tamanho maximo lido:** 3.200 caracteres de documentacao (README.md ou SKILL.md).
* **Prompt estruturado:** Gera um resumo conciso em portugues de ate duas frases e no maximo 55 palavras.
* **Cache em SQLite:** O resumo fica salvo no banco local por 7 dias, evitando retrabalho computacional.

---

## 5. Mapa do Codigo: Onde o Aluno Deve Mexer

Para aprender e exercitar a programacao, o aluno deve explorar os seguintes arquivos centrais:

| Componente | Arquivo | Responsabilidade |
|---|---|---|
| **Ponto de Entrada Rust** | `src-tauri/src/lib.rs` | Registra os comandos Tauri, inicializa o banco SQLite e orquestra a sincronizacao a cada 4 horas. |
| **Integracao GitHub** | `src-tauri/src/github.rs` | Realiza o scraping do GitHub Trending e consultas a API oficial. |
| **Catalogos e Modelos** | `src-tauri/src/catalog.rs` | Conecta com o Hugging Face e catalogos de skills publicas. |
| **Motor de IA Local** | `src-tauri/src/summaries.rs` | Gerencia a fila de resumos com Ollama e armazena respostas no banco. |
| **Card de Exibicao** | `src/components/ContentCard.jsx` | Componente visual do card com estados de download, resumo e expansao de detalhes. |
| **Construtor de Detalhes** | `src/components/cardDetails.js` | Formata descricao, veredito, publico-alvo e fatos tecnicos do projeto. |

---

## 6. Desafios Praticos para o Aluno (Treinamento)

Para fixar o aprendizado, execute os tres exercicios abaixo:

1. **Exercicio 1 (Frontend):** No arquivo `frontend/src/components/ContentCard.jsx`, adicione um botao de copia rapida para transferir o link do repositorio para a area de transferencia.
2. **Exercicio 2 (Backend Rust):** No arquivo `src-tauri/src/summaries.rs`, altere o limite de palavras do resumo de 55 para 70 palavras e observe como a IA se comporta.
3. **Exercicio 3 (Nova Feature):** Crie um novo comando Tauri em `src-tauri/src/lib.rs` chamado `export_favorites_to_markdown` que leia os repositorios favoritados no SQLite e gere um arquivo `meus_favoritos.md` na pasta Documentos.

---

## 7. Solucao de Problemas Comuns (Troubleshooting)

* **O aplicativo abre, mas diz que o Ollama esta desconectado:**
  Abra um terminal e rode `ollama serve`. Certifique-se de que o modelo `qwen3:0.6b` ou `smollm2:1.7b` foi baixado.
* **Erro ao clonar repositorio grande:**
  Repositorios com modelos ou datasets exigem o Git LFS. Instale com o comando `winget install GitHub.GitLFS` no Windows.
* **Como mudar a pasta de download:**
  Abra a aba **Configuracoes** no menu lateral e selecione o diretorio de sua preferencia (o padrao e `E:\REPOSITORIOS`).
