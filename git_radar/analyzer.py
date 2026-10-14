"""
Analyzer Engine for GitTrend Radar.
Extracts deep insights, simple explanations, practical examples and decision guides
from repository metadata and README contents.
"""

import re
import logging
from typing import Dict, Any, List, Optional
import httpx

from git_radar.models import RepoItem, RepoAnalysis

logger = logging.getLogger("git_radar.analyzer")


def clean_markdown_text(text: str) -> str:
    """Removes badges, html tags and images to keep clean prose."""
    # Remove HTML comments
    text = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    # Remove markdown images: ![alt](url)
    text = re.sub(r"!\[.*?\]\(.*?\)", "", text)
    # Remove badges like [![...](...)](...)
    text = re.sub(r"\[!\[.*?\]\(.*?\)\]\(.*?\)", "", text)
    # Remove HTML tags
    text = re.sub(r"<[^>]+>", "", text)
    # Normalize multiple whitespace and empty lines
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


def extract_code_blocks(readme: str) -> List[Dict[str, str]]:
    """Extracts fenced code blocks from markdown."""
    pattern = r"```([a-zA-Z0-9_\-\+]*)\n(.*?)```"
    matches = re.findall(pattern, readme, flags=re.DOTALL)
    blocks = []
    for lang, code in matches:
        lang = lang.strip().lower()
        code_clean = code.strip()
        if len(code_clean) > 15:
            blocks.append({"lang": lang or "text", "code": code_clean})
    return blocks


def extract_key_features(readme: str) -> List[str]:
    """Finds bullet points under Feature/Highlights sections."""
    lines = readme.split("\n")
    features = []
    in_features = False

    feature_headers = ["features", "recursos", "key features", "highlights", "o que faz", "capabilities", "why"]

    for line in lines:
        stripped = line.strip()
        header_match = re.match(r"^#{1,4}\s+(.*)", stripped)
        if header_match:
            header_title = header_match.group(1).lower()
            in_features = any(h in header_title for h in feature_headers)
            continue

        if in_features:
            # Check for bullet point
            bullet_match = re.match(r"^[\*\-\+]\s+\*{0,2}(.*?)\*{0,2}$", stripped)
            if bullet_match:
                feature_text = bullet_match.group(1).strip()
                # Clean links [text](url) -> text
                feature_text = re.sub(r"\[(.*?)\]\(.*?\)", r"\1", feature_text)
                if 10 < len(feature_text) < 160:
                    features.append(feature_text)
                    if len(features) >= 5:
                        break
            elif stripped == "" or stripped.startswith("#"):
                if stripped.startswith("#"):
                    in_features = False

    # Fallback: look for any bullet points in top 150 lines
    if not features:
        for line in lines[:150]:
            stripped = line.strip()
            bullet_match = re.match(r"^[\*\-\+]\s+\*{0,2}(.*?)\*{0,2}$", stripped)
            if bullet_match:
                ft = re.sub(r"\[(.*?)\]\(.*?\)", r"\1", bullet_match.group(1).strip())
                if 15 < len(ft) < 140 and not ft.startswith("http") and not ft.startswith("![]"):
                    features.append(ft)
                    if len(features) >= 4:
                        break

    return features[:5]


def extract_install_cmd(readme: str, lang: str) -> str:
    """Finds CLI installation or run command."""
    code_blocks = extract_code_blocks(readme)
    # Search in bash / sh / terminal blocks first
    for block in code_blocks:
        code = block["code"]
        for prefix in ["pip install", "npm install", "pnpm add", "yarn add", "cargo install", "docker run", "git clone", "uv pip install", "brew install", "go install"]:
            if prefix in code:
                for line in code.split("\n"):
                    if prefix in line:
                        return line.strip()

    # Generic fallback based on repo language
    lang_lower = lang.lower()
    if lang_lower == "python":
        return "pip install -r requirements.txt"
    elif lang_lower in ("typescript", "javascript"):
        return "npm install"
    elif lang_lower == "rust":
        return "cargo build --release"
    elif lang_lower == "go":
        return "go build ."
    return "git clone <url>"


class RepoAnalyzer:
    """Analyzes repositories and generates humanized, practical explanations."""

    def __init__(self, ollama_url: str = "http://localhost:11434"):
        self.ollama_url = ollama_url

    def analyze_heuristic(self, repo: RepoItem, readme: str = "") -> RepoAnalysis:
        """
        Deterministic, immediate NLP analyzer.
        Does not depend on external APIs, ensuring sub-second response times.
        """
        clean_text = clean_markdown_text(readme)
        first_paragraphs = [p.strip() for p in clean_text.split("\n\n") if len(p.strip()) > 30 and not p.strip().startswith("#")]

        # 1. Simple Summary (O que é em termos simples)
        simple_summary = ""
        if repo.description and len(repo.description) > 20:
            base_desc = repo.description.rstrip(".")
            if first_paragraphs:
                extra = first_paragraphs[0]
                # Avoid exact duplicate
                if extra.lower() not in base_desc.lower() and len(extra) < 300:
                    simple_summary = f"{base_desc}. {extra}"
                else:
                    simple_summary = base_desc + "."
            else:
                simple_summary = base_desc + "."
        elif first_paragraphs:
            simple_summary = first_paragraphs[0]
        else:
            simple_summary = f"Ferramenta em {repo.language} projetada para otimizar fluxos de desenvolvimento e automação."

        # Add plain-language explanation tailoring based on category
        cat = repo.category
        lang = repo.language

        category_intros = {
            "ia_agentes": "Este projeto atua na camada de inteligência artificial e agentes autônomos. Ele resolve a dor de integrar modelos de linguagem em fluxos reais sem complexidade excessiva de infraestrutura.",
            "frontend_ui": "Voltado para interfaces visuais e experiência do usuário. Ele oferece componentes prontos e arquitetura modular para acelerar a criação de produtos digitais com acabamento premium.",
            "backend_api": "Focado em serviços de alta performance no servidor. Ele elimina boilerplate e entrega rotas, persistência de dados e segurança de forma robusta e padronizada.",
            "devops_cli": "Utilitário direto de terminal e automação de infraestrutura. Permite gerenciar ambientes, contêineres e tarefas repetitivas diretamente pelo console com máxima velocidade.",
            "security": "Ferramenta especializada em cibersegurança, análise reversa ou auditoria. Ajuda a identificar vulnerabilidades, inspecionar binários ou proteger dados sensíveis.",
            "data_eng": "Projetado para engenharia de dados e pipelines de processamento. Trata ingestão, transformação e armazenamento de grandes volumes com eficiência.",
            "mobile": "Ecossistema para desenvolvimento móvel nativo ou híbrido, acelerando o ciclo de entrega de apps para iOS e Android."
        }

        prefix = category_intros.get(cat, f"Repositório em {lang} com foco em engenharia e produtividade técnica.")
        full_simple_summary = f"{prefix}\n\n{simple_summary}"

        # 2. Por que está em alta (Why Trending)
        why_trending_points = []
        if repo.stars_today > 0:
            why_trending_points.append(f"Viralizou com +{repo.stars_today:,} novas estrelas registradas hoje no GitHub.")
        if repo.stars > 10000:
            why_trending_points.append(f"Projeto maduro e consolidado com comunidade ativa de {repo.stars:,} estrelas e {repo.forks:,} forks.")
        elif repo.stars < 3000:
            why_trending_points.append("Projeto novo em fase de tração acelerada, ganhando tração imediata na comunidade open-source.")

        if cat == "ia_agentes":
            why_trending_points.append("Responde à forte demanda por soluções de IA autônomas, contextuais e locais sem dependência de serviços fechados.")
        elif cat == "devops_cli":
            why_trending_points.append("Otimiza o fluxo de trabalho do desenvolvedor ao substituir comandos verbosos por automações intuitivas.")
        elif cat == "frontend_ui":
            why_trending_points.append("Oferece design systems e padrões de interface alinhados com as tendências visuais modernas.")

        why_trending = " ".join(why_trending_points)

        # 3. Exemplo Prático de Código (Code Example)
        code_blocks = extract_code_blocks(readme)
        code_example = ""
        # Look for code in matching language or bash
        for block in code_blocks:
            c = block["code"]
            l = block["lang"]
            if l in ("python", "typescript", "javascript", "bash", "sh", "rust", "go", "json"):
                if 20 < len(c) < 1200:
                    code_example = f"```{l}\n{c}\n```"
                    break

        if not code_example and code_blocks:
            first_b = code_blocks[0]
            code_example = f"```{first_b['lang']}\n{first_b['code'][:800]}\n```"

        install_cmd = extract_install_cmd(readme, lang)
        if not code_example:
            # Generate realistic snippet
            if lang.lower() == "python":
                code_example = f"```python\n# Instalação:\n# {install_cmd}\n\nimport {repo.name.lower().replace('-', '_')}\n\n# Exemplo de inicialização rápida\nclient = {repo.name.lower().replace('-', '_')}.Client()\nresult = client.run()\nprint(result)\n```"
            elif lang.lower() in ("typescript", "javascript"):
                code_example = f"```typescript\n// Instalação:\n// {install_cmd}\n\nimport {{ {repo.name.replace('-', '')} }} from '{repo.name.lower()}';\n\nconst instance = new {repo.name.replace('-', '')}();\nawait instance.init();\n```"
            else:
                code_example = f"```bash\n# Clonar e executar\ngit clone {repo.url}\ncd {repo.name}\n{install_cmd}\n```"

        # 4. Quando Selecionar (When to select)
        when_to_select = []
        if cat == "ia_agentes":
            when_to_select = [
                "Você precisa construir fluxos autônomos de agentes que interagem com ferramentas ou código.",
                "Seu projeto demanda redução de latência e controle fino sobre o prompt e a memória.",
                "Você quer evitar vendor lock-in e rodar modelos locais ou customizados."
            ]
        elif cat == "frontend_ui":
            when_to_select = [
                "Você precisa de uma interface rica, responsiva e pronta para produção sem desenhar tudo do zero.",
                "Seu time valoriza acessibilidade e consistência visual moderna.",
                "Você está prototipando um MVP rápido e quer parecer um produto enterprise desde o primeiro dia."
            ]
        elif cat == "backend_api":
            when_to_select = [
                "Você quer erguer microserviços ou APIs com tipagem estrita e validação automática de dados.",
                "Sua arquitetura exige alta concorrência e baixo consumo de memória no servidor.",
                "Você precisa integrar bancos de dados modernos com migrações e consultas eficientes."
            ]
        elif cat == "devops_cli":
            when_to_select = [
                "Você quer automatizar tarefas repetitivas de setup, build ou deploy no terminal.",
                "Sua equipe busca padronizar o ambiente de desenvolvimento entre Windows, Linux e macOS.",
                "Você prefere ferramentas focadas em performance escritas em linguagens compiladas como Rust ou Go."
            ]
        elif cat == "security":
            when_to_select = [
                "Você realiza testes de invasão, auditorias de código ou engenharia reversa de binários.",
                "Precisa inspecionar pacotes e dependências em busca de vulnerabilidades e credenciais expostas.",
                "Seu fluxo de CI/CD requer um gate de segurança automatizado antes de ir para produção."
            ]
        else:
            when_to_select = [
                f"Você está desenvolvendo projetos na stack {lang} e quer uma biblioteca bem avaliada pela comunidade.",
                "Você precisa resolver um problema de engenharia sem reinventar a roda.",
                "Seu projeto busca código testado, documentado e com alta tração no GitHub."
            ]

        # 5. Quando Evitar (When to avoid)
        when_to_avoid = [
            f"Se sua equipe não tem familiaridade com a stack {lang} e a curva de aprendizado for um impeditivo.",
            "Se você precisa de uma solução estável há anos com garantias de LTS corporativo estrito.",
            "Se o escopo do seu projeto é trivial e uma biblioteca nativa da linguagem já resolve sem dependências extras."
        ]

        # 6. Quick Verdict (Veredito Rápido)
        if repo.stars_today > 1000:
            quick_verdict = f"Destaque absoluto do dia. Altamente recomendado para quem quer se antecipar às tendências em {lang} e {cat.replace('_', ' ').title()}."
        else:
            quick_verdict = f"Excelente recurso na categoria {cat.replace('_', ' ').title()}. Vale a pena clonar e testar o exemplo prático."

        # 7. Features e Tech Stack
        key_features = extract_key_features(readme)
        tech_stack = [lang] if lang != "Unknown" else []
        for tech in ["Docker", "Kubernetes", "PyTorch", "React", "Next.js", "FastAPI", "Tailwind", "Ollama", "PostgreSQL", "SQLite"]:
            if tech.lower() in clean_text.lower():
                tech_stack.append(tech)

        target_audience = "Desenvolvedores de software, arquitetos de sistemas e entusiastas de open-source."
        if cat == "ia_agentes":
            target_audience = "Engenheiros de IA, desenvolvedores de agentes e criadores de produtos com LLMs."
        elif cat == "frontend_ui":
            target_audience = "Desenvolvedores frontend, UI/UX designers e criadores de produtos web."
        elif cat == "security":
            target_audience = "Especialistas em cibersegurança, pesquisadores de vulnerabilidades e engenheiros reversos."
        elif cat == "devops_cli":
            target_audience = "Engenheiros de DevOps, SREs e desenvolvedores focados em produtividade no terminal."

        return RepoAnalysis(
            repo_id=repo.repo_id,
            simple_summary=full_simple_summary,
            why_trending=why_trending,
            code_example=code_example,
            when_to_select=when_to_select,
            when_to_avoid=when_to_avoid,
            quick_verdict=quick_verdict,
            key_features=key_features,
            tech_stack=tech_stack,
            target_audience=target_audience,
            install_cmd=install_cmd,
            analysis_source="heuristic"
        )

    def analyze_with_ollama(self, repo: RepoItem, readme: str = "", model: str = "qwen3:8b") -> Optional[RepoAnalysis]:
        """
        Deep LLM analysis via local Ollama instance when requested.
        """
        prompt = f"""Você é o analista técnico sênior da Bianchini Tech.
Analise este repositório do GitHub em alta e produza uma explicação direta, rica e em português do Brasil (sem travessões).

Repositório: {repo.repo_id}
Linguagem: {repo.language}
Descrição: {repo.description}
Estrelas Hoje: {repo.stars_today}
Estrelas Totais: {repo.stars}
Categoria: {repo.category}

Trecho do README:
{clean_markdown_text(readme)[:3000]}

Forneça sua resposta estritamente no seguinte formato estruturado:
[RESUMO_SIMPLES]
Explique em 2 parágrafos simples o que este projeto faz e qual problema real ele resolve.

[POR_QUE_DESTAQUE]
Explique por que viralizou hoje e qual é seu diferencial técnico.

[EXEMPLO_PRATICO]
Forneça um bloco de código prático ou comando de uso rápido.

[QUANDO_SELECIONAR]
- Ponto 1
- Ponto 2
- Ponto 3

[QUANDO_EVITAR]
- Ponto 1
- Ponto 2

[VEREDITO]
Uma frase definitiva de recomendação.
"""
        try:
            r = httpx.post(
                f"{self.ollama_url}/api/generate",
                json={"model": model, "prompt": prompt, "stream": False},
                timeout=60.0
            )
            if r.status_code != 200:
                return None
            
            raw_response = r.json().get("response", "")
            if not raw_response:
                return None

            # Parse sections from LLM output
            def get_section(tag: str, text: str) -> str:
                pattern = rf"\[{tag}\](.*?)(?=\[[A-Z_]+\]|$)"
                m = re.search(pattern, text, flags=re.DOTALL)
                return m.group(1).strip() if m else ""

            resumo = get_section("RESUMO_SIMPLES", raw_response) or repo.description
            why = get_section("POR_QUE_DESTAQUE", raw_response) or f"Em alta com +{repo.stars_today} estrelas hoje."
            exemplo = get_section("EXEMPLO_PRATICO", raw_response)
            veredito = get_section("VEREDITO", raw_response) or "Repositório recomendado para testes e validação."

            quando_sel_raw = get_section("QUANDO_SELECIONAR", raw_response)
            quando_sel = [l.strip("- *").strip() for l in quando_sel_raw.split("\n") if l.strip("- *").strip()]
            if not quando_sel:
                quando_sel = [f"Quando precisar de uma solução sólida em {repo.language}."]

            quando_evit_raw = get_section("QUANDO_EVITAR", raw_response)
            quando_evit = [l.strip("- *").strip() for l in quando_evit_raw.split("\n") if l.strip("- *").strip()]
            if not quando_evit:
                quando_evit = ["Quando o projeto for muito simples e dispensar novas dependências."]

            # Extract code block if inside example
            install_cmd = extract_install_cmd(readme, repo.language)
            features = extract_key_features(readme)

            return RepoAnalysis(
                repo_id=repo.repo_id,
                simple_summary=resumo.replace("—", ","),
                why_trending=why.replace("—", ","),
                code_example=exemplo,
                when_to_select=quando_sel,
                when_to_avoid=quando_evit,
                quick_verdict=veredito.replace("—", ","),
                key_features=features,
                tech_stack=[repo.language],
                target_audience="Desenvolvedores e engenheiros de software.",
                install_cmd=install_cmd,
                analysis_source=f"ollama_{model}"
            )
        except Exception as e:
            logger.warning(f"Ollama generation failed or timed out: {e}")
            return None
