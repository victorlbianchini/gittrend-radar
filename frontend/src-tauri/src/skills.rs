use std::collections::HashMap;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::analyze::{tidy, translate};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Skill {
    /// "dono/repo::nome"
    pub id: String,
    pub name: String,
    pub repo: String,
    pub branch: String,
    /// Pasta da skill dentro do repo (vazio = ainda nao localizada)
    pub path: String,
    pub description: String,
    pub stars: i64,
    pub installs: i64,
    pub sources: Vec<String>,
    pub github_url: String,
    pub is_downloaded: bool,
    pub downloaded_path: String,
}

fn local_destination(base: &std::path::Path, s: &Skill) -> Result<std::path::PathBuf, String> {
    let repo: Vec<_> = s.repo.split('/').collect();
    if repo.len() != 2
        || repo
            .iter()
            .copied()
            .chain(std::iter::once(s.name.as_str()))
            .any(|part| {
                part.is_empty()
                    || !part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            })
    {
        return Err("Identidade local da skill invalida.".into());
    }
    crate::local::safe_join(base, &format!("{}/{}", s.repo, s.name))
        .ok_or_else(|| "Pasta da skill insegura.".into())
}

fn finalize(mut s: Skill) -> Skill {
    s.is_downloaded = false;
    s.downloaded_path.clear();
    if let Ok(dir) = local_destination(&crate::settings::skills_dir(), &s) {
        if dir.join("SKILL.md").is_file() {
            s.is_downloaded = true;
            s.downloaded_path = dir.display().to_string();
        }
    }
    if !s.is_downloaded {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
        let eco = std::path::PathBuf::from(home).join(".agents").join("skills").join(&s.name);
        if eco.join("SKILL.md").is_file() {
            s.is_downloaded = true;
            s.downloaded_path = eco.display().to_string();
        }
    }
    if s.github_url.is_empty() {
        s.github_url = if s.path.is_empty() {
            format!("https://github.com/{}", s.repo)
        } else {
            format!("https://github.com/{}/tree/{}/{}", s.repo, s.branch, s.path)
        };
    }
    s
}

/// "https://github.com/o/r/tree/main/skills/x" -> ("o/r", "main", "skills/x")
pub fn parse_tree_url(url: &str) -> Option<(String, String, String)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut it = rest.splitn(5, '/');
    let (owner, repo, kind, branch) = (it.next()?, it.next()?, it.next()?, it.next()?);
    if kind != "tree" && kind != "blob" {
        return None;
    }
    if owner.is_empty() || repo.is_empty() || branch.is_empty() {
        return None;
    }
    let raw = it.next().unwrap_or("").trim_end_matches('/');
    let path = raw.strip_suffix("/SKILL.md").unwrap_or(raw);
    let path = if path == "SKILL.md" { "" } else { path };
    if !path.is_empty() && crate::local::safe_join(std::path::Path::new("."), path).is_none() {
        return None;
    }
    let path = path.to_string();
    Some((format!("{owner}/{repo}"), branch.to_string(), path))
}

fn from_skills_sh(http: &Client, q: &str) -> Result<Vec<Skill>, String> {
    let v = http
        .get("https://skills.sh/api/search")
        .query(&[("q", q)])
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json::<serde_json::Value>())
        .map_err(|e| format!("skills.sh: {e}"))?;
    let skills = v["skills"]
        .as_array()
        .ok_or("skills.sh: resposta sem lista de skills")?;
    Ok(skills
        .iter()
        .filter_map(|s| {
            let repo = s["source"].as_str()?.to_string();
            let name = s["skillId"].as_str().or(s["name"].as_str())?.to_string();
            Some(Skill {
                id: format!("{repo}::{name}"),
                name,
                repo,
                branch: "HEAD".into(),
                installs: s["installs"].as_i64().unwrap_or(0),
                sources: vec!["skills.sh".into()],
                ..Default::default()
            })
        })
        .collect())
}

/// Official repositories offer a second source without requiring a paid API key.
fn from_catalog(http: &Client, repo: &str, q: &str) -> Result<Vec<Skill>, String> {
    let tree = repo_tree(http, repo, "HEAD")?;
    let q = q.to_lowercase();
    Ok(tree
        .into_iter()
        .filter_map(|file| {
            let path = file.strip_suffix("/SKILL.md")?;
            let name = path.rsplit('/').next()?;
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                return None;
            }
            let github_url = format!("https://github.com/{repo}/tree/HEAD/{path}");
            let (_, branch, path) = parse_tree_url(&github_url)?;
            Some(Skill {
                id: format!("{repo}::{name}"),
                name: name.into(),
                repo: repo.into(),
                branch,
                path,
                github_url,
                sources: vec!["GitHub".into()],
                ..Default::default()
            })
        })
        .collect())
}

fn collect_sources(results: Vec<Result<Vec<Skill>, String>>) -> Result<Vec<Skill>, String> {
    let mut successes = 0;
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(mut list) => {
                successes += 1;
                skills.append(&mut list);
            }
            Err(error) => errors.push(error),
        }
    }
    if successes == 0 {
        Err(errors.join("; "))
    } else {
        Ok(skills)
    }
}

/// Busca nas duas fontes em paralelo e junta pelo id (repo + nome).
pub fn search(http: &Client, q: &str) -> Result<Vec<Skill>, String> {
    let q = q.trim();
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| from_skills_sh(http, q));
        let b = scope.spawn(|| from_catalog(http, "anthropics/skills", q));
        let c = scope.spawn(|| from_catalog(http, "openai/skills", q));
        [a, b, c]
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("Falha na fonte de skills".into()))
            })
            .collect()
    });
    let results = collect_sources(results)?;
    let mut by_id: HashMap<String, Skill> = HashMap::new();
    for s in results {
        by_id
            .entry(s.id.clone())
            .and_modify(|e| {
                e.installs = e.installs.max(s.installs);
                e.stars = e.stars.max(s.stars);
                if e.path.is_empty() && !s.path.is_empty() {
                    e.path = s.path.clone();
                    e.branch = s.branch.clone();
                    e.github_url = s.github_url.clone();
                }
                if e.description.is_empty() {
                    e.description = s.description.clone();
                }
                for src in &s.sources {
                    if !e.sources.contains(src) {
                        e.sources.push(src.clone());
                    }
                }
            })
            .or_insert(s);
    }
    let mut list: Vec<Skill> = by_id.into_values().map(finalize).collect();
    list.sort_by(|a, b| (b.installs, b.stars).cmp(&(a.installs, a.stars)));
    Ok(list)
}

/// Vitrine inicial: as mais instaladas de varios temas (as APIs exigem um termo de busca).
pub fn popular(http: &Client) -> Result<Vec<Skill>, String> {
    search(http, "")
}

/// Arvore do repo (1 chamada a API) -> todos os arquivos (caminho, url raw).
fn repo_tree(http: &Client, repo: &str, branch: &str) -> Result<Vec<String>, String> {
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
        })
        || branch.is_empty()
        || branch.contains(['?', '#', '\\'])
    {
        return Err("Repositorio ou branch da skill invalido.".into());
    }
    let v: serde_json::Value = crate::github::api_get(
        http,
        &format!("https://api.github.com/repos/{repo}/git/trees/{branch}"),
    )
    .query(&[("recursive", "1")])
    .send()
    .map_err(|e| format!("Sem conexão com o GitHub: {e}"))
    .and_then(|r| match r.status().as_u16() {
        403 | 429 => Err(
            "Limite da API do GitHub atingido. Espere ou cadastre um token nas Configurações."
                .into(),
        ),
        404 => Err("Repositório da skill não encontrado no GitHub.".into()),
        _ => r
            .error_for_status()
            .and_then(|r| r.json())
            .map_err(|e| e.to_string()),
    })?;
    if v["truncated"].as_bool() == Some(true) {
        return Err("Arvore da skill incompleta: GitHub truncou a resposta.".into());
    }
    let tree = v["tree"]
        .as_array()
        .ok_or("GitHub retornou arvore invalida")?;
    Ok(tree
        .iter()
        .filter(|t| t["type"] == "blob")
        .filter_map(|t| t["path"].as_str().map(String::from))
        .collect())
}

/// Garante que sabemos a pasta da skill no repo (skills.sh nao informa o caminho).
pub fn locate(http: &Client, mut s: Skill) -> Result<(Skill, Vec<String>), String> {
    let tree = repo_tree(http, &s.repo, &s.branch)?;
    if s.path.is_empty() {
        let suffix = format!("{}/SKILL.md", s.name);
        let found = tree
            .iter()
            .filter(|p| {
                p.eq_ignore_ascii_case(&suffix)
                    || p.to_lowercase()
                        .ends_with(&format!("/{}", suffix.to_lowercase()))
            })
            .min_by_key(|p| p.len())
            .or_else(|| tree.iter().find(|p| p.eq_ignore_ascii_case("SKILL.md")))
            .ok_or_else(|| {
                format!(
                    "SKILL.md da skill \"{}\" não encontrado em {}.",
                    s.name, s.repo
                )
            })?;
        s.path = found
            .strip_suffix("SKILL.md")
            .unwrap_or(found)
            .trim_end_matches('/')
            .to_string();
    }
    let prefix = if s.path.is_empty() {
        String::new()
    } else {
        format!("{}/", s.path)
    };
    let files: Vec<String> = tree
        .into_iter()
        .filter(|p| p.starts_with(&prefix))
        .collect();
    if !files
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&format!("{prefix}SKILL.md")))
    {
        return Err("A pasta da skill não tem SKILL.md.".into());
    }
    Ok((finalize(s), files))
}

fn raw_url(s: &Skill, file: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}",
        s.repo, s.branch, file
    )
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct SkillExplanation {
    pub summary: String,
    pub when_to_use: Vec<String>,
    pub sections: Vec<String>,
    pub files: Vec<String>,
    pub preview: String,
    pub install_hint: String,
}

fn frontmatter(md: &str) -> (HashMap<String, String>, String) {
    let md = md.replace("\r\n", "\n");
    let Some((front, body)) = md.strip_prefix("---\n").and_then(|r| r.split_once("\n---")) else {
        return (HashMap::new(), md);
    };
    let map = front
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| {
            (
                k.trim().to_string(),
                v.trim().trim_matches(['"', '\'']).to_string(),
            )
        })
        .collect();
    (map, body.to_string())
}

pub fn explain(
    http: &Client,
    conn: &rusqlite::Connection,
    s: Skill,
) -> Result<(Skill, SkillExplanation), String> {
    let (s, files) = locate(http, s)?;
    let md_path = if s.path.is_empty() {
        "SKILL.md".to_string()
    } else {
        format!("{}/SKILL.md", s.path)
    };
    let md = http
        .get(raw_url(&s, &md_path))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Não foi possível ler o SKILL.md: {e}"))?;
    let (front, body) = frontmatter(&md);
    let desc = front
        .get("description")
        .cloned()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| s.description.clone());

    let mut summary = vec![];
    if !desc.is_empty() {
        summary.push(translate(http, conn, &desc));
    }
    if let Some(p) = crate::analyze::clean_markdown(&body)
        .split("\n\n")
        .filter(|p| {
            !p.trim_start()
                .starts_with(['#', '>', '|', '`', '<', '-', '*'])
        })
        .map(tidy)
        .find(|p| p.chars().filter(|c| c.is_alphabetic()).count() > 60 && p.contains('.'))
    {
        summary.push(translate(
            http,
            conn,
            &p.chars().take(600).collect::<String>(),
        ));
    }

    // Itens da secao "When to use"/"Quando usar", se existir
    let mut when = vec![];
    let mut inside = false;
    let mut sections = vec![];
    for line in body.lines().map(str::trim) {
        if let Some(h) = line.strip_prefix('#') {
            let h = h.trim_start_matches('#').trim();
            sections.push(h.to_string());
            let l = h.to_lowercase();
            inside = l.contains("when to use")
                || l.contains("quando usar")
                || l.contains("use when")
                || l.contains("when to activate");
            continue;
        }
        if inside {
            if let Some(item) = line.strip_prefix("- ").or(line.strip_prefix("* ")) {
                if when.len() < 6 {
                    when.push(translate(http, conn, item));
                }
            }
        }
    }
    sections.truncate(14);

    let rel: Vec<String> = files
        .iter()
        .map(|f| f[s.path.len()..].trim_start_matches('/').to_string())
        .collect();
    Ok((
        s.clone(),
        SkillExplanation {
            summary: if summary.is_empty() {
                "O SKILL.md não traz descrição.".into()
            } else {
                summary.join("\n\n")
            },
            when_to_use: when,
            sections,
            preview: md.chars().take(4000).collect(),
            install_hint: format!(
                "Projeto: copie a pasta para .claude/skills/{0}/  |  Global: ~/.claude/skills/{0}/  |  Codex/Gemini: .agents/skills/{0}/",
                crate::local::sanitize(&s.name)
            ),
            files: rel,
        },
    ))
}

/// Keep incomplete downloads separate; publish only a fully downloaded directory.
/// Failed staging directories remain available for inspection and never block retries.
fn stage_download(
    destination: &std::path::Path,
    write_files: impl FnOnce(&std::path::Path) -> Result<(), String>,
) -> Result<(), String> {
    if destination.exists() || crate::local::has_reparse_ancestor(destination) {
        return Err(
            "A pasta da skill ja existe ou e insegura. Nenhum arquivo foi substituido.".into(),
        );
    }
    let parent = destination.parent().ok_or("Pasta de destino invalida")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let staging = loop {
        let path = parent.join(format!(
            ".gtr-skill-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => break path,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let result = (|| {
        write_files(&staging)?;
        if destination.exists() || crate::local::has_reparse_ancestor(destination) {
            return Err("Destino existente ou inseguro; arquivos originais preservados.".into());
        }
        std::fs::rename(&staging, destination)
            .map_err(|e| format!("Nao foi possivel publicar a skill: {e}"))
    })();
    result.map_err(|error| {
        format!(
            "{error} Arquivos parciais preservados em {}. Voce pode tentar novamente.",
            staging.display()
        )
    })
}

/// Baixa a pasta inteira da skill para <Skills>/<dono>/<repositorio>/<nome>.
pub fn download(http: &Client, s: Skill) -> Result<crate::local::ActionResult, String> {
    let (s, files) = locate(http, s)?;
    let dest_root = local_destination(&crate::settings::skills_dir(), &s)?;
    download_files(http, &s, &files, dest_root)
}

/// Instala a skill no ecossistema de agentes em `<base>/<nome>/SKILL.md` (layout plano do hub
/// `.agents\skills`). Decisao do Victor (17/10): instalacao automatica pela MCP. Nunca sobrescreve
/// uma skill existente com o mesmo nome.
pub fn install_ecosystem(
    http: &Client,
    s: Skill,
    base: &std::path::Path,
) -> Result<crate::local::ActionResult, String> {
    let (s, files) = locate(http, s)?;
    let name = crate::local::sanitize(&s.name);
    let dest_root = crate::local::safe_join(base, &name)
        .filter(|p| p.parent() == Some(base))
        .ok_or_else(|| format!("Nome de skill inseguro: {}", s.name))?;
    if dest_root.exists() {
        return Err(format!(
            "Ja existe uma skill chamada \"{name}\" em {}. Nada foi substituido.",
            base.display()
        ));
    }
    download_files(http, &s, &files, dest_root)
}

fn download_files(
    http: &Client,
    s: &Skill,
    files: &[String],
    dest_root: std::path::PathBuf,
) -> Result<crate::local::ActionResult, String> {
    let job = format!("skill:{}", s.id);
    let total = files.len() as u64;
    // Validate every remote path before creating anything or making network requests.
    let relative_files = files
        .iter()
        .map(|f| {
            let rel = f[s.path.len()..].trim_start_matches('/');
            crate::local::safe_join(&dest_root, rel)
                .ok_or_else(|| format!("Caminho inseguro na skill: {rel}"))?;
            Ok(rel.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    crate::downloads::try_start(&job, &format!("Skill {}", s.name), 0, &dest_root)?;
    let mut saved = 0;
    stage_download(&dest_root, |staging| {
        for (file, rel) in files.iter().zip(&relative_files) {
            let dest = crate::local::safe_join(staging, rel).ok_or("Pasta temporaria insegura")?;
            crate::downloads::fetch_to_file(http, &job, &raw_url(&s, file), &dest, None)?;
            saved += 1;
        }
        Ok(())
    })
    .inspect_err(|error| crate::downloads::finish(&job, false, error.clone()))?;
    let msg = format!(
        "{saved} de {total} arquivos salvos em {}",
        dest_root.display()
    );
    crate::downloads::finish(&job, true, msg.clone());
    Ok(crate::local::ActionResult {
        success: true,
        already_exists: false,
        message: msg,
        folder_name: Some(crate::local::sanitize(&s.name)),
        folder_path: Some(dest_root.display().to_string()),
    })
}

pub fn list_downloaded() -> Vec<Skill> {
    let mut list = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let skills_dir = crate::settings::skills_dir();
    scan_skills_dir(&skills_dir, 3, &mut list, &mut seen_ids);

    list
}

fn scan_skills_dir(
    dir: &std::path::Path,
    depth: usize,
    out: &mut Vec<Skill>,
    seen: &mut std::collections::HashSet<String>,
) {
    if depth == 0 || !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if skill_md.is_file() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let repo = if !parent_name.is_empty() && parent_name != "Skills" && parent_name != "skills" {
                format!("{parent_name}/{folder_name}")
            } else {
                format!("skills/{folder_name}")
            };
            let id = format!("{repo}::{folder_name}");
            if seen.insert(id.clone()) {
                let mut name = folder_name.clone();
                let mut description = format!("Skill {}", folder_name);
                if let Ok(content) = std::fs::read_to_string(&skill_md) {
                    let (fm, _) = frontmatter(&content);
                    if let Some(n) = fm.get("name").filter(|s| !s.is_empty()) {
                        name = n.clone();
                    }
                    if let Some(d) = fm.get("description").filter(|s| !s.is_empty()) {
                        description = d.clone();
                    }
                }
                out.push(Skill {
                    id,
                    name,
                    repo,
                    branch: "main".into(),
                    path: String::new(),
                    description,
                    github_url: String::new(),
                    sources: vec!["Local".into()],
                    stars: 0,
                    installs: 0,
                    is_downloaded: true,
                    downloaded_path: path.display().to_string(),
                });
            }
        } else {
            scan_skills_dir(&path, depth - 1, out, seen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_sources_and_rejects_unsafe_identity() {
        let base = std::env::temp_dir().join("gtr_skill_identity");
        let skill = Skill {
            repo: "first/catalog".into(),
            name: "pdf".into(),
            ..Default::default()
        };
        let other = Skill {
            repo: "second/catalog".into(),
            ..skill.clone()
        };
        assert_ne!(
            local_destination(&base, &skill).unwrap(),
            local_destination(&base, &other).unwrap()
        );
        assert!(local_destination(
            &base,
            &Skill {
                name: "../pdf".into(),
                ..skill.clone()
            }
        )
        .is_err());
        assert!(local_destination(
            &base,
            &Skill {
                name: "CON".into(),
                ..skill.clone()
            }
        )
        .is_err());
        assert!(local_destination(
            &base,
            &Skill {
                repo: "../catalog".into(),
                ..skill
            }
        )
        .is_err());
    }

    #[test]
    fn partial_download_can_retry_without_replacing_existing_files() {
        let root = std::env::temp_dir().join(format!(
            "gtr_skill_staging_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let destination = root.join("complete");
        let failure = stage_download(&destination, |staging| {
            std::fs::write(staging.join("SKILL.md"), "partial").unwrap();
            Err("simulated interrupted download".into())
        });
        assert!(failure
            .unwrap_err()
            .contains("simulated interrupted download"));
        assert!(!destination.exists());
        let partial = std::fs::read_dir(&root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read_to_string(partial.join("SKILL.md")).unwrap(),
            "partial"
        );
        stage_download(&destination, |staging| {
            std::fs::write(staging.join("SKILL.md"), "complete").map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "complete"
        );
        assert!(stage_download(&destination, |_| panic!("must not touch existing skill")).is_err());
        assert_eq!(
            std::fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "complete"
        );
        for directory in [&partial, &destination] {
            std::fs::remove_file(directory.join("SKILL.md")).unwrap();
            std::fs::remove_dir(directory).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    #[ignore = "requires public skills.sh and GitHub network access"]
    fn live_sources_smoke() {
        let http = crate::github::client();
        let sh = from_skills_sh(&http, "react").expect("skills.sh search");
        assert!(!sh.is_empty());
        for repo in ["anthropics/skills", "openai/skills"] {
            let skills = from_catalog(&http, repo, "").expect(repo);
            assert!(!skills.is_empty(), "{repo}");
            assert!(skills.iter().all(|s| !s.path.is_empty() && s.repo == repo));
        }
    }

    #[test]
    fn parses_urls_and_frontmatter() {
        assert_eq!(
            parse_tree_url(
                "https://github.com/mattpocock/skills/tree/main/skills/in-progress/setup-ts"
            ),
            Some((
                "mattpocock/skills".into(),
                "main".into(),
                "skills/in-progress/setup-ts".into()
            ))
        );
        assert_eq!(parse_tree_url("https://example.com/x"), None);
        assert_eq!(
            parse_tree_url("https://github.com/a/b/blob/main/SKILL.md"),
            Some(("a/b".into(), "main".into(), "".into()))
        );
        assert_eq!(
            parse_tree_url("https://github.com/a/b/tree/main/x/SKILL.md/SKILL.md")
                .unwrap()
                .2,
            "x/SKILL.md"
        );
        assert!(collect_sources(vec![Err("offline".into()), Err("denied".into())]).is_err());
        assert!(collect_sources(vec![Ok(vec![]), Err("offline".into())])
            .unwrap()
            .is_empty());
        let (f, body) =
            frontmatter("---\nname: pdf\ndescription: \"Work with PDFs\"\n---\n# PDF\ntext");
        assert_eq!(
            f.get("description").map(String::as_str),
            Some("Work with PDFs")
        );
        assert!(body.contains("# PDF"));
    }
}
