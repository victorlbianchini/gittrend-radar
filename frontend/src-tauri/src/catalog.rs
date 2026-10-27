//! Unified, paginated discovery. Source creation dates and local discovery dates
//! are deliberately separate: discovering a skill does not establish its release.
use crate::{
    datasets,
    db::{self, TrendItem},
    github,
    models::{self, HfModel},
    skills::{self, Skill},
    summaries::{SummaryInput, SummaryRecord, SummaryService},
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

const PAGE_SIZE: usize = 50;
fn default_domain() -> String {
    "github".into()
}
fn default_mode() -> String {
    "tops".into()
}
fn default_page() -> u32 {
    1
}
#[derive(Deserialize, Debug)]
pub struct CatalogRequest {
    #[serde(default = "default_domain")]
    pub domain: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}
#[derive(Serialize)]
pub struct CatalogCategory {
    pub id: String,
    pub label: String,
}
#[derive(Serialize)]
pub struct CatalogPage {
    pub items: Vec<CatalogItem>,
    pub page: u32,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub total: Option<i64>,
    pub notice: String,
    pub categories: Vec<CatalogCategory>,
}
#[derive(Serialize)]
pub struct CatalogItem {
    pub key: String,
    pub id: String,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub description: String,
    /// Descricao traduzida para PT-BR (vazia se a traducao falhou; a tela entao mostra o original rotulado).
    pub description_pt: String,
    pub category: String,
    pub source: String,
    pub first_seen: String,
    pub published_at: String,
    pub repo: Option<TrendItem>,
    pub model: Option<HfModel>,
    pub skill: Option<Skill>,
    pub summary: SummaryRecord,
}

impl CatalogRequest {
    fn validate(&mut self) -> Result<(), String> {
        if !["github", "models", "skills", "mcp", "datasets"].contains(&self.domain.as_str()) {
            return Err("Categoria de catálogo inválida.".into());
        }
        if !["tops", "news", "breakout", "explore"].contains(&self.mode.as_str()) {
            return Err("Modo de catálogo inválido.".into());
        }
        if self.source.is_empty() {
            self.source = match self.domain.as_str() {
                "models" | "datasets" => "hf",
                "skills" => "all",
                _ => "github",
            }
            .into();
        }
        let valid = match self.domain.as_str() {
            "github" | "mcp" => self.source == "github",
            "models" | "datasets" => ["hf", "github"].contains(&self.source.as_str()),
            _ => ["all", "github", "skills.sh"].contains(&self.source.as_str()),
        };
        if !valid {
            return Err("Fonte de catálogo inválida.".into());
        }
        if (self.domain == "models" || self.domain == "datasets")
            && self.source == "hf"
            && self.page > 1
            && self.cursor.as_deref().is_none_or(str::is_empty)
        {
            return Err("Continue os modelos ou datasets usando o cursor da página anterior.".into());
        }
        if self.page == 0
            || self.page > 10000
            || self.query.len() > 500
            || self.cursor.as_ref().is_some_and(|s| s.len() > 8192)
            || self.category.as_ref().is_some_and(|s| s.len() > 100)
        {
            return Err("Parâmetros de paginação ou busca inválidos.".into());
        }
        if (self.domain == "github" && (self.mode == "explore" || self.mode == "breakout")
            || self.domain == "mcp"
            || (self.domain == "models" || self.domain == "datasets") && self.source == "github")
            && self.page > 20
        {
            return Err("O GitHub permite até 1.000 resultados por busca (20 páginas de 50). Refine a busca.".into());
        }
        self.query = self.query.trim().into();
        self.category = self.category.take().filter(|s| !s.is_empty() && s != "all");
        Ok(())
    }
}

fn observation_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS catalog_observations (kind TEXT NOT NULL, id TEXT NOT NULL, first_seen TEXT NOT NULL, last_seen TEXT NOT NULL, PRIMARY KEY(kind,id));").map_err(|e| e.to_string())
}
fn observe(conn: &Connection, kind: &str, id: &str, today: &str) -> Result<String, String> {
    conn.execute("INSERT INTO catalog_observations(kind,id,first_seen,last_seen) VALUES (?1,?2,?3,?3) ON CONFLICT(kind,id) DO UPDATE SET last_seen=excluded.last_seen", params![kind,id,today]).map_err(|e|e.to_string())?;
    conn.query_row(
        "SELECT first_seen FROM catalog_observations WHERE kind=?1 AND id=?2",
        params![kind, id],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}
fn categories(values: impl Iterator<Item = String>) -> Vec<CatalogCategory> {
    values
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| CatalogCategory {
            label: id.replace(['_', '-'], " "),
            id,
        })
        .collect()
}
fn matches_query(query: &str, text: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
}
fn discovered_today(first_seen: &str, today: &str) -> bool {
    !first_seen.is_empty() && first_seen == today
}
fn slice<T>(rows: Vec<T>, page: u32) -> (Vec<T>, bool, i64) {
    let total = rows.len();
    let start = (page as usize - 1) * PAGE_SIZE;
    (
        rows.into_iter().skip(start).take(PAGE_SIZE).collect(),
        total > start + PAGE_SIZE,
        total as i64,
    )
}
type SkillsCache = HashMap<String, (Instant, Vec<Skill>)>;
static SKILLS_CACHE: LazyLock<Mutex<SkillsCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));
fn skill_pool(query: &str) -> Result<Vec<Skill>, String> {
    if let Some(rows) = SKILLS_CACHE
        .lock()
        .map_err(|_| "Cache indisponível")?
        .get(query)
        .filter(|(at, _)| at.elapsed() < Duration::from_secs(300))
        .map(|(_, rows)| rows.clone())
    {
        return Ok(rows);
    }
    let http = github::client();
    let rows = if query.is_empty() {
        skills::popular(&http)?
    } else {
        skills::search(&http, query)?
    };
    let mut cache = SKILLS_CACHE.lock().map_err(|_| "Cache indisponível")?;
    if cache.len() >= 32 {
        cache.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(300));
        if cache.len() >= 32 {
            cache.clear();
        }
    }
    cache.insert(query.into(), (Instant::now(), rows.clone()));
    Ok(rows)
}

pub fn load(
    db_path: &Path,
    summaries: &SummaryService,
    mut request: CatalogRequest,
) -> Result<CatalogPage, String> {
    request.validate()?;
    let conn = db::open(db_path).map_err(|e| e.to_string())?;
    observation_schema(&conn)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut page = if request.domain == "skills" {
        load_skills(&conn, summaries, &request, &today)?
    } else if request.domain == "models" && request.source == "hf" {
        load_models(&conn, summaries, &request, &today)?
    } else if request.domain == "datasets" && request.source == "hf" {
        load_datasets(&conn, summaries, &request, &today)?
    } else if request.domain == "mcp" {
        load_mcp(&conn, summaries, &request, &today)?
    } else {
        load_github(&conn, summaries, &request, &today)?
    };
    translate_page(db_path, &mut page);
    Ok(page)
}

/// Traduz as descricoes da pagina em paralelo (cache no SQLite: so a primeira vez vai a rede).
fn translate_page(db_path: &Path, page: &mut CatalogPage) {
    // Testes unitarios ficam offline; a traducao real e coberta pelo teste live ignorado.
    if cfg!(test) {
        return;
    }
    let http = crate::github::client();
    let size = page.items.len().div_ceil(8).max(1);
    std::thread::scope(|s| {
        for chunk in page.items.chunks_mut(size) {
            let http = &http;
            s.spawn(move || {
                let Ok(conn) = db::open(db_path) else { return };
                for item in chunk {
                    if let Some(pt) = crate::analyze::try_translate(http, &conn, &item.description) {
                        item.description_pt = pt;
                    }
                }
            });
        }
    });
}

fn load_github(
    conn: &Connection,
    summaries: &SummaryService,
    r: &CatalogRequest,
    today: &str,
) -> Result<CatalogPage, String> {
    let remote = r.domain == "models"
        || r.domain == "datasets"
        || r.mode == "explore"
        || r.mode == "breakout";
    let (mut repos, has_more, total, notice, cats) = if r.mode == "breakout" {
        let base_q = if r.domain == "models" {
            if r.query.is_empty() {
                "large-language-model OR diffusion-model OR text-to-image in:name,description"
                    .to_string()
            } else {
                format!("{} model in:name,description", r.query)
            }
        } else if r.domain == "datasets" {
            if r.query.is_empty() {
                "topic:dataset OR dataset in:name,description".to_string()
            } else {
                format!("{} dataset in:name,description", r.query)
            }
        } else {
            r.query.clone()
        };
        let min_stars = if r.domain == "github" && r.query.is_empty() {
            25
        } else {
            5
        };
        let days = if r.domain == "github" { 7 } else { 14 };
        let since = (chrono::Local::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string();
        let (repos, total) = github::fetch_breakout(
            &github::client(),
            &base_q,
            days,
            min_stars,
            r.page,
            PAGE_SIZE as u32,
        )?;
        db::save_repos_period(conn, &repos, today, "search").map_err(|e| e.to_string())?;
        let cats = categories(repos.iter().map(|v| v.category.clone()));
        let rows = repos
            .into_iter()
            .map(|repo| {
                let a = db::get_analysis(conn, &repo.repo_id);
                (repo, String::new(), a)
            })
            .collect();
        let items = crate::to_items(conn, rows);
        let notice = format!(
            "Foguetes dos últimos {days} dias no GitHub (created:>={since}), ordenados por velocidade média (est./dia) a partir de stargazers_count e created_at oficiais da API."
        );
        let more = (r.page as i64) * (PAGE_SIZE as i64) < total.min(1000) && !items.is_empty();
        (items, more, Some(total), notice, cats)
    } else if remote {
        let mut q = if r.domain == "models" {
            if r.query.is_empty() {
                "large-language-model OR diffusion-model OR text-to-image in:name,description"
                    .into()
            } else {
                format!("{} model in:name,description", r.query)
            }
        } else if r.domain == "datasets" {
            if r.query.is_empty() {
                "topic:dataset OR dataset in:name,description".into()
            } else {
                format!("{} dataset in:name,description", r.query)
            }
        } else if r.query.is_empty() {
            "stars:>0".into()
        } else {
            r.query.clone()
        };
        if r.mode == "news" {
            q.push_str(&format!(" created:>={today}"));
        }
        let sort = if r.mode == "news" {
            "updated"
        } else if r.mode == "tops" || r.query.is_empty() {
            "stars"
        } else {
            "best"
        };
        let (repos, total) =
            github::search_repos(&github::client(), &q, sort, r.page, PAGE_SIZE as u32)?;
        db::save_repos_period(conn, &repos, today, "search").map_err(|e| e.to_string())?;
        let cats = categories(repos.iter().map(|v| v.category.clone()));
        let rows = repos
            .into_iter()
            .map(|repo| {
                let a = db::get_analysis(conn, &repo.repo_id);
                (repo, String::new(), a)
            })
            .collect();
        let items = crate::to_items(conn, rows);
        let mut notice = "Busca pública do GitHub; até 1.000 resultados por consulta. Categorias filtram a página atual.".to_string();
        if r.domain == "models" {
            if r.query.is_empty() {
                notice.push_str(" Projetos encontrados pelos termos large-language-model, diffusion-model ou text-to-image.");
            } else {
                notice
                    .push_str(" A busca digitada combina seu termo com 'model' no nome/descrição.");
            }
            notice
                .push_str(" Inclui ferramentas e cursos, sem comprovar disponibilidade de pesos.");
        } else if r.domain == "datasets" {
            notice.push_str(" Repositórios de datasets abertos no GitHub.");
        }
        if r.mode == "news" {
            notice.push_str(&format!(" Consulta por created:>={today}; a data individual não está disponível nesta listagem."));
        }
        let more = (r.page as i64) * (PAGE_SIZE as i64) < total.min(1000) && !items.is_empty();
        (items, more, Some(total), notice, cats)
    } else {
        let mut rows = db::trends(conn).map_err(|e| e.to_string())?;
        rows.retain(|(repo, first, _)| {
            (r.mode != "news" || discovered_today(first, today))
                && matches_query(&r.query, &format!("{} {}", repo.repo_id, repo.description))
        });
        let cats = categories(rows.iter().map(|(v, _, _)| v.category.clone()));
        rows.retain(|(v, _, _)| r.category.as_ref().is_none_or(|c| c == &v.category));
        let (rows, more, total) = slice(rows, r.page);
        let notice=if r.mode=="news" {"Descobertos hoje no Trending; a primeira aparição não representa lançamento."}else{"Última coleta disponível do Trending, ordenada por estrelas do período e estrelas totais."}.into();
        (crate::to_items(conn, rows), more, Some(total), notice, cats)
    };
    if remote {
        repos.retain(|v| r.category.as_ref().is_none_or(|c| c == &v.repo.category));
    }
    let mut items = Vec::new();
    for mut repo in repos {
        repo.is_new = discovered_today(&repo.first_seen, today);
        let id = repo.repo.repo_id.clone();
        let key = format!("github:{id}");
        let summary = summaries.enqueue(SummaryInput {
            key: key.clone(),
            kind: "github".into(),
            id: id.clone(),
            title: id.clone(),
            description: repo.repo.description.clone(),
            repo: id.clone(),
            branch: String::new(),
            path: String::new(),
        });
        items.push(CatalogItem {
            key,
            id: id.clone(),
            kind: "github".into(),
            title: id,
            url: repo.repo.url.clone(),
            description: repo.repo.description.clone(),
            description_pt: String::new(),
            category: repo.repo.category.clone(),
            source: "github".into(),
            first_seen: repo.first_seen.clone(),
            published_at: repo.repo.repo_created_at.clone(),
            repo: Some(repo),
            model: None,
            skill: None,
            summary,
        });
    }
    Ok(CatalogPage {
        items,
        page: r.page,
        has_more,
        next_cursor: None,
        total,
        notice,
        categories: cats,
    })
}

fn load_models(
    conn: &Connection,
    summaries: &SummaryService,
    r: &CatalogRequest,
    today: &str,
) -> Result<CatalogPage, String> {
    let page = models::search(
        &github::client(),
        &r.query,
        if r.mode == "news" {
            "createdAt"
        } else {
            "trendingScore"
        },
        r.cursor.as_deref(),
    )?;
    let cats = categories(page.models.iter().map(|m| m.group.clone()));
    // Descending creation order means an older date marks the end of today's entries.
    let older = page
        .models
        .iter()
        .any(|m| !m.created_at.is_empty() && m.created_at.as_str() < today);
    let has_more = page.next_cursor.is_some() && (r.mode != "news" || !older);
    let mut items = Vec::new();
    for mut model in page.models {
        if r.mode == "news" && model.created_at != today {
            continue;
        }
        if r.mode == "breakout" && !model.is_new {
            continue;
        }
        if r.category.as_ref().is_some_and(|c| c != &model.group) {
            continue;
        }
        let published_today = model.created_at == today;
        if r.mode != "breakout" {
            model.is_new = published_today;
        }
        let id = model.id.clone();
        let key = format!("hf:{id}");
        let first = observe(conn, "hf", &id, today)?;
        let description = format!("{}; tarefa: {}", model.name, model.task_label);
        let summary = summaries.enqueue(SummaryInput {
            key: key.clone(),
            kind: "hf".into(),
            id: id.clone(),
            title: id.clone(),
            description: description.clone(),
            repo: String::new(),
            branch: String::new(),
            path: String::new(),
        });
        items.push(CatalogItem {
            key,
            id: id.clone(),
            kind: "hf".into(),
            title: id,
            url: model.url.clone(),
            description,
            description_pt: String::new(),
            category: model.group.clone(),
            source: "hf".into(),
            first_seen: first,
            published_at: model.created_at.clone(),
            repo: None,
            model: Some(model),
            skill: None,
            summary,
        });
    }
    Ok(CatalogPage{items,page:r.page,has_more,next_cursor:if has_more{page.next_cursor}else{None},total:None,notice:if r.mode=="news"{"Modelos criados hoje, conforme createdAt do Hugging Face. Categorias filtram a página atual."}else if r.mode=="breakout"{"Modelos lançados nos últimos 14 dias em maior tendência (trendingScore) no Hugging Face. Categorias filtram a página atual."}else{"Modelos públicos do Hugging Face por tendência. Categorias filtram a página atual."}.into(),categories:cats})
}

fn load_datasets(
    conn: &Connection,
    summaries: &SummaryService,
    r: &CatalogRequest,
    today: &str,
) -> Result<CatalogPage, String> {
    let page = datasets::search(
        &github::client(),
        &r.query,
        if r.mode == "news" {
            "createdAt"
        } else {
            "trendingScore"
        },
        r.cursor.as_deref(),
    )?;
    let cats = categories(page.models.iter().map(|m| m.group.clone()));
    let older = page
        .models
        .iter()
        .any(|m| !m.created_at.is_empty() && m.created_at.as_str() < today);
    let has_more = page.next_cursor.is_some() && (r.mode != "news" || !older);
    let mut items = Vec::new();
    for mut model in page.models {
        if r.mode == "news" && model.created_at != today {
            continue;
        }
        if r.mode == "breakout" && !model.is_new {
            continue;
        }
        if r.category.as_ref().is_some_and(|c| c != &model.group) {
            continue;
        }
        if r.mode != "breakout" {
            model.is_new = model.created_at == today;
        }
        let id = model.id.clone();
        let key = format!("dataset:{id}");
        let first = observe(conn, "dataset", &id, today)?;
        let description = model
            .summary
            .clone()
            .unwrap_or_else(|| format!("{}; modalidade: {}", model.name, model.task_label));
        let description_pt = crate::db::cached_translation(conn, &description).unwrap_or_default();
        let summary = summaries.enqueue(SummaryInput {
            key: key.clone(),
            kind: "dataset".into(),
            id: id.clone(),
            title: id.clone(),
            description: description.clone(),
            repo: String::new(),
            branch: String::new(),
            path: String::new(),
        });
        items.push(CatalogItem {
            key,
            id: id.clone(),
            kind: "dataset".into(),
            title: id,
            url: model.url.clone(),
            description,
            description_pt,
            category: model.group.clone(),
            source: "hf".into(),
            first_seen: first,
            published_at: model.created_at.clone(),
            repo: None,
            model: Some(model),
            skill: None,
            summary,
        });
    }
    Ok(CatalogPage {
        items,
        page: r.page,
        has_more,
        next_cursor: if has_more { page.next_cursor } else { None },
        total: None,
        notice: if r.mode == "news" {
            "Datasets criados hoje no Hugging Face. Categorias filtram a página atual.".into()
        } else if r.mode == "breakout" {
            "Datasets lançados nos últimos 14 dias em maior tendência (trendingScore) no Hugging Face. Categorias filtram a página atual.".into()
        } else {
            "Datasets públicos do Hugging Face por tendência. Categorias filtram a página atual.".into()
        },
        categories: cats,
    })
}

fn load_mcp(
    conn: &Connection,
    summaries: &SummaryService,
    r: &CatalogRequest,
    today: &str,
) -> Result<CatalogPage, String> {
    let base_q = if r.query.is_empty() {
        "mcp server in:name,description".to_string()
    } else {
        format!("{} mcp server in:name,description", r.query)
    };
    let (repos, total, notice) = if r.mode == "breakout" {
        let since = (chrono::Local::now() - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        let (repos, total) =
            github::fetch_breakout(&github::client(), &base_q, 30, 2, r.page, PAGE_SIZE as u32)?;
        (
            repos,
            total,
            format!(
                "Servidores MCP criados nos últimos 30 dias (created:>={since}), ordenados por velocidade média (est./dia)."
            ),
        )
    } else {
        let mut q = base_q;
        if r.mode == "news" {
            q.push_str(&format!(" created:>={today}"));
        }
        let sort = if r.mode == "news" {
            "updated"
        } else if r.mode == "tops" || r.query.is_empty() {
            "stars"
        } else {
            "best"
        };
        let (repos, total) =
            github::search_repos(&github::client(), &q, sort, r.page, PAGE_SIZE as u32)?;
        (
            repos,
            total,
            "Servidores MCP (Model Context Protocol) públicos no GitHub para Claude Desktop, ALFRED, Cursor e agentes de IA. Categorias filtram a página atual.".to_string(),
        )
    };
    db::save_repos_period(conn, &repos, today, "search").map_err(|e| e.to_string())?;
    let cats = categories(repos.iter().map(|v| v.category.clone()));
    let rows = repos
        .into_iter()
        .map(|repo| {
            let a = db::get_analysis(conn, &repo.repo_id);
            (repo, String::new(), a)
        })
        .collect();
    let mut items_raw = crate::to_items(conn, rows);
    if let Some(ref cat) = r.category {
        items_raw.retain(|v| &v.repo.category == cat);
    }
    let mut items = Vec::new();
    for mut repo in items_raw {
        repo.is_new = discovered_today(&repo.first_seen, today);
        let id = repo.repo.repo_id.clone();
        let key = format!("mcp:{id}");
        let summary = summaries.enqueue(SummaryInput {
            key: key.clone(),
            kind: "github".into(),
            id: id.clone(),
            title: id.clone(),
            description: repo.repo.description.clone(),
            repo: id.clone(),
            branch: String::new(),
            path: String::new(),
        });
        items.push(CatalogItem {
            key,
            id: id.clone(),
            kind: "mcp".into(),
            title: id,
            url: repo.repo.url.clone(),
            description: repo.repo.description.clone(),
            description_pt: String::new(),
            category: "Servidor MCP".into(),
            source: "github".into(),
            first_seen: repo.first_seen.clone(),
            published_at: repo.repo.repo_created_at.clone(),
            repo: Some(repo),
            model: None,
            skill: None,
            summary,
        });
    }
    let more = (r.page as i64) * (PAGE_SIZE as i64) < total.min(1000) && !items.is_empty();
    Ok(CatalogPage {
        items,
        page: r.page,
        has_more: more,
        next_cursor: None,
        total: Some(total),
        notice,
        categories: cats,
    })
}

fn load_skills(
    conn: &Connection,
    summaries: &SummaryService,
    r: &CatalogRequest,
    today: &str,
) -> Result<CatalogPage, String> {
    let since_7d = (chrono::Local::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let mut pool = skill_pool(&r.query)?;
    pool.retain(|s| {
        r.source == "all"
            || s.sources
                .iter()
                .any(|src| src.eq_ignore_ascii_case(&r.source))
    });
    let mut rows = Vec::new();
    for s in pool {
        let first = observe(conn, "skill", &s.id, today)?;
        let category = github::detect_category(&s.name, &s.description, "");
        let keep = if r.mode == "news" {
            discovered_today(&first, today)
        } else if r.mode == "breakout" {
            !first.is_empty() && first.as_str() >= since_7d.as_str()
        } else {
            true
        };
        if keep {
            rows.push((s, first, category));
        }
    }
    rows.sort_by(|a, b| {
        (b.0.installs, b.0.stars)
            .cmp(&(a.0.installs, a.0.stars))
            .then(a.0.id.cmp(&b.0.id))
    });
    let cats = categories(rows.iter().map(|(_, _, c)| c.clone()));
    rows.retain(|(_, _, c)| r.category.as_ref().is_none_or(|selected| selected == c));
    let (rows, has_more, total) = slice(rows, r.page);
    let mut items = Vec::new();
    for (skill, first, category) in rows {
        let key = format!("skill:{}", skill.id);
        let summary = summaries.enqueue(SummaryInput {
            key: key.clone(),
            kind: "skill".into(),
            id: skill.id.clone(),
            title: skill.name.clone(),
            description: skill.description.clone(),
            repo: skill.repo.clone(),
            branch: skill.branch.clone(),
            path: skill.path.clone(),
        });
        items.push(CatalogItem {
            key,
            id: skill.id.clone(),
            kind: "skill".into(),
            title: skill.name.clone(),
            url: skill.github_url.clone(),
            description: skill.description.clone(),
            description_pt: String::new(),
            category,
            source: skill.sources.join(", "),
            first_seen: first,
            published_at: String::new(),
            repo: None,
            model: None,
            skill: Some(skill),
            summary,
        });
    }
    Ok(CatalogPage{items,page:r.page,has_more,next_cursor:None,total:Some(total),notice:if r.mode=="news"{"Skills descobertas hoje nesta instalação; a carga inicial também conta como descoberta, não como lançamento. As fontes não fornecem data de criação."}else if r.mode=="breakout"{"Skills descobertas nos últimos 7 dias nesta instalação, ordenadas por instalações e estrelas. As fontes não fornecem data de criação."}else{"Catálogo disponível em skills.sh e GitHub Anthropic/OpenAI, ordenado por instalações e estrelas informadas. A busca não cobre todas as skills da internet."}.into(),categories:cats})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> CatalogRequest {
        serde_json::from_str("{}").unwrap()
    }
    #[test]
    fn validates_before_network() {
        let mut r = request();
        assert!(r.validate().is_ok());
        assert_eq!(r.source, "github");
        r.page = 0;
        assert!(r.validate().is_err());
        r.page = 21;
        r.mode = "explore".into();
        assert!(r.validate().is_err());
        r.mode = "breakout".into();
        assert!(r.validate().is_err());
        r.page = 1;
        assert!(r.validate().is_ok());
        r.source = "unknown".into();
        assert!(r.validate().is_err());
    }
    #[test]
    fn pagination_has_no_duplicates() {
        let (a, more, total) = slice((0..101).collect(), 1);
        assert_eq!(a.len(), 50);
        assert!(more);
        assert_eq!(total, 101);
        let (b, more, _) = slice((0..101).collect(), 3);
        assert_eq!(b, vec![100]);
        assert!(!more);
    }
    #[test]
    fn stale_latest_collection_is_not_today() {
        assert!(!discovered_today("2026-10-14", "2026-10-15"));
        assert!(!discovered_today("", "2026-10-15"));
        assert!(discovered_today("2026-10-15", "2026-10-15"));
    }
    #[test]
    fn hf_page_requires_cursor() {
        let mut r = request();
        r.domain = "models".into();
        r.page = 2;
        assert!(r.validate().is_err());
        r.cursor = Some("next-page".into());
        assert!(r.validate().is_ok());
    }
    #[test]
    fn observations_preserve_first_day_and_separate_sources() {
        let conn = Connection::open_in_memory().unwrap();
        observation_schema(&conn).unwrap();
        assert_eq!(
            observe(&conn, "skill", "x", "2026-10-14").unwrap(),
            "2026-10-14"
        );
        assert_eq!(
            observe(&conn, "skill", "x", "2026-10-15").unwrap(),
            "2026-10-14"
        );
        assert_eq!(
            observe(&conn, "hf", "x", "2026-10-15").unwrap(),
            "2026-10-15"
        );
    }
    #[test]
    fn categories_deduplicate_and_query_is_case_insensitive() {
        let cats = categories(["texto".into(), "audio".into(), "texto".into()].into_iter());
        assert_eq!(cats.len(), 2);
        assert!(matches_query("RuSt", "Rust library"));
        assert!(!matches_query("python", "Rust library"));
    }

    #[test]
    fn offline_adapter_paginates_queues_and_keeps_daily_discovery_separate() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gtr_catalog_adapter_{}_{}_{}",
            std::process::id(),
            unique,
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("catalog.db");
        let conn = db::open(&path).unwrap();
        let date = chrono::Local::now().date_naive();
        let today = date.format("%Y-%m-%d").to_string();
        let yesterday = date.pred_opt().unwrap().format("%Y-%m-%d").to_string();
        let repos: Vec<_> = (0..51)
            .map(|i| db::Repo {
                repo_id: format!("catalog-fixture/repo{i:03}"),
                owner: "catalog-fixture".into(),
                name: format!("repo{i:03}"),
                description: format!("Offline fixture {i}"),
                url: format!("https://github.com/catalog-fixture/repo{i:03}"),
                language: "Rust".into(),
                stars: 100 - i,
                stars_today: 100 - i,
                category: if i % 2 == 0 { "backend" } else { "frontend" }.into(),
                rank: i + 1,
                ..Default::default()
            })
            .collect();
        db::save_repos(&conn, &repos[..1], &yesterday).unwrap();
        // Seeing a repo via search yesterday must not make today's daily debut old.
        db::save_repos_period(&conn, &repos[1..2], &yesterday, "search").unwrap();
        let service = SummaryService::offline_for_test(path.clone());
        let mut news = request();
        news.mode = "news".into();
        let stale = load(&path, &service, news).unwrap();
        assert!(
            stale.items.is_empty(),
            "A stale latest collection is not news today"
        );
        db::save_repos(&conn, &repos, &today).unwrap();
        let first = load(&path, &service, request()).unwrap();
        assert_eq!(first.items.len(), 50);
        assert!(first.has_more);
        assert_eq!(first.total, Some(51));
        assert!(
            first
                .items
                .iter()
                .all(|item| item.key == format!("github:{}", item.id)
                    && item.summary.key == item.key)
        );
        let state = service.status();
        assert_eq!(
            state.pending + state.running + state.ready + state.errors,
            50,
            "Every displayed item enters the real summary service"
        );
        let mut second_request = request();
        second_request.page = 2;
        let second = load(&path, &service, second_request).unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(!second.has_more);
        assert!(
            !first
                .items
                .iter()
                .any(|item| item.key == second.items[0].key)
        );
        let mut news = request();
        news.mode = "news".into();
        let news = load(&path, &service, news).unwrap();
        assert_eq!(news.total, Some(50));
        assert_eq!(news.items.len(), 50);
        assert!(!news.items.iter().any(|item| item.id == repos[0].repo_id));
        assert!(news.items.iter().any(|item| item.id == repos[1].repo_id
            && item.first_seen == today
            && item.repo.as_ref().unwrap().is_new));
        let mut filtered = request();
        filtered.category = Some("frontend".into());
        let filtered = load(&path, &service, filtered).unwrap();
        assert_eq!(filtered.total, Some(25));
        assert_eq!(filtered.items.len(), 25);
        assert!(
            filtered
                .items
                .iter()
                .all(|item| item.category == "frontend")
        );
        drop(conn);
        // The test motor uses an unreachable loopback port and never fetches source
        // documents. Wait for final SQLite writes before removing only this fixture.
        let started = Instant::now();
        loop {
            let status = service.status();
            if status.pending + status.running == 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "Offline worker did not settle; fixture retained at {}",
                root.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(service);
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
