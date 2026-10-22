# GitTrend Radar

Programa para Windows que mostra todo dia as novidades do GitHub: Trending geral, Trending por linguagem e repositórios criados na última semana, com explicação em português e clone direto para `E:\REPOSITORIOS`.

Feito com Tauri 2 (Rust) + React. Não depende de Python.

## Instalar

```bash
cd frontend
npm install
npx tauri build
```

Execute `frontend/src-tauri/target/release/bundle/nsis/GitTrend Radar_<versao>_x64-setup.exe`. Depois, no Menu Iniciar, clique com o botão direito em "GitTrend Radar" e escolha **Fixar na barra de tarefas**.

## Uso

- Ao abrir, o app busca as novidades do dia sozinho e repete a cada 4h enquanto estiver aberto.
- A aba **Novidades de Hoje** mostra só os repositórios que apareceram pela primeira vez no dia.
- **Baixar Repo** clona para `E:\REPOSITORIOS`. Repositórios com Git LFS precisam de `winget install GitHub.GitLFS` para trazer os arquivos grandes.
- O resumo curto em português é preenchido automaticamente com um modelo local rápido.

## Cards simples e resumos rápidos (0.1.3)

O menu tem três áreas independentes: **GitHub**, **Modelos de IA** e **Skills**. Cada uma tem **Tops**, **Novidades de hoje** e **Explorar todos**, com até **50 itens por página**. Favoritos, Baixados, Downloads e Configurações continuam separados.

Todos os itens usam o mesmo card: nome, categoria, métricas disponíveis, **Resumo** e **Baixar**. A listagem mostra um resumo curto em português, sem “IA explica”, tutoriais ou telas de detalhes. O download é direto no card; após concluir, aparece **Abrir pasta**. Para modelos, a opção padrão baixa arquivos sem pesos; os pesos completos exigem seleção explícita no próprio card. O progresso real do job mantém o botão ocupado e só confirma sucesso quando termina.

Cada página coloca automaticamente seus itens na fila do **Ollama local**, que traduz e resume o inglês do README, model card ou SKILL.md em até **duas frases e 55 palavras**. Os cards mostram “Resumindo…” ou erro recuperável até o resultado ficar pronto. Favoritos e Baixados usam o mesmo padrão e fila por páginas de 50. As explicações longas do cache anterior não são reaproveitadas pelo novo perfil `brief-pt-v2`.

O motor prioriza `qwen3:0.6b`, depois `smollm2:1.7b` e `qwen3.5:4b`, sem baixar modelos novos. A entrada foi reduzida para 3.200 caracteres de documentação, contexto 2.048 e até 240 tokens de saída, com raciocínio desativado. A inferência é sequencial para controlar consumo local, enquanto a navegação continua disponível. O primeiro carregamento do modelo pode demorar mais; não há prazo fixo por lote. Traduções/resumos persistem no SQLite por sete dias ou até os metadados/perfil mudarem, e trabalhos pendentes são retomados ao abrir. Há limite de 500 trabalhos ativos; excesso e indisponibilidade do Ollama aparecem como erros recuperáveis.

| Área | Tops | Novidades de hoje | Explorar todos |
| --- | --- | --- | --- |
| GitHub | Última coleta Trending por estrelas do período | Primeira aparição no Trending na data local atual | Busca pública com filtros e páginas de 50 |
| Modelos de IA — Hugging Face | Tendência da fonte | Data de criação convertida para a data local | Busca paginada por cursor |
| Modelos de IA — GitHub | Projetos relacionados a modelos por estrelas | Consulta `created:>=data` do GitHub | Busca por termos de modelos de texto/imagem; inclui ferramentas e cursos |
| Skills | Instalações e estrelas informadas pelas fontes | Primeira descoberta nesta instalação, inclusive carga inicial | Busca nas fontes disponíveis |

Skills sem data de publicação não são apresentadas como lançamentos. Repositórios sem README podem ser resumidos apenas pelos metadados, com a limitação declarada; falha HTTP de servidor continua erro. Categorias técnicas locais filtram antes da paginação; nos catálogos remotos o filtro de categoria se aplica à página atual, conforme aviso na tela. O catálogo GitHub de modelos encontra projetos relacionados, sem atestar existência de pesos em todos os resultados.

Os [parâmetros de saída estruturada do Ollama](https://docs.ollama.com/api/generate) são usados para separar tradução e resumo. Documentos são dados não confiáveis: o aplicativo não executa instruções nem código de README/SKILL.md, e a leitura é limitada a 1 MiB. Resumos gerados devem ser conferidos na documentação antes de decisões técnicas.

Testes adicionais:

```powershell
cd frontend
npm test
cd src-tauri
cargo test summaries::tests --lib
cargo test catalog::tests --lib
cargo test live_single_summary -- --ignored --nocapture
cargo test live_hf_and_skill_summaries -- --ignored --nocapture
```

As duas últimas verificações precisam de Ollama com modelo já instalado e internet; não fazem download de modelos. `frontend/tests/catalog-fixture.html` é um harness de UI com dados sintéticos e banner explícito, servido só no desenvolvimento. Ele não é importado pelo ponto de entrada de produção. Automação de cliques na janela nativa ainda precisa de ambiente disponível.

## Busca global, modelos e skills

- **Buscar no GitHub** consulta repositórios pela API, com filtros (`language:rust`, `topic:mcp`, `user:...`), ordenação e paginação. Detalhes leem o README para explicar o projeto em português. Os resultados pesquisados entram na biblioteca, sem alterar as novidades do Trending.
- **Modelos de IA** mantém a vitrine em alta e permite buscar no catálogo público do Hugging Face e em projetos relacionados no GitHub, com o mesmo resumo curto e botão Baixar. Download leve exclui pesos; completo inclui pesos. Modelos restritos exigem acesso pelo site.
- **Skills** consulta skills.sh e os catálogos públicos da Anthropic e OpenAI no GitHub. Mostra o resumo do SKILL.md e baixa a pasta pelo próprio card. Não instala nem executa os scripts baixados. Os destinos distinguem `<dono>/<repo>/<skill>` para evitar colisões.
- **Configurações** permite trocar separadamente as pastas GitHub, Hugging Face e Skills, digitando o caminho ou usando o seletor. A mudança vale para os próximos downloads e não move arquivos anteriores. Um token GitHub é opcional; fica protegido pelo DPAPI do Windows em `settings.json`, sem voltar inteiro à tela.
- **Downloads** acompanha modelos e skills da sessão. Mantenha o app aberto até concluir. Arquivos existentes são preservados; download leve seguido de completo acrescenta os pesos. O conteúdo dos arquivos previamente existentes não é revalidado. Skills só aparecem como concluídas após todos os arquivos; falhas ficam em pasta temporária identificada e permitem nova tentativa.

A busca cobre o catálogo acessível de cada fonte, sem espelhar todo o GitHub, Hugging Face ou a internet. O GitHub fornece [até 1.000 resultados por consulta](https://docs.github.com/en/rest/search/search#about-search); refine os filtros para encontrar outros resultados. As [APIs do Hugging Face](https://huggingface.co/docs/hub/en/api) usam paginação. SkillsMP não está habilitado porque sua [API exige chave própria](https://skillsmp.com/docs/api).

## Verificação

```powershell
cd frontend
npm run lint
npm run build
cd src-tauri
cargo fmt --check
cargo test --lib
# Rede real, em execução isolada (settings globais no processo de teste):
cargo test live_global_catalogs -- --ignored --nocapture
cargo test live_sources_smoke -- --ignored --nocapture
cargo test live_download_roundtrip -- --ignored --nocapture --test-threads=1
```

O roundtrip usa pastas temporárias: clona um projeto público, baixa um modelo de teste leve e completo (cerca de 28 MB) e uma skill com seus arquivos. Não executa o conteúdo baixado. Verifique também os controles na janela nativa; testes do backend e build não comprovam cliques na interface.

Dados ficam em `%APPDATA%\com.bianchini.gittrendradar\gittrend.db`.

Detalhes técnicos e comandos de desenvolvimento: [AGENTS.md](AGENTS.md).
