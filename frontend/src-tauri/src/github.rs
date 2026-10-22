use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};

use crate::db::Repo;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";

const LANGUAGES: &[&str] = &[
    "python",
    "typescript",
    "rust",
    "go",
    "javascript",
    "cpp",
    "c",
    "csharp",
    "java",
    "swift",
    "kotlin",
    "zig",
];

// Ordem importa: a primeira categoria que casar vence.
const CATEGORIES: &[(&str, &[&str])] = &[
    (
        "ia_agentes",
        &[
            "agent",
            "agents",
            "llm",
            "llms",
            "gpt",
            "claude",
            "gemini",
            "deepseek",
            "rag",
            "model",
            "models",
            "embedding",
            "transformer",
            "diffusion",
            "lora",
            "ollama",
            "vllm",
            "langchain",
            "llamaindex",
            "prompt",
            "reasoning",
            "fine-tune",
            "multimodal",
            "speech",
            "voice",
            "text-to-speech",
            "asr",
            "ai",
            "mcp",
        ],
    ),
    (
        "frontend_ui",
        &[
            "react",
            "vue",
            "svelte",
            "nextjs",
            "next.js",
            "tailwind",
            "css",
            "frontend",
            "ui",
            "component",
            "components",
            "design system",
            "animation",
            "canvas",
            "webgl",
            "diagram",
            "editor",
            "theme",
            "html",
        ],
    ),
    (
        "backend_api",
        &[
            "api",
            "fastapi",
            "express",
            "django",
            "flask",
            "grpc",
            "rest",
            "database",
            "sql",
            "postgres",
            "sqlite",
            "redis",
            "microservice",
            "serverless",
            "orm",
            "backend",
            "websocket",
        ],
    ),
    (
        "devops_cli",
        &[
            "cli",
            "terminal",
            "docker",
            "kubernetes",
            "ci/cd",
            "devops",
            "monitoring",
            "linux",
            "bash",
            "shell",
            "tui",
            "sysadmin",
            "deploy",
            "proxy",
            "gateway",
            "infrastructure",
        ],
    ),
    (
        "security",
        &[
            "security",
            "reverse-engineering",
            "reverse engineering",
            "vulnerability",
            "cve",
            "exploit",
            "pentest",
            "malware",
            "privacy",
            "firewall",
            "decompiler",
        ],
    ),
    (
        "data_eng",
        &[
            "etl",
            "analytics",
            "spark",
            "parquet",
            "dataframe",
            "pandas",
            "polars",
            "pipeline",
            "duckdb",
            "warehouse",
            "scraper",
            "scraping",
            "crawler",
        ],
    ),
    (
        "mobile",
        &["ios", "android", "flutter", "react-native", "mobile"],
    ),
];

static CATEGORY_RES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    CATEGORIES
        .iter()
        .map(|(cat, kws)| {
            let alt = kws
                .iter()
                .map(|k| regex::escape(k))
                .collect::<Vec<_>>()
                .join("|");
            (
                *cat,
                Regex::new(&format!(r"(?i)(^|[^a-z0-9])(?:{alt})($|[^a-z0-9])")).unwrap(),
            )
        })
        .collect()
});

/// Classifica por palavra inteira (o Python antigo casava substring: "ui" em "build", "api" em "rapid").
pub fn detect_category(name: &str, desc: &str, lang: &str) -> String {
    let text = format!("{name} {desc} {lang}");
    CATEGORY_RES
        .iter()
        .find(|(_, re)| re.is_match(&text))
        .map(|(c, _)| c.to_string())
        .unwrap_or_else(|| "geral".into())
}

fn parse_number(s: &str) -> i64 {
    s.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

pub fn client() -> Client {
    Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(15))
        .build()
        .expect("http client")
}

pub fn parse_trending(html: &str) -> Vec<Repo> {
    let doc = Html::parse_document(html);
    let sel = |s: &str| Selector::parse(s).unwrap();
    let (row_s, link_s, p_s, lang_s, today_s, muted_s) = (
        sel("article.Box-row"),
        sel("h2 a"),
        sel("p"),
        sel("span[itemprop=programmingLanguage]"),
        sel("span.d-inline-block.float-sm-right"),
        sel("a.Link--muted"),
    );
    let text = |e: scraper::ElementRef| {
        e.text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };

    doc.select(&row_s)
        .enumerate()
        .filter_map(|(i, row)| {
            let href = row
                .select(&link_s)
                .next()?
                .value()
                .attr("href")?
                .trim_matches('/')
                .to_string();
            let (owner, name) = href.split_once('/')?;
            let description = row.select(&p_s).next().map(text).unwrap_or_default();
            let language = row
                .select(&lang_s)
                .next()
                .map(text)
                .unwrap_or_else(|| "Unknown".into());
            let muted: Vec<_> = row.select(&muted_s).map(text).collect();
            let repo_id = format!("{owner}/{name}");
            Some(Repo {
                category: detect_category(&repo_id, &description, &language),
                url: format!("https://github.com/{repo_id}"),
                stars_today: row
                    .select(&today_s)
                    .next()
                    .map(|e| parse_number(&text(e)))
                    .unwrap_or(0),
                stars: muted.first().map(|s| parse_number(s)).unwrap_or(0),
                forks: muted.get(1).map(|s| parse_number(s)).unwrap_or(0),
                owner: owner.into(),
                name: name.into(),
                repo_id,
                description,
                language,
                rank: i as i64 + 1,
            })
        })
        .collect()
}

fn fetch_trending(http: &Client, lang: &str) -> Vec<Repo> {
    let url = if lang.is_empty() {
        "https://github.com/trending".into()
    } else {
        format!("https://github.com/trending/{lang}")
    };
    match http
        .get(&url)
        .query(&[("since", "daily")])
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
    {
        Ok(html) => parse_trending(&html),
        Err(_) => vec![],
    }
}

/// Repos criados nos ultimos 7 dias com mais estrelas: novidade de verdade, sem "estrelas hoje" (API nao informa).
fn fetch_new_repos(http: &Client) -> Vec<Repo> {
    let since = (chrono::Local::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    search_repos(
        http,
        &format!("created:>{since} stars:>100"),
        "stars",
        1,
        30,
    )
    .map(|(r, _)| r)
    .unwrap_or_default()
}

/// GET na API do GitHub com o token das configuracoes (se houver). Token so vai para api.github.com.
pub fn api_get(http: &Client, url: &str) -> reqwest::blocking::RequestBuilder {
    let mut req = http
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if url.starts_with("https://api.github.com/") {
        req = req.header("User-Agent", "GitTrendRadar/0.1.2 (Windows; x64)");
        let token = crate::settings::get().github_token;
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
    }
    req
}

/// Busca em TODOS os repositorios publicos do GitHub. A API limita a 1000 resultados por busca.
/// `sort`: "stars" | "updated" | "best" (relevancia).
pub fn search_repos(
    http: &Client,
    query: &str,
    sort: &str,
    page: u32,
    per_page: u32,
) -> Result<(Vec<Repo>, i64), String> {
    let per_page = per_page.clamp(1, 100);
    let max_page = 1000 / per_page;
    if page == 0 || page > max_page {
        return Err("Página além dos 1.000 resultados disponíveis por busca no GitHub.".into());
    }
    let mut params = vec![
        ("q", query.to_string()),
        ("per_page", per_page.to_string()),
        ("page", page.to_string()),
    ];
    if sort != "best" {
        params.push(("sort", sort.to_string()));
        params.push(("order", "desc".into()));
    }
    let resp = api_get(http, "https://api.github.com/search/repositories")
        .query(&params)
        .send()
        .map_err(|e| format!("Sem conexão com o GitHub: {e}"))?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let json: serde_json::Value = resp.json().unwrap_or_default();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        let reset_info = if let Some(reset_val) = headers.get("x-ratelimit-reset") {
            if let Ok(reset_str) = reset_val.to_str() {
                if let Ok(reset_ts) = reset_str.parse::<i64>() {
                    let now = chrono::Utc::now().timestamp();
                    let diff = (reset_ts - now).max(0);
                    if diff > 0 {
                        format!(" Limite da API reseta em {diff}s.")
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        return Err(format!(
            "Limite de buscas do GitHub atingido.{reset_info} Um token nas Configurações pode ampliar a cota; buscas têm limites próprios."
        ));
    }
    if status.as_u16() == 401 {
        return Err("Token do GitHub inválido ou expirado. Atualize nas Configurações.".into());
    }
    if !status.is_success() {
        return Err(format!(
            "GitHub respondeu {status}: {}",
            json["message"].as_str().unwrap_or("")
        ));
    }
    let total = json["total_count"].as_i64().unwrap_or(0);
    let repos = json["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|it| {
            let repo_id = it["full_name"].as_str()?.to_string();
            let (owner, name) = repo_id.split_once('/')?;
            let description = it["description"].as_str().unwrap_or_default().to_string();
            let language = it["language"].as_str().unwrap_or("Unknown").to_string();
            Some(Repo {
                category: detect_category(&repo_id, &description, &language),
                owner: owner.into(),
                name: name.into(),
                url: format!("https://github.com/{repo_id}"),
                stars: it["stargazers_count"].as_i64().unwrap_or(0),
                forks: it["forks_count"].as_i64().unwrap_or(0),
                stars_today: 0,
                rank: 0,
                repo_id,
                description,
                language,
            })
        })
        .collect();
    Ok((repos, total))
}

/// Trending geral + por linguagem + repos novos da semana, sem duplicados.
pub fn collect_all(http: &Client) -> Vec<Repo> {
    let mut by_id: HashMap<String, Repo> = HashMap::new();
    let sources = std::iter::once("").chain(LANGUAGES.iter().copied());
    for r in sources
        .flat_map(|l| fetch_trending(http, l))
        .chain(fetch_new_repos(http))
    {
        by_id.entry(r.repo_id.clone()).or_insert(r);
    }
    let mut all: Vec<Repo> = by_id.into_values().collect();
    all.sort_by(|a, b| (b.stars_today, b.stars).cmp(&(a.stars_today, a.stars)));
    for (i, r) in all.iter_mut().enumerate() {
        r.rank = i as i64 + 1;
    }
    all
}

/// README via raw.githubusercontent (HEAD = branch padrao), sem gastar cota da API.
pub fn fetch_readme(http: &Client, owner: &str, name: &str) -> String {
    ["README.md", "readme.md", "Readme.md"]
        .iter()
        .find_map(|f| {
            let url = format!("https://raw.githubusercontent.com/{owner}/{name}/HEAD/{f}");
            let body = http
                .get(url)
                .send()
                .ok()
                .filter(|r| r.status().is_success())?
                .text()
                .ok()?;
            (body.trim().len() > 50).then_some(body)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_uses_whole_words() {
        assert_eq!(
            detect_category("x/build-tool", "fast rapid builder", "Go"),
            "geral"
        );
        assert_eq!(
            detect_category("x/y", "An AI agent framework", "Python"),
            "ia_agentes"
        );
        assert_eq!(
            detect_category("x/y", "Kubernetes deploy helper", "Rust"),
            "devops_cli"
        );
    }

    #[test]
    fn parses_trending_row() {
        let html = r#"<article class="Box-row"><h2><a href="/acme/rocket">acme / rocket</a></h2>
            <p> Fast CLI tool </p><span itemprop="programmingLanguage">Rust</span>
            <a class="Link--muted">1,234</a><a class="Link--muted">56</a>
            <span class="d-inline-block float-sm-right">789 stars today</span></article>"#;
        let r = &parse_trending(html)[0];
        assert_eq!(
            (r.repo_id.as_str(), r.stars, r.forks, r.stars_today),
            ("acme/rocket", 1234, 56, 789)
        );
        assert_eq!(
            (r.language.as_str(), r.category.as_str()),
            ("Rust", "devops_cli")
        );
    }
}
