use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Repo {
    pub repo_id: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub language: String,
    pub stars: i64,
    pub forks: i64,
    pub stars_today: i64,
    #[serde(default)]
    pub stars_per_day: i64,
    #[serde(default)]
    pub repo_created_at: String,
    pub category: String,
    pub rank: i64,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Analysis {
    pub description_pt: String,
    pub simple_summary: String,
    pub why_trending: String,
    pub code_example: String,
    pub when_to_select: Vec<String>,
    pub when_to_avoid: Vec<String>,
    pub quick_verdict: String,
    pub key_features: Vec<String>,
    pub tech_stack: Vec<String>,
    pub target_audience: String,
    pub install_cmd: String,
    pub analysis_source: String,
}

/// Item enviado para a interface: repo + analise + flags de novidade e download.
#[derive(Serialize, Clone, Debug)]
pub struct TrendItem {
    #[serde(flatten)]
    pub repo: Repo,
    pub first_seen: String,
    pub is_new: bool,
    pub has_analysis: bool,
    pub simple_summary: Option<String>,
    pub why_trending: Option<String>,
    pub quick_verdict: Option<String>,
    pub when_to_select: Vec<String>,
    pub install_cmd: Option<String>,
    pub analysis: Option<Analysis>,
    pub is_downloaded: bool,
    pub is_favorite: bool,
    pub downloaded_folder: Option<String>,
    pub downloaded_path: Option<String>,
    pub pretty_name: String,
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(30))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS repositories (
            repo_id TEXT, owner TEXT, name TEXT, description TEXT, url TEXT, language TEXT,
            stars INTEGER DEFAULT 0, forks INTEGER DEFAULT 0, stars_today INTEGER DEFAULT 0,
            period TEXT DEFAULT 'daily', category TEXT DEFAULT 'geral', date_collected TEXT,
            rank INTEGER DEFAULT 1, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (repo_id, date_collected, period));
        CREATE INDEX IF NOT EXISTS idx_repos_date ON repositories (date_collected);
        CREATE TABLE IF NOT EXISTS repo_analysis (repo_id TEXT PRIMARY KEY, data TEXT NOT NULL,
            analyzed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP);
        CREATE TABLE IF NOT EXISTS translations (src TEXT PRIMARY KEY, pt TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS favorites (repo_id TEXT PRIMARY KEY, added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP);
        CREATE TABLE IF NOT EXISTS agent_requests (id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, target TEXT NOT NULL, full INTEGER NOT NULL DEFAULT 0, agent TEXT NOT NULL DEFAULT '', reason TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'pending', message TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')), resolved_at TEXT);
        CREATE TABLE IF NOT EXISTS explanations (kind TEXT, id TEXT, data TEXT NOT NULL, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (kind, id));
        CREATE TABLE IF NOT EXISTS sync_history (id INTEGER PRIMARY KEY AUTOINCREMENT,
            sync_time TEXT, date_str TEXT, repos_found INTEGER, status TEXT, message TEXT,
            duration_sec REAL);",
    )?;
    let _ = conn.execute(
        "ALTER TABLE repositories ADD COLUMN repo_created_at TEXT DEFAULT ''",
        [],
    );
    Ok(conn)
}

pub fn save_repos(conn: &Connection, repos: &[Repo], date: &str) -> rusqlite::Result<()> {
    save_repos_period(conn, repos, date, "daily")
}

/// period: "daily" (Trending) ou "search" (resultado de busca; nao conta como novidade).
pub fn save_repos_period(
    conn: &Connection,
    repos: &[Repo],
    date: &str,
    period: &str,
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT INTO repositories (repo_id, owner, name, description, url, language, stars, forks,
                stars_today, period, category, date_collected, rank, repo_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?13, ?10, ?11, ?12, ?14)
             ON CONFLICT(repo_id, date_collected, period) DO UPDATE SET
                description = excluded.description, stars = excluded.stars, forks = excluded.forks,
                stars_today = excluded.stars_today, language = excluded.language,
                category = excluded.category, rank = excluded.rank,
                repo_created_at = CASE WHEN excluded.repo_created_at != '' THEN excluded.repo_created_at ELSE repositories.repo_created_at END",
        )?;
        for r in repos {
            st.execute(params![
                r.repo_id,
                r.owner,
                r.name,
                r.description,
                r.url,
                r.language,
                r.stars,
                r.forks,
                r.stars_today,
                r.category,
                date,
                r.rank,
                period,
                r.repo_created_at
            ])?;
        }
    }
    tx.commit()
}

pub fn get_analysis(conn: &Connection, repo_id: &str) -> Option<Analysis> {
    conn.query_row(
        "SELECT data FROM repo_analysis WHERE repo_id = ?1",
        [repo_id],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn save_analysis(conn: &Connection, repo_id: &str, a: &Analysis) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO repo_analysis (repo_id, data, analyzed_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(repo_id) DO UPDATE SET data = excluded.data, analyzed_at = excluded.analyzed_at",
        params![repo_id, serde_json::to_string(a).unwrap_or_default()],
    )?;
    Ok(())
}

pub fn cached_translation(conn: &Connection, src: &str) -> Option<String> {
    conn.query_row("SELECT pt FROM translations WHERE src = ?1", [src], |r| {
        r.get(0)
    })
    .optional()
    .ok()
    .flatten()
}

pub fn save_translation(conn: &Connection, src: &str, pt: &str) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO translations (src, pt) VALUES (?1, ?2)",
        params![src, pt],
    );
}

pub fn latest_date(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT MAX(date_collected) FROM repositories WHERE period = 'daily'",
        [],
        |r| r.get(0),
    )
    .ok()
    .flatten()
}

/// Repos da data mais recente, com analise e `first_seen` (primeiro dia em que o repo apareceu).
pub fn trends(conn: &Connection) -> rusqlite::Result<Vec<(Repo, String, Option<Analysis>)>> {
    let Some(date) = latest_date(conn) else {
        return Ok(vec![]);
    };
    let mut st = conn.prepare(
        "SELECT r.repo_id, r.owner, r.name, r.description, r.url, r.language, r.stars, r.forks,
                r.stars_today, r.category, r.rank,
                COALESCE((SELECT MIN(r2.date_collected) FROM repositories r2 WHERE r2.repo_id = r.repo_id AND r2.period = 'daily'), r.date_collected),
                a.data, COALESCE(r.repo_created_at, '')
         FROM repositories r LEFT JOIN repo_analysis a ON a.repo_id = r.repo_id
         WHERE r.date_collected = ?1 AND r.period = 'daily'
         ORDER BY r.stars_today DESC, r.stars DESC",
    )?;
    let rows = st.query_map([&date], row)?;
    rows.collect()
}

type Row = (Repo, String, Option<Analysis>);

fn row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    let stars: i64 = r.get(6)?;
    let repo_created_at: String = r.get::<_, Option<String>>(13)?.unwrap_or_default();
    let stars_per_day = crate::github::compute_stars_per_day(
        stars,
        &repo_created_at,
        chrono::Local::now().date_naive(),
    );
    Ok((
        Repo {
            repo_id: r.get(0)?,
            owner: r.get(1)?,
            name: r.get(2)?,
            description: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            url: r.get(4)?,
            language: r
                .get::<_, Option<String>>(5)?
                .unwrap_or_else(|| "Unknown".into()),
            stars,
            forks: r.get(7)?,
            stars_today: r.get(8)?,
            stars_per_day,
            repo_created_at,
            category: r.get(9)?,
            rank: r.get(10)?,
        },
        r.get::<_, String>(11)?,
        r.get::<_, Option<String>>(12)?
            .and_then(|s| serde_json::from_str(&s).ok()),
    ))
}

/// Ultima versao de cada repo ja visto (base para Favoritos e Baixados, que nao dependem do Trending de hoje).
pub fn all_latest(conn: &Connection) -> rusqlite::Result<Vec<Row>> {
    let mut st = conn.prepare(
        "SELECT r.repo_id, r.owner, r.name, r.description, r.url, r.language, r.stars, r.forks,
                r.stars_today, r.category, r.rank,
                COALESCE((SELECT MIN(r2.date_collected) FROM repositories r2 WHERE r2.repo_id = r.repo_id AND r2.period = 'daily'), ''),
                a.data, COALESCE(r.repo_created_at, '')
         FROM repositories r LEFT JOIN repo_analysis a ON a.repo_id = r.repo_id
         WHERE r.rowid = (SELECT r3.rowid FROM repositories r3 WHERE r3.repo_id = r.repo_id
             ORDER BY r3.date_collected DESC, CASE WHEN r3.period = 'daily' THEN 0 ELSE 1 END LIMIT 1)
         ORDER BY r.stars DESC",
    )?;
    let rows = st.query_map([], row)?;
    rows.collect()
}

pub fn favorite_ids(conn: &Connection) -> std::collections::HashSet<String> {
    let Ok(mut st) = conn.prepare("SELECT repo_id FROM favorites") else {
        return Default::default();
    };
    st.query_map([], |r| r.get(0))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// Liga/desliga favorito. Retorna o novo estado.
pub fn toggle_favorite(conn: &Connection, repo_id: &str) -> rusqlite::Result<bool> {
    if conn.execute("DELETE FROM favorites WHERE repo_id = ?1", [repo_id])? > 0 {
        return Ok(false);
    }
    conn.execute("INSERT INTO favorites (repo_id) VALUES (?1)", [repo_id])?;
    Ok(true)
}

pub fn last_sync(conn: &Connection) -> Option<(String, String, i64)> {
    conn.query_row(
        "SELECT date_str, sync_time, repos_found FROM sync_history WHERE status = 'success' ORDER BY id DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

pub fn log_sync(
    conn: &Connection,
    date: &str,
    time: &str,
    found: i64,
    status: &str,
    msg: &str,
    secs: f64,
) {
    let _ = conn.execute(
        "INSERT INTO sync_history (sync_time, date_str, repos_found, status, message, duration_sec)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![time, date, found, status, msg, secs],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_seen_marks_only_new_repos() {
        let dir = std::env::temp_dir().join(format!("gtr_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&dir);
        let conn = open(&dir).unwrap();
        let mk = |id: &str| Repo {
            repo_id: id.into(),
            owner: "o".into(),
            name: id.into(),
            url: "u".into(),
            category: "geral".into(),
            ..Default::default()
        };
        save_repos(&conn, &[mk("old")], "2026-10-13").unwrap();
        save_repos(&conn, &[mk("old"), mk("new")], "2026-10-14").unwrap();
        let t = trends(&conn).unwrap();
        let seen: Vec<_> = t
            .iter()
            .map(|(r, f, _)| (r.repo_id.as_str(), f.as_str()))
            .collect();
        assert!(seen.contains(&("old", "2026-10-13")));
        assert!(seen.contains(&("new", "2026-10-14")));
        assert!(toggle_favorite(&conn, "old").unwrap());
        assert!(favorite_ids(&conn).contains("old"));
        assert!(!toggle_favorite(&conn, "old").unwrap());
        assert!(favorite_ids(&conn).is_empty());
        // "old" saiu do trending de hoje? continua acessivel com os dados mais recentes
        assert_eq!(all_latest(&conn).unwrap().len(), 2);
        save_repos_period(
            &conn,
            &[mk("only-search"), mk("old")],
            "2026-10-14",
            "search",
        )
        .unwrap();
        assert_eq!(trends(&conn).unwrap().len(), 2);
        let all = all_latest(&conn).unwrap();
        assert_eq!(all.len(), 3);
        assert!(
            all.iter()
                .find(|(r, _, _)| r.repo_id == "only-search")
                .unwrap()
                .1
                .is_empty()
        );
        assert_eq!(
            all.iter().find(|(r, _, _)| r.repo_id == "old").unwrap().1,
            "2026-10-13"
        );
        let sample = serde_json::json!({"summary": "example"});
        save_explanation(&conn, "hf", "test", &sample);
        assert!(get_explanation::<serde_json::Value>(&conn, "hf", "test").is_some());
        conn.execute(
            "UPDATE explanations SET created_at = datetime('now', '-2 hours')",
            [],
        )
        .unwrap();
        assert!(get_explanation::<serde_json::Value>(&conn, "hf", "test").is_none());
        drop(conn);
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn recovers_stale_requests_reconciling_verified_clones() {
        let root = std::env::temp_dir().join(format!(
            "gittrend-db-recover-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let cloned = root.join("Deepseek Harness").join(".git");
        std::fs::create_dir_all(&cloned).unwrap();
        std::fs::write(cloned.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(
            cloned.join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/deepseek-ai/deepseek-harness.git\n",
        )
        .unwrap();

        let db_file = root.join("test.db");
        let conn = open(&db_file).unwrap();
        let id_ok = add_agent_request(
            &conn,
            "repo",
            "deepseek-ai/deepseek-harness",
            false,
            "alfred",
            "harness",
        )
        .unwrap();
        let id_missing = add_agent_request(
            &conn,
            "repo",
            "foo/bar-missing",
            false,
            "alfred",
            "missing",
        )
        .unwrap();
        set_agent_request(&conn, id_ok, "running", "Baixando...", "pending").unwrap();
        set_agent_request(&conn, id_missing, "running", "Baixando...", "pending").unwrap();

        let n = recover_stale_agent_requests_in(&conn, &root).unwrap();
        assert_eq!(n, 2);

        let r_ok = get_agent_request(&conn, id_ok).unwrap();
        assert_eq!(r_ok.status, "done");
        assert!(r_ok.message.contains("Deepseek Harness"));

        let r_missing = get_agent_request(&conn, id_missing).unwrap();
        assert_eq!(r_missing.status, "error");

        drop(conn);
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// Cache de explicacoes de modelos do Hugging Face e skills (kind = "hf" | "skill").
pub fn get_explanation<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Option<T> {
    conn.query_row(
        "SELECT data FROM explanations WHERE kind = ?1 AND id = ?2 AND created_at >= datetime('now', '-1 hour')",
        params![kind, id],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn save_explanation<T: Serialize>(conn: &Connection, kind: &str, id: &str, data: &T) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO explanations (kind, id, data, created_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
        params![kind, id, serde_json::to_string(data).unwrap_or_default()],
    );
}

/// Pedido de download feito por um agente via MCP. So executa depois de aprovado no app.
#[derive(Serialize, Clone, Debug)]
pub struct AgentRequest {
    pub id: i64,
    /// "repo" | "model"
    pub kind: String,
    pub target: String,
    pub full: bool,
    pub agent: String,
    pub reason: String,
    /// "pending" | "approved" | "running" | "done" | "rejected" | "error"
    pub status: String,
    pub message: String,
    pub created_at: String,
}

/// Cria o pedido; se ja houver um pendente igual, devolve o existente (sem duplicar).
pub fn add_agent_request(conn: &Connection, kind: &str, target: &str, full: bool, agent: &str, reason: &str) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM agent_requests WHERE kind = ?1 AND target = ?2 AND full = ?3 AND status = 'pending'",
            params![kind, target, full],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO agent_requests (kind, target, full, agent, reason) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![kind, target, full, agent, reason],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_agent_requests(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<AgentRequest>> {
    let mut st = conn.prepare(
        "SELECT id, kind, target, full, agent, reason, status, message, created_at FROM agent_requests
         ORDER BY CASE status WHEN 'pending' THEN 0 WHEN 'running' THEN 1 ELSE 2 END, id DESC LIMIT ?1",
    )?;
    let rows = st.query_map([limit], |r| {
        Ok(AgentRequest {
            id: r.get(0)?,
            kind: r.get(1)?,
            target: r.get(2)?,
            full: r.get(3)?,
            agent: r.get(4)?,
            reason: r.get(5)?,
            status: r.get(6)?,
            message: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    rows.collect()
}

pub fn get_agent_request(conn: &Connection, id: i64) -> Option<AgentRequest> {
    list_agent_requests(conn, 500).ok()?.into_iter().find(|r| r.id == id)
}

/// Muda o status. `only_from` evita corrida: so troca se o status atual for o esperado.
pub fn set_agent_request(conn: &Connection, id: i64, status: &str, message: &str, only_from: &str) -> rusqlite::Result<bool> {
    let n = conn.execute(
        "UPDATE agent_requests SET status = ?2, message = ?3, resolved_at = datetime('now','localtime') WHERE id = ?1 AND status = ?4",
        params![id, status, message, only_from],
    )?;
    Ok(n == 1)
}

/// Recupera pedidos que ficaram presos em `running` se o app ou processo fechou no meio.
pub fn recover_stale_agent_requests(conn: &Connection) -> rusqlite::Result<usize> {
    recover_stale_agent_requests_in(conn, &crate::settings::github_dir())
}

pub(crate) fn recover_stale_agent_requests_in(
    conn: &Connection,
    github_dir: &std::path::Path,
) -> rusqlite::Result<usize> {
    let running: Vec<(i64, String, String)> = {
        let mut s = conn.prepare(
            "SELECT id, kind, target FROM agent_requests WHERE status = 'running'",
        )?;
        let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut updated = 0usize;
    for (id, kind, target) in running {
        if kind == "repo" {
            if let Some(cloned_path) = crate::local::verified_clone_in(github_dir, &target) {
                let msg = format!("Clonado em: {}", cloned_path.display());
                if set_agent_request(conn, id, "done", &msg, "running")? {
                    updated += 1;
                }
                continue;
            }
        }
        if set_agent_request(
            conn,
            id,
            "error",
            "Download interrompido ao fechar o app. Clique em Tentar novamente para retomar.",
            "running",
        )? {
            updated += 1;
        }
    }
    Ok(updated)
}
