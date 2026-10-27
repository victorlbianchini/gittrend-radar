use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::Connection;

use crate::db::{self, Analysis, Repo};

const OLLAMA: &str = "http://localhost:11434";

static RE_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
static RE_IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static RE_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());
static RE_CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```([A-Za-z0-9_+\-]*)[^\n]*\n(.*?)```").unwrap());
static RE_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,4}\s+(.*)").unwrap());
static RE_BULLET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[*+\-]\s+\**(.*?)\**$").unwrap());
static RE_SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([A-Z_]+)\]").unwrap());

pub fn clean_markdown(text: &str) -> String {
    let t = RE_COMMENT.replace_all(text, "");
    let t = RE_IMAGE.replace_all(&t, "");
    let t = RE_TAG.replace_all(&t, "");
    t.trim().to_string()
}

/// Sem emojis e sem travessao (padrao do Victor).
pub fn tidy(text: &str) -> String {
    let t = RE_LINK.replace_all(text, "$1");
    let t: String = t
        .chars()
        .filter(|c| !matches!(*c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F))
        .collect();
    t.replace('—', ", ")
        .replace('–', "-")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Traduz para PT-BR via endpoint publico do Google, com cache no SQLite. Sem rede: devolve o original.
pub fn translate(http: &Client, conn: &Connection, text: &str) -> String {
    try_translate(http, conn, text).unwrap_or_else(|| tidy(text))
}

/// Como `translate`, mas `None` quando nao foi possivel traduzir (para a tela nao rotular original como traducao).
pub fn try_translate(http: &Client, conn: &Connection, text: &str) -> Option<String> {
    let src = tidy(text);
    if src.is_empty() {
        return None;
    }
    if let Some(pt) = db::cached_translation(conn, &src) {
        return Some(pt);
    }
    let q: String = src.chars().take(1500).collect();
    let resp = http
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", "pt"),
            ("dt", "t"),
            ("q", &q),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .and_then(|r| r.json::<serde_json::Value>());
    let pt = resp
        .ok()
        .and_then(|v| {
            v[0].as_array().map(|segs| {
                segs.iter()
                    .filter_map(|s| s[0].as_str())
                    .collect::<String>()
            })
        })
        .map(|s| tidy(&s))
        .filter(|s| !s.is_empty());
    if let Some(pt) = &pt {
        db::save_translation(conn, &src, pt);
    }
    pt
}

pub fn code_blocks(readme: &str) -> Vec<(String, String)> {
    RE_CODE
        .captures_iter(readme)
        .map(|c| (c[1].to_lowercase(), c[2].trim().to_string()))
        .filter(|(_, code)| code.len() > 15)
        .collect()
}

pub fn key_features(readme: &str) -> Vec<String> {
    const HEADERS: &[&str] = &["features", "recursos", "highlights", "capabilities", "why"];
    let mut out = vec![];
    let mut in_section = false;
    for line in readme.lines().map(str::trim) {
        if let Some(h) = RE_HEADER.captures(line) {
            let h = h[1].to_lowercase();
            in_section = HEADERS.iter().any(|k| h.contains(k));
            continue;
        }
        if in_section {
            if let Some(b) = RE_BULLET.captures(line) {
                let f = RE_LINK.replace_all(b[1].trim(), "$1").to_string();
                if f.len() > 10 && f.len() < 160 {
                    out.push(f);
                }
                if out.len() >= 4 {
                    break;
                }
            }
        }
    }
    out
}

pub fn install_cmd(readme: &str, lang: &str) -> String {
    const PREFIXES: &[&str] = &[
        "pip install",
        "uv pip install",
        "uv add",
        "npm install",
        "npx ",
        "pnpm add",
        "yarn add",
        "cargo install",
        "docker run",
        "brew install",
        "go install",
        "winget install",
    ];
    for (_, code) in code_blocks(readme) {
        if let Some(line) = code
            .lines()
            .find(|l| PREFIXES.iter().any(|p| l.contains(p)))
        {
            return line.trim().trim_start_matches("$ ").to_string();
        }
    }
    match lang.to_lowercase().as_str() {
        "python" => "pip install -r requirements.txt",
        "typescript" | "javascript" => "npm install",
        "rust" => "cargo build --release",
        "go" => "go build .",
        _ => "",
    }
    .into()
}

fn category_label(cat: &str) -> &'static str {
    match cat {
        "ia_agentes" => "IA e Agentes",
        "frontend_ui" => "Frontend e UI",
        "backend_api" => "Backend e APIs",
        "devops_cli" => "DevOps e CLI",
        "security" => "Segurança",
        "data_eng" => "Engenharia de Dados",
        "mobile" => "Mobile",
        _ => "Geral",
    }
}

pub fn fmt_int(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn heuristic(http: &Client, conn: &Connection, repo: &Repo, readme: &str) -> Analysis {
    let readme = readme.replace("\r\n", "\n");
    let lang = &repo.language;
    let clean = clean_markdown(&readme);
    let description_pt = translate(http, conn, &repo.description);

    let mut summary = vec![];
    if !description_pt.is_empty() {
        summary.push(description_pt.clone());
    }
    let desc_lower = repo.description.to_lowercase();
    if let Some(p) = clean
        .split("\n\n")
        .filter(|p| !p.trim_start().starts_with(['#', '>', '|', '`', '<']))
        .map(tidy)
        // Prosa de verdade: ignora seletores de idioma ("English | 中文"), tabelas e listas de links
        .find(|p| {
            p.chars().filter(|c| c.is_alphabetic()).count() > 60
                && p.contains('.')
                && !p.contains('|')
                && !p.starts_with(['*', '-', '['])
                && (desc_lower.is_empty() || !p.to_lowercase().contains(&desc_lower))
        })
    {
        summary.push(translate(
            http,
            conn,
            &p.chars().take(600).collect::<String>(),
        ));
    }
    if summary.is_empty() {
        summary.push(format!(
            "Projeto em {lang} na área de {}. O README não traz uma descrição clara.",
            category_label(&repo.category)
        ));
    }

    let mut why = vec![];
    if repo.stars_today > 0 {
        why.push(format!(
            "Ganhou +{} estrelas hoje no GitHub Trending.",
            fmt_int(repo.stars_today)
        ));
    } else {
        why.push("Repositório criado nesta semana que já acumulou estrelas rapidamente.".into());
    }
    if repo.stars > 10_000 {
        why.push(format!(
            "Comunidade grande: {} estrelas e {} forks.",
            fmt_int(repo.stars),
            fmt_int(repo.forks)
        ));
    } else if repo.stars < 2_500 {
        why.push(format!("Projeto novo em ascensão na comunidade {lang}."));
    }

    let features: Vec<String> = key_features(&readme)
        .iter()
        .map(|f| translate(http, conn, f).trim_end_matches('.').to_string())
        .collect();
    if let Some(f) = features.first() {
        why.push(format!("Recurso principal: {f}."));
    }

    let install = install_cmd(&readme, lang);
    let blocks = code_blocks(&readme);
    const PREFERRED: &[&str] = &[
        "python",
        "typescript",
        "ts",
        "javascript",
        "js",
        "bash",
        "sh",
        "shell",
        "rust",
        "go",
        "json",
        "toml",
        "yaml",
    ];
    // ponytail: so codigo real do README; sem exemplo, mostra apenas clone + instalacao (nada inventado)
    let code_example = blocks
        .iter()
        .find(|(l, c)| PREFERRED.contains(&l.as_str()) && c.len() < 1200)
        .or(blocks.first())
        .map(|(l, c)| {
            format!(
                "```{}\n{}\n```",
                if l.is_empty() { "text" } else { l },
                c.chars().take(1200).collect::<String>()
            )
        })
        .unwrap_or_else(|| {
            let extra = if install.is_empty() {
                String::new()
            } else {
                format!("\n{install}")
            };
            format!(
                "```bash\ngit clone {}\ncd {}{extra}\n```",
                repo.url, repo.name
            )
        });

    let when_to_select: Vec<String> = match repo.category.as_str() {
        "ia_agentes" => vec![
            "Você precisa construir fluxos de agentes que usam ferramentas, código ou modelos.",
            "Quer controle fino sobre prompt, memória e custo, de preferência rodando local.",
            "Quer evitar dependencia de um único fornecedor de IA.",
        ],
        "frontend_ui" => vec![
            "Você precisa de interface pronta para produção sem desenhar tudo do zero.",
            "Seu time valoriza consistência visual e acessibilidade.",
            "Está prototipando um MVP e quer acabamento profissional desde o início.",
        ],
        "backend_api" => vec![
            "Você está erguendo APIs ou serviços e quer validação e tipagem fortes.",
            "Sua arquitetura exige concorrência alta e uso baixo de memória.",
            "Precisa integrar bancos de dados com consultas eficientes.",
        ],
        "devops_cli" => vec![
            "Você quer automatizar setup, build ou deploy pelo terminal.",
            "Precisa padronizar o ambiente entre Windows, Linux e macOS.",
            "Prefere ferramentas rapidas, de binário único.",
        ],
        "security" => vec![
            "Você faz pentest, auditoria de código ou engenharia reversa.",
            "Precisa varrer dependencias atras de vulnerabilidades ou segredos expostos.",
            "Quer um gate de segurança automatizado no CI/CD.",
        ],
        _ => vec![
            "Você trabalha com a stack do projeto e quer algo bem avaliado pela comunidade.",
            "Precisa resolver o problema sem reinventar a roda.",
            "Busca código com tração e manutenção ativa no GitHub.",
        ],
    }
    .into_iter()
    .map(String::from)
    .collect();

    let when_to_avoid = vec![
        format!("Seu time não domina {lang} e a curva de aprendizado pesa."),
        "Você precisa de estabilidade de anos com suporte LTS corporativo.".into(),
        "O problema é trivial e a biblioteca padrão da linguagem já resolve.".into(),
    ];

    let quick_verdict = if repo.stars_today > 1000 {
        format!(
            "Destaque do dia. Vale olhar agora se você acompanha {}.",
            category_label(&repo.category)
        )
    } else {
        format!(
            "Bom recurso em {}. Vale clonar e testar o exemplo.",
            category_label(&repo.category)
        )
    };

    let mut tech_stack: Vec<String> = if lang != "Unknown" {
        vec![lang.clone()]
    } else {
        vec![]
    };
    let low = clean.to_lowercase();
    for t in [
        "Docker",
        "Kubernetes",
        "PyTorch",
        "React",
        "Next.js",
        "FastAPI",
        "Tailwind",
        "Ollama",
        "PostgreSQL",
        "SQLite",
        "Tauri",
        "MCP",
    ] {
        if low.contains(&t.to_lowercase()) && !tech_stack.iter().any(|s| s == t) {
            tech_stack.push(t.into());
        }
    }

    let target_audience = match repo.category.as_str() {
        "ia_agentes" => "Engenheiros de IA e quem cria produtos com LLMs.",
        "frontend_ui" => "Desenvolvedores frontend e designers de produto.",
        "security" => "Profissionais de segurança e engenharia reversa.",
        "devops_cli" => "DevOps, SREs e quem vive no terminal.",
        _ => "Desenvolvedores de software em geral.",
    }
    .into();

    Analysis {
        description_pt,
        simple_summary: summary.join("\n\n"),
        why_trending: why.join(" "),
        code_example,
        when_to_select,
        when_to_avoid,
        quick_verdict,
        key_features: features,
        tech_stack,
        target_audience,
        install_cmd: install,
        analysis_source: "heuristic".into(),
    }
}

pub fn ollama_models(http: &Client) -> Option<Vec<String>> {
    let v: serde_json::Value = http
        .get(format!("{OLLAMA}/api/tags"))
        .timeout(Duration::from_millis(1500))
        .send()
        .ok()?
        .json()
        .ok()?;
    Some(
        v["models"]
            .as_array()?
            .iter()
            .filter_map(|m| m["name"].as_str().map(String::from))
            .collect(),
    )
}

pub fn best_model(models: &[String]) -> Option<String> {
    const PRIORITY: &[&str] = &[
        "nemotron-bolt:e4b",
        "qwen3.5:4b",
        "qwen3:8b",
        "qwen2.5-coder:7b",
        "llama3.1:8b",
        "gemma4:12b",
        "hermes3:8b",
        "qwen2.5:7b",
    ];
    let text_models: Vec<_> = models
        .iter()
        .filter(|m| {
            let name = m.to_lowercase();
            !name.contains("embed") && !name.contains("bge-") && !name.contains("nomic")
        })
        .collect();
    PRIORITY
        .iter()
        .find_map(|p| {
            text_models
                .iter()
                .find(|m| m.as_str() == *p)
                .map(|m| (*m).clone())
        })
        .or_else(|| text_models.first().map(|m| (*m).clone()))
}

fn section(text: &str, tag: &str) -> String {
    let marks: Vec<_> = RE_SECTION
        .captures_iter(text)
        .map(|c| (c[1].to_string(), c.get(0).unwrap()))
        .collect();
    marks
        .iter()
        .enumerate()
        .find(|(_, (t, _))| t == tag)
        .map(|(i, (_, m))| {
            let end = marks
                .get(i + 1)
                .map(|(_, n)| n.start())
                .unwrap_or(text.len());
            text[m.end()..end].trim().to_string()
        })
        .unwrap_or_default()
}

fn bullets(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| {
            l.trim_matches(|c: char| c == '-' || c == '*' || c.is_whitespace())
                .trim_end_matches('.')
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// Analise profunda com LLM local. Mantem os campos da heuristica que o modelo nao cobre.
pub fn ollama(
    http: &Client,
    conn: &Connection,
    repo: &Repo,
    readme: &str,
    model: &str,
    base: Analysis,
) -> Result<Analysis, String> {
    let prompt = format!(
        "Você e um analista tecnico senior. Analise este repositorio do GitHub em alta e responda 100% em portugues do Brasil, sem travessoes.\n\n\
Repositorio: {}\nLinguagem: {}\nDescricao: {}\nEstrelas hoje: {}\nEstrelas totais: {}\n\nTrecho do README:\n{}\n\n\
Responda exatamente neste formato:\n[RESUMO_SIMPLES]\n2 paragrafos simples: o que faz e qual problema real resolve.\n\
[POR_QUE_DESTAQUE]\nPor que esta em alta e qual o diferencial tecnico.\n[EXEMPLO_PRATICO]\nUm bloco de codigo ou comando real de uso rapido.\n\
[QUANDO_SELECIONAR]\n- ponto 1\n- ponto 2\n- ponto 3\n[QUANDO_EVITAR]\n- ponto 1\n- ponto 2\n[VEREDITO]\nUma frase de recomendacao.",
        repo.repo_id,
        repo.language,
        repo.description,
        repo.stars_today,
        repo.stars,
        clean_markdown(readme)
            .chars()
            .take(3000)
            .collect::<String>()
    );
    let v: serde_json::Value = http
        .post(format!("{OLLAMA}/api/generate"))
        .json(&serde_json::json!({"model": model, "prompt": prompt, "stream": false, "think": false}))
        .timeout(Duration::from_secs(180))
        .send()
        .map_err(|e| format!("Ollama indisponivel: {e}"))?
        .json()
        .map_err(|e| format!("Resposta inválida do Ollama: {e}"))?;
    let raw = v["response"].as_str().unwrap_or_default();
    // Modelos com raciocinio (qwen3) mandam <think>...</think> antes da resposta
    let raw = raw.rsplit("</think>").next().unwrap_or(raw);
    let resumo = section(raw, "RESUMO_SIMPLES");
    if resumo.is_empty() {
        return Err("O modelo não respondeu no formato esperado.".into());
    }
    let pt = |s: String| {
        if s.is_empty() {
            s
        } else {
            translate(http, conn, &s)
        }
    };
    let or = |s: String, d: String| if s.is_empty() { d } else { s };
    let sel = bullets(&section(raw, "QUANDO_SELECIONAR"));
    let avoid = bullets(&section(raw, "QUANDO_EVITAR"));
    Ok(Analysis {
        simple_summary: resumo.replace('—', ", "),
        why_trending: or(
            pt(section(raw, "POR_QUE_DESTAQUE")),
            base.why_trending.clone(),
        ),
        code_example: or(section(raw, "EXEMPLO_PRATICO"), base.code_example.clone()),
        quick_verdict: or(pt(section(raw, "VEREDITO")), base.quick_verdict.clone()),
        when_to_select: if sel.is_empty() {
            base.when_to_select.clone()
        } else {
            sel
        },
        when_to_avoid: if avoid.is_empty() {
            base.when_to_avoid.clone()
        } else {
            avoid
        },
        analysis_source: format!("ollama_{model}"),
        ..base
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_model_priority_and_embedding_exclusion() {
        let with_bolt = [
            "nomic-embed-text:latest",
            "qwen3.5:4b",
            "nemotron-bolt:e4b",
            "qwen3:8b",
        ]
        .map(str::to_owned);
        assert_eq!(best_model(&with_bolt).as_deref(), Some("nemotron-bolt:e4b"));

        let models = [
            "nomic-embed-text:latest",
            "qwen3.5:4b",
            "smollm2:1.7b",
            "qwen3:0.6b",
        ]
        .map(str::to_owned);
        assert_eq!(best_model(&models).as_deref(), Some("qwen3.5:4b"));
        assert_eq!(best_model(&models[..3]).as_deref(), Some("qwen3.5:4b"));
        assert_eq!(best_model(&models[..2]).as_deref(), Some("qwen3.5:4b"));
        assert!(best_model(&models[..1]).is_none());
    }

    #[test]
    fn extracts_sections_and_install() {
        let llm = "<think>x</think>[RESUMO_SIMPLES]\nFaz X.\n[POR_QUE_DESTAQUE]\nRapido.\n[QUANDO_SELECIONAR]\n- A\n- B.\n[VEREDITO]\nUse.";
        let raw = llm.rsplit("</think>").next().unwrap();
        assert_eq!(section(raw, "RESUMO_SIMPLES"), "Faz X.");
        assert_eq!(bullets(&section(raw, "QUANDO_SELECIONAR")), vec!["A", "B"]);
        assert_eq!(section(raw, "QUANDO_EVITAR"), "");
        let readme = "# X\n```bash\n$ pip install foo-bar\n```\n";
        assert_eq!(install_cmd(readme, "Python"), "pip install foo-bar");
        assert_eq!(fmt_int(1234567), "1.234.567");
        assert_eq!(tidy("Ola — mundo 🚀 [link](http://x)"), "Ola , mundo link");
    }
}
