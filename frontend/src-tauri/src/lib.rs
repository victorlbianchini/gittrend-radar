mod analyze;
mod catalog;
#[cfg(test)]
mod catalog_smoke_tests;
pub mod datasets;
mod db;
mod downloads;
mod github;
mod local;
mod models;
mod settings;
mod mcp;
pub use mcp::run as run_mcp;
mod skills;
mod stack;
mod summaries;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

use db::{Repo, TrendItem};

const SYNC_EVERY: Duration = Duration::from_secs(4 * 3600);
const ANALYSIS_WORKERS: usize = 8;
const PULL_WORKERS: usize = 4;
const PENDING_CHECK_EVERY: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize, Clone, Default)]
struct SyncStatus {
    is_syncing: bool,
    last_sync_time: Option<String>,
    last_sync_date: Option<String>,
    total_repos_today: i64,
    message: String,
    current_step: String,
}

struct App {
    db_path: PathBuf,
    summaries: summaries::SummaryService,
    status: Mutex<SyncStatus>,
    handle: tauri::AppHandle,
    hf_cache: Mutex<Option<(Instant, Vec<models::HfModel>)>>,
    pending_notified: Mutex<i64>,
}

const HF_CACHE_TTL: Duration = Duration::from_secs(3600);

type State<'a> = tauri::State<'a, Arc<App>>;

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn set_step(app: &App, step: String) {
    app.status.lock().unwrap().current_step = step;
}

fn run_sync(app: &App, force: bool) {
    {
        let mut st = app.status.lock().unwrap();
        if st.is_syncing
            || (!force
                && st.last_sync_date.as_deref() == Some(&today())
                && st.total_repos_today > 0)
        {
            return;
        }
        st.is_syncing = true;
        st.current_step = "Buscando repositórios em alta no GitHub...".into();
    }
    let started = Instant::now();
    let date = today();
    let result = sync_inner(app, &date);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let conn = db::open(&app.db_path).ok();
    let mut st = app.status.lock().unwrap();
    st.is_syncing = false;
    match result {
        Ok(n) => {
            st.last_sync_date = Some(date.clone());
            st.last_sync_time = Some(now.clone());
            st.total_repos_today = n as i64;
            st.message = format!("Atualizado às {} ({n} repositórios)", &now[11..16]);
            st.current_step = "Concluído".into();
            if let Some(c) = conn {
                db::log_sync(
                    &c,
                    &date,
                    &now,
                    n as i64,
                    "success",
                    &st.message,
                    started.elapsed().as_secs_f64(),
                );
                // So no sync automatico (abrir o app / a cada 4h); no manual a propria tela ja avisa
                let new_today = build_items(&c).iter().filter(|i| i.is_new).count();
                if !force && new_today > 0 {
                    let _ = app
                        .handle
                        .notification()
                        .builder()
                        .title("GitTrend Radar")
                        .body(format!("{new_today} novidades no GitHub hoje"))
                        .show();
                }
            }
        }
        Err(e) => {
            st.message = format!("Erro na sincronização: {e}");
            st.current_step = "Erro".into();
            if let Some(c) = conn {
                db::log_sync(
                    &c,
                    &date,
                    &now,
                    0,
                    "error",
                    &e,
                    started.elapsed().as_secs_f64(),
                );
            }
        }
    }
}

fn check_pending_requests(app: &App) {
    let Ok(conn) = db::open(&app.db_path) else {
        return;
    };
    let Ok(requests) = db::list_agent_requests(&conn, 100) else {
        return;
    };
    let pending = requests.iter().filter(|r| r.status == "pending").count() as i64;
    if pending == 0 {
        *app.pending_notified.lock().unwrap() = 0;
        return;
    }
    let mut last = app.pending_notified.lock().unwrap();
    if *last == pending {
        return;
    }
    *last = pending;
    drop(last);
    let _ = app
        .handle
        .notification()
        .builder()
        .title("GitTrend Radar")
        .body(format!(
            "{pending} pedido(s) de agente aguardando sua aprovacao no app"
        ))
        .show();
}

fn sync_inner(app: &App, date: &str) -> Result<usize, String> {
    let http = github::client();
    let repos = github::collect_all(&http);
    if repos.is_empty() {
        return Err("GitHub não retornou repositórios (sem internet?).".into());
    }
    let conn = db::open(&app.db_path).map_err(|e| e.to_string())?;
    db::save_repos(&conn, &repos, date).map_err(|e| e.to_string())?;

    let pending: Vec<&Repo> = repos
        .iter()
        .filter(|r| db::get_analysis(&conn, &r.repo_id).is_none())
        .collect();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..ANALYSIS_WORKERS {
            s.spawn(|| {
                let Ok(conn) = db::open(&app.db_path) else {
                    return;
                };
                while let Some(repo) = pending.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let readme = github::fetch_readme(&http, &repo.owner, &repo.name);
                    let a = analyze::heuristic(&http, &conn, repo, &readme);
                    let _ = db::save_analysis(&conn, &repo.repo_id, &a);
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    set_step(
                        app,
                        format!(
                            "Analisando novidades ({n}/{}): {}",
                            pending.len(),
                            repo.repo_id
                        ),
                    );
                }
            });
        }
    });
    Ok(repos.len())
}

fn build_items(conn: &rusqlite::Connection) -> Vec<TrendItem> {
    to_items(conn, db::trends(conn).unwrap_or_default())
}

pub(crate) fn to_items(
    conn: &rusqlite::Connection,
    rows: Vec<(Repo, String, Option<db::Analysis>)>,
) -> Vec<TrendItem> {
    let latest = db::latest_date(conn).unwrap_or_default();
    let dirs = local::existing_dirs();
    let favs = db::favorite_ids(conn);
    rows.into_iter()
        .map(|(mut repo, first_seen, analysis)| {
            if let Some(pt) = analysis
                .as_ref()
                .map(|a| a.description_pt.clone())
                .filter(|s| !s.is_empty())
            {
                repo.description = pt;
            }
            let dl = local::find_downloaded(&dirs, &repo.repo_id, &repo.name).cloned();
            TrendItem {
                is_new: !first_seen.is_empty() && first_seen == latest,
                first_seen,
                has_analysis: analysis.is_some(),
                simple_summary: analysis.as_ref().map(|a| a.simple_summary.clone()),
                why_trending: analysis.as_ref().map(|a| a.why_trending.clone()),
                quick_verdict: analysis.as_ref().map(|a| a.quick_verdict.clone()),
                when_to_select: analysis
                    .as_ref()
                    .map(|a| a.when_to_select.clone())
                    .unwrap_or_default(),
                install_cmd: analysis.as_ref().map(|a| a.install_cmd.clone()),
                analysis,
                is_downloaded: dl.is_some(),
                is_favorite: favs.contains(&repo.repo_id),
                downloaded_path: dl
                    .as_ref()
                    .map(|d| settings::github_dir().join(d).display().to_string()),
                downloaded_folder: dl,
                pretty_name: local::pretty_name(&repo.name),
                repo,
            }
        })
        .collect()
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_favorite(state: State<'_>, repo_id: String) -> Result<bool, String> {
    let path = state.db_path.clone();
    blocking(move || {
        db::open(&path)
            .and_then(|c| db::toggle_favorite(&c, &repo_id))
            .map_err(|e| e.to_string())
    })
    .await?
}

#[derive(Serialize)]
struct Library {
    favorites: Vec<TrendItem>,
    downloaded: Vec<TrendItem>,
    downloaded_models: Vec<models::HfModel>,
    downloaded_datasets: Vec<models::HfModel>,
    downloaded_skills: Vec<skills::Skill>,
    total_downloaded: usize,
}

/// Favoritos e baixados entre todos os repos ja vistos (nao so os do Trending de hoje),
/// alem de modelos do Hugging Face, datasets e skills locais/ecossistema.
#[tauri::command]
async fn get_library(state: State<'_>) -> Result<Library, String> {
    let path = state.db_path.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let all = to_items(&conn, db::all_latest(&conn).map_err(|e| e.to_string())?);
        let favorites = all.iter().filter(|i| i.is_favorite).cloned().collect();
        let downloaded: Vec<TrendItem> = all.into_iter().filter(|i| i.is_downloaded).collect();
        let downloaded_models = models::list_downloaded();
        let downloaded_datasets = datasets::list_downloaded();
        let downloaded_skills = skills::list_downloaded();
        let total_downloaded = downloaded.len()
            + downloaded_models.len()
            + downloaded_datasets.len()
            + downloaded_skills.len();
        Ok(Library {
            favorites,
            downloaded,
            downloaded_models,
            downloaded_datasets,
            downloaded_skills,
            total_downloaded,
        })
    })
    .await?
}

#[tauri::command]
async fn get_hf_models(state: State<'_>, force: bool) -> Result<Vec<models::HfModel>, String> {
    if !force {
        if let Some((at, list)) = state.hf_cache.lock().unwrap().as_ref() {
            if at.elapsed() < HF_CACHE_TTL {
                return Ok(list.clone());
            }
        }
    }
    let list = blocking(|| models::fetch_trending(&github::client())).await??;
    *state.hf_cache.lock().unwrap() = Some((Instant::now(), list.clone()));
    Ok(list)
}

#[tauri::command]
fn get_status(state: State) -> SyncStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
fn start_sync(state: State, force: bool) -> &'static str {
    if state.status.lock().unwrap().is_syncing {
        return "already_syncing";
    }
    let app = state.inner().clone();
    std::thread::spawn(move || run_sync(&app, force));
    "started"
}

#[tauri::command]
async fn get_trends(state: State<'_>) -> Result<Vec<TrendItem>, String> {
    let path = state.db_path.clone();
    blocking(move || {
        db::open(&path)
            .map(|c| build_items(&c))
            .map_err(|e| e.to_string())
    })
    .await?
}

#[derive(Serialize)]
struct CategoryGroup {
    category: &'static str,
    title: &'static str,
    count: usize,
    top_repositories: Vec<TrendItem>,
}

#[tauri::command]
async fn get_categories(state: State<'_>) -> Result<Vec<CategoryGroup>, String> {
    const CATS: &[(&str, &str)] = &[
        ("ia_agentes", "IA e Agentes Autônomos"),
        ("frontend_ui", "Frontend e Design UI"),
        ("backend_api", "Backend e APIs"),
        ("devops_cli", "DevOps, Terminal e CLI"),
        ("security", "Segurança"),
        ("data_eng", "Dados e Engenharia"),
        ("mobile", "Mobile"),
        ("geral", "Geral e Utilidades"),
    ];
    let path = state.db_path.clone();
    blocking(move || {
        let items = db::open(&path)
            .map(|c| build_items(&c))
            .map_err(|e| e.to_string())?;
        Ok(CATS
            .iter()
            .map(|(id, title)| {
                let top: Vec<_> = items
                    .iter()
                    .filter(|i| i.repo.category == *id)
                    .take(4)
                    .cloned()
                    .collect();
                CategoryGroup {
                    category: id,
                    title,
                    count: top.len(),
                    top_repositories: top,
                }
            })
            .filter(|g| g.count > 0)
            .collect())
    })
    .await?
}

#[tauri::command]
async fn clone_repo(
    url: String,
    name: String,
    custom_name: Option<String>,
) -> Result<local::ActionResult, String> {
    blocking(move || local::clone_repo(&url, &name, custom_name.as_deref())).await
}

#[tauri::command]
fn open_folder(folder_path: String) -> local::ActionResult {
    local::open_folder(&folder_path)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    local::open_url(&url)
}

#[derive(Serialize)]
struct AiStatus {
    ollama_online: bool,
    models: Vec<String>,
    default_model: Option<String>,
}

#[tauri::command]
async fn ai_models() -> Result<AiStatus, String> {
    blocking(|| {
        let models = analyze::ollama_models(&github::client());
        AiStatus {
            ollama_online: models.is_some(),
            default_model: models.as_deref().and_then(analyze::best_model),
            models: models.unwrap_or_default(),
        }
    })
    .await
}

#[tauri::command]
async fn analyze_ollama(state: State<'_>, repo_id: String) -> Result<db::Analysis, String> {
    let path = state.db_path.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let item = to_items(&conn, db::all_latest(&conn).map_err(|e| e.to_string())?)
            .into_iter()
            .find(|i| i.repo.repo_id == repo_id)
            .ok_or("Repositório não encontrado")?;
        let http = github::client();
        let models =
            analyze::ollama_models(&http).ok_or("Ollama não está rodando (localhost:11434).")?;
        let model = analyze::best_model(&models).ok_or("Nenhum modelo instalado no Ollama.")?;
        let readme = github::fetch_readme(&http, &item.repo.owner, &item.repo.name);
        let base = item
            .analysis
            .clone()
            .unwrap_or_else(|| analyze::heuristic(&http, &conn, &item.repo, &readme));
        let a = analyze::ollama(&http, &conn, &item.repo, &readme, &model, base)?;
        db::save_analysis(&conn, &repo_id, &a).map_err(|e| e.to_string())?;
        Ok(a)
    })
    .await?
}

#[derive(Serialize)]
struct SearchPage {
    repositories: Vec<TrendItem>,
    total: i64,
    page: u32,
    has_more: bool,
}

#[tauri::command]
async fn search_github(
    state: State<'_>,
    query: String,
    sort: String,
    page: u32,
) -> Result<SearchPage, String> {
    let path = state.db_path.clone();
    blocking(move || {
        if !(1..=10).contains(&page) {
            return Err(
                "A busca permite até 10 páginas de 100 resultados. Refine os filtros.".into(),
            );
        }
        let query = if query.trim().is_empty() {
            "stars:>=0"
        } else {
            query.trim()
        };
        let (repos, total) = github::search_repos(&github::client(), query, &sort, page, 100)?;
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        db::save_repos_period(&conn, &repos, &today(), "search").map_err(|e| e.to_string())?;
        let rows = repos
            .into_iter()
            .map(|r| {
                let analysis = db::get_analysis(&conn, &r.repo_id);
                (r, String::new(), analysis)
            })
            .collect();
        Ok(SearchPage {
            repositories: to_items(&conn, rows),
            total,
            page,
            has_more: page < 10 && (page as i64 * 100) < total,
        })
    })
    .await?
}

#[tauri::command]
async fn explain_repo(state: State<'_>, repo_id: String) -> Result<db::Analysis, String> {
    let path = state.db_path.clone();
    let summaries = state.summaries.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        if let Some(a) = db::get_analysis(&conn, &repo_id) {
            return Ok(with_ai_summary(&summaries, &repo_id, a));
        }
        let repo = db::all_latest(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|(r, _, _)| r.repo_id == repo_id)
            .ok_or("Repositório não encontrado")?
            .0;
        let http = github::client();
        let readme = github::fetch_readme(&http, &repo.owner, &repo.name);
        let a = analyze::heuristic(&http, &conn, &repo, &readme);
        db::save_analysis(&conn, &repo_id, &a).map_err(|e| e.to_string())?;
        Ok(with_ai_summary(&summaries, &repo_id, a))
    })
    .await?
}

fn with_ai_summary(
    service: &summaries::SummaryService,
    repo_id: &str,
    mut analysis: db::Analysis,
) -> db::Analysis {
    if let Some(record) = service
        .records(&[format!("github:{repo_id}")])
        .into_iter()
        .find(|r| r.status == "ready")
    {
        analysis.simple_summary = record.summary;
        analysis.description_pt = record.description_pt;
        analysis.analysis_source = format!("ollama_{}", record.model);
    }
    analysis
}

pub(crate) fn validate_model_id(id: &str) -> Result<(), String> {
    let parts: Vec<_> = id.split('/').collect();
    if !(1..=2).contains(&parts.len())
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    {
        return Err("Identificador do modelo inválido.".into());
    }
    Ok(())
}

#[tauri::command]
async fn search_hf(query: String, cursor: Option<String>) -> Result<models::ModelPage, String> {
    blocking(move || {
        models::search(
            &github::client(),
            &query,
            "trendingScore",
            cursor.as_deref(),
        )
    })
    .await?
}

#[tauri::command]
async fn explain_model(
    state: State<'_>,
    model: models::HfModel,
) -> Result<models::ModelExplanation, String> {
    validate_model_id(&model.id)?;
    let path = state.db_path.clone();
    let summaries = state.summaries.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        if let Some(a) = db::get_explanation(&conn, "hf", &model.id) {
            return Ok(with_model_summary(&summaries, &model.id, a));
        }
        let a = models::explain(&github::client(), &conn, &model)?;
        db::save_explanation(&conn, "hf", &model.id, &a);
        Ok(with_model_summary(&summaries, &model.id, a))
    })
    .await?
}

fn with_model_summary(
    service: &summaries::SummaryService,
    id: &str,
    mut explanation: models::ModelExplanation,
) -> models::ModelExplanation {
    if let Some(record) = service
        .records(&[format!("hf:{id}")])
        .into_iter()
        .find(|r| r.status == "ready")
    {
        explanation.summary = record.summary;
    }
    explanation
}

#[tauri::command]
async fn model_files(id: String) -> Result<models::ModelFiles, String> {
    validate_model_id(&id)?;
    blocking(move || models::files(&github::client(), &id)).await?
}

#[tauri::command]
async fn download_model(id: String, full: bool) -> Result<(), String> {
    validate_model_id(&id)?;
    blocking(move || models::download(&github::client(), &id, full)).await?
}

#[tauri::command]
async fn search_global(
    state: State<'_>,
    query: String,
    page: u32,
) -> Result<Vec<GlobalSearchItem>, String> {
    let path = state.db_path.clone();
    blocking(move || {
        if !(1..=10).contains(&page) {
            return Err("A busca global permite até 10 páginas.".into());
        }
        let q = query.trim();
        let http = github::client();
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let dirs = local::existing_dirs();
        let mut items: Vec<GlobalSearchItem> = Vec::new();

        if q.is_empty() {
            let rows = db::trends(&conn).map_err(|e| e.to_string())?;
            for (repo, _first_seen, _analysis) in rows.into_iter().take(25) {
                let is_mcp = repo.category == "Servidor MCP"
                    || repo.repo_id.to_lowercase().contains("mcp")
                    || repo.description.to_lowercase().contains("model context protocol")
                    || repo.description.to_lowercase().contains("mcp server");
                let kind = if is_mcp { "mcp" } else { "github" };
                let dl = local::find_downloaded(&dirs, &repo.repo_id, &repo.name);
                items.push(GlobalSearchItem {
                    key: format!("{}:{}", kind, repo.repo_id),
                    kind: kind.to_string(),
                    title: repo.name,
                    url: repo.url,
                    description: repo.description.clone(),
                    description_pt: String::new(),
                    category: if is_mcp { "Servidor MCP".into() } else { repo.category },
                    source: "GitHub".into(),
                    is_downloaded: dl.is_some(),
                    downloaded_path: dl.map(|d| settings::github_dir().join(d).display().to_string()),
                });
            }
            let trending = models::fetch_trending(&http).map_err(|e| e.to_string())?;
            for model in trending.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("hf:{}", model.id),
                    kind: "hf".into(),
                    title: model.name,
                    url: model.url,
                    description: model.summary.clone().unwrap_or_default(),
                    description_pt: String::new(),
                    category: model.group,
                    source: "Hugging Face".into(),
                    is_downloaded: model.is_downloaded,
                    downloaded_path: model.downloaded_path,
                });
            }
            let page_ds = datasets::search(&http, "", "trendingScore", None).map_err(|e| e.to_string())?;
            for ds in page_ds.models.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("dataset:{}", ds.id),
                    kind: "dataset".into(),
                    title: ds.name,
                    url: ds.url,
                    description: ds.summary.clone().unwrap_or_default(),
                    description_pt: String::new(),
                    category: ds.group,
                    source: "Hugging Face Datasets".into(),
                    is_downloaded: ds.is_downloaded,
                    downloaded_path: ds.downloaded_path,
                });
            }
            let skills_list = skills::popular(&http).map_err(|e| e.to_string())?;
            for skill in skills_list.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("skill:{}", skill.id),
                    kind: "skill".into(),
                    title: skill.name,
                    url: skill.github_url,
                    description: skill.description,
                    description_pt: String::new(),
                    category: "skills".into(),
                    source: skill.sources.join(", "),
                    is_downloaded: skill.is_downloaded,
                    downloaded_path: if skill.is_downloaded { Some(skill.downloaded_path) } else { None },
                });
            }
        } else {
            let (repos, _total) = github::search_repos(&http, q, "stars", page, 50)?;
            db::save_repos_period(&conn, &repos, &today(), "search").map_err(|e| e.to_string())?;
            for repo in repos {
                let is_mcp = repo.category == "Servidor MCP"
                    || repo.repo_id.to_lowercase().contains("mcp")
                    || repo.description.to_lowercase().contains("model context protocol")
                    || repo.description.to_lowercase().contains("mcp server");
                let kind = if is_mcp { "mcp" } else { "github" };
                let dl = local::find_downloaded(&dirs, &repo.repo_id, &repo.name);
                items.push(GlobalSearchItem {
                    key: format!("{}:{}", kind, repo.repo_id),
                    kind: kind.to_string(),
                    title: repo.name,
                    url: repo.url,
                    description: repo.description.clone(),
                    description_pt: String::new(),
                    category: if is_mcp { "Servidor MCP".into() } else { repo.category },
                    source: "GitHub".into(),
                    is_downloaded: dl.is_some(),
                    downloaded_path: dl.map(|d| settings::github_dir().join(d).display().to_string()),
                });
            }
            let page_models = models::search(&http, q, "downloads", None).map_err(|e| e.to_string())?;
            for model in page_models.models.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("hf:{}", model.id),
                    kind: "hf".into(),
                    title: model.name,
                    url: model.url,
                    description: model.summary.clone().unwrap_or_default(),
                    description_pt: String::new(),
                    category: model.group,
                    source: "Hugging Face".into(),
                    is_downloaded: model.is_downloaded,
                    downloaded_path: model.downloaded_path,
                });
            }
            let page_ds = datasets::search(&http, q, "downloads", None).map_err(|e| e.to_string())?;
            for ds in page_ds.models.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("dataset:{}", ds.id),
                    kind: "dataset".into(),
                    title: ds.name,
                    url: ds.url,
                    description: ds.summary.clone().unwrap_or_default(),
                    description_pt: String::new(),
                    category: ds.group,
                    source: "Hugging Face Datasets".into(),
                    is_downloaded: ds.is_downloaded,
                    downloaded_path: ds.downloaded_path,
                });
            }
            let skills_list = skills::search(&http, q).map_err(|e| e.to_string())?;
            for skill in skills_list.into_iter().take(25) {
                items.push(GlobalSearchItem {
                    key: format!("skill:{}", skill.id),
                    kind: "skill".into(),
                    title: skill.name,
                    url: skill.github_url,
                    description: skill.description,
                    description_pt: String::new(),
                    category: "skills".into(),
                    source: skill.sources.join(", "),
                    is_downloaded: skill.is_downloaded,
                    downloaded_path: if skill.is_downloaded { Some(skill.downloaded_path) } else { None },
                });
            }
        }

        items.sort_by(|a, b| {
            b.is_downloaded.cmp(&a.is_downloaded).then_with(|| a.title.cmp(&b.title))
        });

        Ok(items)
    })
    .await?
}

#[derive(Serialize)]
struct GlobalSearchItem {
    pub key: String,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub description_pt: String,
    pub category: String,
    pub source: String,
    pub is_downloaded: bool,
    pub downloaded_path: Option<String>,
}

#[tauri::command]
async fn search_skills(query: String) -> Result<Vec<skills::Skill>, String> {
    blocking(move || {
        if query.trim().is_empty() {
            skills::popular(&github::client())
        } else {
            skills::search(&github::client(), &query)
        }
    })
    .await?
}

#[derive(Serialize)]
struct ExplainedSkill {
    skill: skills::Skill,
    explanation: skills::SkillExplanation,
}

#[tauri::command]
async fn explain_skill(state: State<'_>, skill: skills::Skill) -> Result<ExplainedSkill, String> {
    let path = state.db_path.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let (skill, explanation) = skills::explain(&github::client(), &conn, skill)?;
        Ok(ExplainedSkill { skill, explanation })
    })
    .await?
}

#[tauri::command]
async fn download_skill(skill: skills::Skill) -> Result<local::ActionResult, String> {
    blocking(move || skills::download(&github::client(), skill)).await?
}

#[tauri::command]
async fn install_ecosystem_skill(skill: skills::Skill) -> Result<serde_json::Value, String> {
    blocking(move || {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
        let base = PathBuf::from(&home).join(".agents").join("skills");
        std::fs::create_dir_all(&base)
            .map_err(|e| format!("Pasta de skills do ecossistema indisponível: {e}"))?;
        let result = skills::install_ecosystem(&github::client(), skill, &base)?;
        let sync_script = PathBuf::from(&home).join(".agents").join("sync-indexes.py");
        let sync = std::process::Command::new("python")
            .arg(sync_script)
            .output()
            .map(|o| {
                if o.status.success() {
                    "sync-indexes.py executado".to_string()
                } else {
                    "sync-indexes.py retornou aviso".to_string()
                }
            })
            .unwrap_or_else(|_| "sync-indexes.py não executado".into());
        Ok(serde_json::json!({
            "success": result.success,
            "folder_path": result.folder_path,
            "message": format!("{} ({})", result.message, sync)
        }))
    })
    .await?
}

#[tauri::command]
async fn list_agent_requests(state: State<'_>) -> Result<Vec<db::AgentRequest>, String> {
    let path = state.db_path.clone();
    blocking(move || {
        db::open(&path)
            .and_then(|c| db::list_agent_requests(&c, 100))
            .map_err(|e| e.to_string())
    })
    .await?
}

/// Aprova ou recusa um pedido de download feito por agente via MCP.
#[tauri::command]
async fn resolve_agent_request(state: State<'_>, id: i64, approve: bool) -> Result<db::AgentRequest, String> {
    let path = state.db_path.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let req = db::get_agent_request(&conn, id).ok_or("Pedido não encontrado.")?;
        if !approve {
            db::set_agent_request(&conn, id, "rejected", "Recusado pelo Victor.", "pending").map_err(|e| e.to_string())?;
            return db::get_agent_request(&conn, id).ok_or_else(|| "Pedido não encontrado.".into());
        }
        // So um aprovador vence: pending -> running (ou retry a partir de error).
        let transitioned = db::set_agent_request(&conn, id, "running", "Baixando…", "pending").map_err(|e| e.to_string())?
            || db::set_agent_request(&conn, id, "running", "Baixando…", "error").map_err(|e| e.to_string())?;
        if !transitioned {
            return Err("Este pedido já foi resolvido.".into());
        }
        let (status, message) = match req.kind.as_str() {
            "repo" => {
                let name = req.target.split('/').nth(1).unwrap_or(&req.target).to_string();
                let r = local::clone_repo(&format!("https://github.com/{}", req.target), &name, None);
                (if r.success { "done" } else { "error" }, r.message)
            }
            "model" | "dataset" => {
                if let Err(e) = validate_model_id(&req.target) {
                    db::set_agent_request(&conn, id, "error", &e, "running").map_err(|e| e.to_string())?;
                    return db::get_agent_request(&conn, id).ok_or_else(|| "Pedido não encontrado.".into());
                }
                db::set_agent_request(
                    &conn,
                    id,
                    "running",
                    "Baixando do Hugging Face; acompanhe em Downloads.",
                    "running",
                )
                .map_err(|e| e.to_string())?;
                let cb_db = path.clone();
                let on_done = Box::new(move |ok: bool, msg: &str| {
                    if let Ok(c) = db::open(&cb_db) {
                        let _ = db::set_agent_request(
                            &c,
                            id,
                            if ok { "done" } else { "error" },
                            msg,
                            "running",
                        );
                    }
                });
                match models::download_with_callback(&github::client(), &req.target, req.full, Some(on_done)) {
                    Ok(()) => {
                        return db::get_agent_request(&conn, id).ok_or_else(|| "Pedido não encontrado.".into());
                    }
                    Err(e) => ("error", e),
                }
            }
            other => ("error", format!("Tipo de pedido desconhecido: {other}")),
        };
        db::set_agent_request(&conn, id, status, &message, "running").map_err(|e| e.to_string())?;
        db::get_agent_request(&conn, id).ok_or_else(|| "Pedido não encontrado.".into())
    })
    .await?
}

/// Análise de stack dos repositórios baixados: manifestos locais, dependências
/// órfãs e pendências de Git LFS, com sugestões automáticas de download.
#[tauri::command]
async fn analyze_local_stacks(state: State<'_>) -> Result<stack::StackAnalysis, String> {
    let path = state.db_path.clone();
    blocking(move || stack::analyze(&path)).await?
}

/// Registra uma sugestão de stack como pedido na fila "Pedidos dos agentes"
/// (nada baixa sem aprovação no app).
#[tauri::command]
async fn request_stack_download(
    state: State<'_>,
    kind: String,
    id: String,
    reason: String,
) -> Result<i64, String> {
    let path = state.db_path.clone();
    blocking(move || {
        let id = id.trim().trim_end_matches('/').to_string();
        validate_model_id(&id)?;
        let kind = match kind.as_str() {
            "repo" | "model" => kind.clone(),
            _ => return Err("Tipo deve ser repo ou model.".into()),
        };
        if kind == "repo" && id.split('/').count() != 2 {
            return Err("Repositório deve estar no formato dono/nome.".into());
        }
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        let reason: String = reason.chars().take(300).collect();
        db::add_agent_request(&conn, &kind, &id, false, "análise-de-stack", &reason)
            .map_err(|e| e.to_string())
    })
    .await?
}

/// Instala o Git LFS via winget (se faltar) e roda `git lfs pull` nas pastas
/// com ponteiros pendentes.
#[tauri::command]
async fn fix_lfs(state: State<'_>) -> Result<stack::LfsFixReport, String> {
    let path = state.db_path.clone();
    blocking(move || stack::fix_lfs(&path)).await?
}

#[tauri::command]
async fn update_downloaded_repos(_state: State<'_>) -> Result<Vec<local::PullOutcome>, String> {    blocking(move || {
        let repos: Vec<String> = local::existing_dirs()
            .into_iter()
            .filter(|d| {
                std::fs::metadata(crate::settings::github_dir().join(d).join(".git"))
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            })
            .collect();
        if repos.is_empty() {
            return Err("Nenhum repositorio git baixado na pasta configurada.".into());
        }
        let next = AtomicUsize::new(0);
        let outcomes: Mutex<Vec<Option<local::PullOutcome>>> =
            Mutex::new((0..repos.len()).map(|_| None).collect());
        std::thread::scope(|s| {
            for _ in 0..PULL_WORKERS {
                s.spawn(|| loop {
                    let idx = next.fetch_add(1, Ordering::Relaxed);
                    let Some(repo) = repos.get(idx) else { break };
                    outcomes.lock().unwrap()[idx] = Some(local::pull_repo(repo));
                });
            }
        });
        let mut list: Vec<local::PullOutcome> =
            outcomes.into_inner().unwrap().into_iter().flatten().collect();
        list.sort_by(|a, b| a.folder.cmp(&b.folder));
        Ok(list)
    })
    .await?
}

#[tauri::command]
fn get_downloads() -> Vec<downloads::Job> {
    downloads::list()
}

#[tauri::command]
fn get_settings() -> settings::PublicSettings {
    settings::public()
}

#[tauri::command]
async fn save_settings(
    github_dir: String,
    hf_dir: String,
    skills_dir: String,
    token: Option<String>,
) -> Result<settings::PublicSettings, String> {
    blocking(move || settings::save(&github_dir, &hf_dir, &skills_dir, token)).await?
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|p| {
                p.into_path()
                    .map(|p| p.display().to_string())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await?
}

#[tauri::command]
async fn get_catalog(
    state: State<'_>,
    request: catalog::CatalogRequest,
) -> Result<catalog::CatalogPage, String> {
    let path = state.db_path.clone();
    let summaries = state.summaries.clone();
    blocking(move || catalog::load(&path, &summaries, request)).await?
}

#[tauri::command]
async fn queue_repo_summaries(
    state: State<'_>,
    repo_ids: Vec<String>,
) -> Result<Vec<summaries::SummaryRecord>, String> {
    validate_summary_keys(&repo_ids)?;
    let path = state.db_path.clone();
    let service = state.summaries.clone();
    blocking(move || {
        let conn = db::open(&path).map_err(|e| e.to_string())?;
        Ok(db::all_latest(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|(repo, _, _)| repo_ids.contains(&repo.repo_id))
            .map(|(repo, _, _)| {
                service.enqueue(summaries::SummaryInput {
                    key: format!("github:{}", repo.repo_id),
                    kind: "github".into(),
                    id: repo.repo_id.clone(),
                    title: repo.repo_id.clone(),
                    description: repo.description,
                    repo: repo.repo_id,
                    branch: String::new(),
                    path: String::new(),
                })
            })
            .collect())
    })
    .await?
}

fn validate_summary_keys(keys: &[String]) -> Result<(), String> {
    if keys.len() > 50 || keys.iter().any(|key| key.len() > 512) {
        return Err("Consulte até 50 resumos por lote.".into());
    }
    Ok(())
}

#[tauri::command]
async fn get_summary_records(
    state: State<'_>,
    keys: Vec<String>,
) -> Result<Vec<summaries::SummaryRecord>, String> {
    validate_summary_keys(&keys)?;
    let service = state.summaries.clone();
    blocking(move || service.records(&keys)).await
}

#[tauri::command]
fn get_summary_status(state: State<'_>) -> summaries::QueueStatus {
    state.summaries.status()
}

#[tauri::command]
async fn retry_summaries(state: State<'_>, keys: Vec<String>) -> Result<(), String> {
    validate_summary_keys(&keys)?;
    let service = state.summaries.clone();
    blocking(move || service.retry(&keys)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            settings::init(dir.join("settings.json"));
            let db_path = dir.join("gittrend.db");
            let conn = db::open(&db_path)?;
            let _ = db::recover_stale_agent_requests(&conn);
            let mut status = SyncStatus {
                message: "Ainda não sincronizado".into(),
                ..Default::default()
            };
            if let Some((date, time, n)) = db::last_sync(&conn) {
                status.message = format!("Última atualização: {time}");
                status.last_sync_date = Some(date);
                status.last_sync_time = Some(time);
                status.total_repos_today = n;
            }
            let state = Arc::new(App {
                summaries: summaries::SummaryService::new(db_path.clone()),
                db_path,
                status: Mutex::new(status),
                handle: app.handle().clone(),
                hf_cache: Mutex::new(None),
                pending_notified: Mutex::new(0),
            });
            app.manage(state.clone());
            let pending_state = state.clone();
            // Sincroniza ao abrir (se ainda nao rodou hoje) e re-checa a cada 4h enquanto o app estiver aberto
            std::thread::spawn(move || {
                loop {
                    run_sync(&state, false);
                    std::thread::sleep(SYNC_EVERY);
                }
            });
            // Avisa na bandeja quando houver pedidos de agentes aguardando aprovacao
            std::thread::spawn(move || loop {
                check_pending_requests(&pending_state);
                std::thread::sleep(PENDING_CHECK_EVERY);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            start_sync,
            get_trends,
            list_agent_requests,
            resolve_agent_request,
            toggle_favorite,
            get_library,
            get_hf_models,
            get_categories,
            clone_repo,
            open_folder,
            open_url,
            ai_models,
            analyze_ollama,
            search_github,
            explain_repo,
            search_hf,
            explain_model,
            model_files,
            download_model,
            search_skills,
            explain_skill,
            download_skill,
            install_ecosystem_skill,
            search_global,
            get_downloads,
            get_settings,
            save_settings,
            pick_folder,
            get_catalog,
            get_summary_records,
            get_summary_status,
            retry_summaries,
            queue_repo_summaries,
            update_downloaded_repos,
            analyze_local_stacks,
            request_stack_download,
            fix_lfs
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o GitTrend Radar");
}

#[cfg(test)]
mod live {
    #[test]
    fn rejects_invalid_model_identifiers() {
        for id in ["owner/model", "bert-base-uncased"] {
            assert!(super::validate_model_id(id).is_ok());
        }
        for id in ["../x", "x/..", "x/y/z", "x?auth=z", "", "/x"] {
            assert!(super::validate_model_id(id).is_err());
        }
    }

    #[test]
    #[ignore]
    fn live_global_catalogs() {
        let http = crate::github::client();
        let (repos, total) =
            crate::github::search_repos(&http, "repo:octocat/Hello-World", "stars", 1, 100)
                .unwrap();
        assert!(total > 0 && repos.iter().any(|r| r.repo_id == "octocat/Hello-World"));
        let first = crate::models::search(&http, "bert", "downloads", None).unwrap();
        assert!(!first.models.is_empty());
        let cursor = first.next_cursor.as_deref().expect("cursor HF");
        let second = crate::models::search(&http, "bert", "downloads", Some(cursor)).unwrap();
        assert!(!second.models.is_empty());
        assert_ne!(first.models[0].id, second.models[0].id);
        let info = crate::models::files(&http, "hf-internal-testing/tiny-random-bert").unwrap();
        assert!(info.full_count > 0 && info.full_bytes > 0);
        println!(
            "GitHub={total}; HF page1={} page2={}; files={}",
            first.models.len(),
            second.models.len(),
            info.full_count
        );
    }

    // Rede real: cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_sync_pipeline() {
        let http = crate::github::client();
        let repos = crate::github::collect_all(&http);
        let new_week = repos.iter().filter(|r| r.stars_today == 0).count();
        println!(
            "coletados={} com_estrelas_hoje={} novos_da_semana={}",
            repos.len(),
            repos.len() - new_week,
            new_week
        );
        assert!(repos.len() > 50);
        let conn = crate::db::open(&std::env::temp_dir().join("gtr_live.db")).unwrap();
        let r = &repos[0];
        let readme = crate::github::fetch_readme(&http, &r.owner, &r.name);
        let a = crate::analyze::heuristic(&http, &conn, r, &readme);
        println!(
            "{} | readme={}B\nDESC_PT: {}\nRESUMO: {}\nPOR QUE: {}\nINSTALL: {}\nCODE:\n{}",
            r.repo_id,
            readme.len(),
            a.description_pt,
            a.simple_summary,
            a.why_trending,
            a.install_cmd,
            a.code_example.chars().take(300).collect::<String>()
        );
        assert!(!a.simple_summary.is_empty());
    }
}
