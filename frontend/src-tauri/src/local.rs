use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static GITHUB_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/?$").unwrap()
});

#[derive(Serialize)]
pub struct ActionResult {
    pub success: bool,
    pub already_exists: bool,
    pub message: String,
    pub folder_name: Option<String>,
    pub folder_path: Option<String>,
}

impl ActionResult {
    fn fail(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            already_exists: false,
            message: msg.into(),
            folder_name: None,
            folder_path: None,
        }
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Nome de pasta legivel no padrao de E:\REPOSITORIOS: "agent-zero" -> "Agent Zero", "fastapi" -> "FastAPI".
pub fn pretty_name(name: &str) -> String {
    const KNOWN: &[(&str, &str)] = &[
        ("fastapi", "FastAPI"),
        ("nextjs", "Next.js"),
        ("langchain", "LangChain"),
        ("autogpt", "AutoGPT"),
        ("autogen", "AutoGen"),
        ("crewai", "CrewAI"),
        ("vllm", "vLLM"),
        ("llamaindex", "LlamaIndex"),
        ("pytorch", "PyTorch"),
        ("tensorflow", "TensorFlow"),
        ("tailwindcss", "TailwindCSS"),
        ("vscode", "VSCode"),
        ("ffmpeg", "FFmpeg"),
        ("ollama", "Ollama"),
        ("supabase", "Supabase"),
    ];
    let lookup = |s: &str| {
        KNOWN
            .iter()
            .find(|(k, _)| *k == norm(s))
            .map(|(_, v)| v.to_string())
    };
    if let Some(v) = lookup(name) {
        return v;
    }
    let pretty = name
        .split(['-', '_', '.'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            lookup(p).unwrap_or_else(|| {
                if p.len() <= 3 && p.chars().all(|c| c.is_ascii_alphabetic()) {
                    p.to_uppercase()
                } else {
                    let mut c = p.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default()
                }
            })
        })
        .collect::<Vec<String>>()
        .join(" ");
    sanitize(&pretty)
}

/// Remove caracteres invalidos no Windows e impede "..", barras e afins.
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !r#"<>:"/\|?*"#.contains(*c) && !c.is_control())
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();
    if cleaned.is_empty() {
        "repositorio".into()
    } else {
        cleaned
    }
}

pub fn existing_dirs() -> Vec<String> {
    std::fs::read_dir(crate::settings::github_dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Ja baixado se alguma pasta bate com o nome do repo ou o nome bonito (ignorando caixa e separadores).
fn name_matches(dir: &str, name: &str) -> bool {
    let d = norm(dir);
    d == norm(name) || d == norm(&pretty_name(name))
}

/// Ja baixado = pasta com o nome do repo no diretorio configurado.
/// Prioriza correspondencia exata do remote git (.git/config); se nao houver,
/// considera qualquer pasta local cujo nome coincida com o repositorio.
pub fn find_downloaded<'a>(dirs: &'a [String], repo_id: &str, name: &str) -> Option<&'a String> {
    if let Some(matched) = dirs.iter().filter(|d| name_matches(d, name)).find(|d| {
        std::fs::read_to_string(crate::settings::github_dir().join(d).join(".git/config"))
            .is_ok_and(|cfg| remote_matches(&cfg, repo_id))
    }) {
        return Some(matched);
    }
    dirs.iter().find(|d| name_matches(d, name))
}

/// Retorna o caminho local apenas quando o clone git do `repo_id` está íntegro (`.git/HEAD` e remote batendo).
pub(crate) fn verified_clone_in(base: &Path, repo_id: &str) -> Option<PathBuf> {
    let name = repo_id.split('/').nth(1).unwrap_or(repo_id);
    let rd = std::fs::read_dir(base).ok()?;
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        if !name_matches(&dir_name, name) {
            continue;
        }
        if !path.join(".git/HEAD").is_file() {
            continue;
        }
        if std::fs::read_to_string(path.join(".git/config"))
            .is_ok_and(|cfg| remote_matches(&cfg, repo_id))
        {
            return Some(path);
        }
    }
    None
}

fn remote_matches(config: &str, repo_id: &str) -> bool {
    config
        .lines()
        .filter_map(|line| line.trim().split_once('='))
        .filter(|(key, _)| key.trim() == "url")
        .any(|(_, value)| {
            let value = value.trim().trim_end_matches('/');
            let remote = value
                .strip_prefix("https://github.com/")
                .or_else(|| value.strip_prefix("git@github.com:"));
            remote.is_some_and(|r| {
                r.strip_suffix(".git")
                    .unwrap_or(r)
                    .eq_ignore_ascii_case(repo_id)
            })
        })
}

fn run_with_timeout(cmd: &mut Command, limit: Duration) -> Result<(bool, String), String> {
    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("git não encontrado: {e}"))?;
    // Drain concurrently: git can fill stderr's pipe before exiting.
    let stderr = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut message = Vec::new();
        let mut stderr = stderr;
        let mut buf = [0u8; 8192];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            if message.len() < 65536 {
                let remaining = 65536 - message.len();
                message.extend_from_slice(&buf[..n.min(remaining)]);
            }
        }
        String::from_utf8_lossy(&message).trim().to_string()
    });
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok((status.success(), reader.join().unwrap_or_default()));
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Tempo limite esgotado ao clonar.".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn clone_repo(url: &str, name: &str, custom: Option<&str>) -> ActionResult {
    let url = url.trim();
    if !GITHUB_URL.is_match(url) {
        return ActionResult::fail("URL inválida: apenas https://github.com/<dono>/<repo>.");
    }
    let folder = sanitize(
        custom
            .filter(|c| !c.trim().is_empty())
            .unwrap_or(&pretty_name(name)),
    );
    let base = crate::settings::github_dir();
    if let Err(e) = std::fs::create_dir_all(&base) {
        return ActionResult::fail(format!(
            "Pasta do GitHub inválida ({}): {e}",
            base.display()
        ));
    }
    let Some(target) = safe_join(&base, &folder) else {
        return ActionResult::fail("Pasta de destino insegura ou nome reservado do Windows.");
    };
    let ok = |already: bool, msg: String| ActionResult {
        success: true,
        already_exists: already,
        message: msg,
        folder_name: Some(folder.clone()),
        folder_path: Some(target.display().to_string()),
    };
    if target.exists() {
        let repo_id = url
            .trim_end_matches('/')
            .trim_start_matches("https://github.com/");
        let repo_id = repo_id.strip_suffix(".git").unwrap_or(repo_id);
        let same_repo = std::fs::read_to_string(target.join(".git/config"))
            .is_ok_and(|config| remote_matches(&config, repo_id));
        let msg = if same_repo {
            format!("O repositório já existe na pasta: {folder}")
        } else {
            format!("A pasta {folder} já existe na sua máquina.")
        };
        return ok(true, msg);
    }
    let git_url = format!(
        "{}.git",
        url.trim_end_matches('/')
            .strip_suffix(".git")
            .unwrap_or(url.trim_end_matches('/'))
    );
    let has_lfs = crate::stack::git_lfs_available();
    let mut cmd = crate::stack::git_with_lfs_path();
    if !has_lfs {
        // Sem git-lfs instalado o filtro global quebra o checkout; clona com arquivos LFS como ponteiros
        cmd.args([
            "-c",
            "filter.lfs.process=",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
        ]);
    }
    cmd.args(["clone", "-q", "--depth", "1", "--", &git_url])
        .arg(&folder)
        .current_dir(&base);
    match run_with_timeout(&mut cmd, Duration::from_secs(300)) {
        Ok((true, _)) => {
            spawn_codebase_index(&target);
            let lfs_note = if !has_lfs && crate::stack::has_lfs_pointers(&target) {
                " (arquivos Git LFS ficaram como ponteiros: instale com `winget install GitHub.GitLFS` e rode `git lfs pull` na pasta)"
            } else {
                ""
            };
            ok(false, format!("Clonado em: {}{lfs_note}", target.display()))
        }
        Ok((false, err)) => ActionResult::fail(format!("Falha ao clonar: {err}")),
        Err(e) => ActionResult::fail(e),
    }
}

/// Dispara em background a indexacao automatica no codebase-memory-mcp (se instalado no sistema)
/// para que qualquer agente ja consiga consultar o grafo do repositorio recem-clonado.
fn spawn_codebase_index(repo_path: &Path) {
    if cfg!(test) {
        return;
    }
    let path_str = repo_path.display().to_string();
    std::thread::spawn(move || {
        let mut cmd = Command::new("codebase-memory-mcp");
        cmd.args(["index", &path_str])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.status();
    });
}

#[derive(Serialize, Clone)]
pub struct PullOutcome {
    pub folder: String,
    /// "updated" | "up_to_date" | "error"
    pub status: String,
    pub message: String,
    /// Commits novos puxados no pull (formato curto), quando houver atualizacao real
    pub changelog: Vec<String>,
}

fn drain_pipe<R: std::io::Read + Send + 'static>(mut pipe: R) -> String {
    let mut buf = [0u8; 8192];
    let mut message = Vec::new();
    while let Ok(n) = pipe.read(&mut buf) {
        if n == 0 {
            break;
        }
        if message.len() < 65536 {
            let remaining = 65536 - message.len();
            message.extend_from_slice(&buf[..n.min(remaining)]);
        }
    }
    String::from_utf8_lossy(&message).trim().to_string()
}

/// Roda `git pull --rebase` numa pasta ja baixada e resume o resultado.
pub fn pull_repo(dir: &str) -> PullOutcome {
    let base = crate::settings::github_dir();
    let fail = |status: &str, msg: String| PullOutcome {
        folder: dir.to_string(),
        status: status.into(),
        message: msg,
        changelog: Vec::new(),
    };
    let Some(target) = safe_join(&base, dir) else {
        return fail("error", "Pasta invalida.".into());
    };
    if !target.join(".git").is_dir() {
        return fail("error", "Nao e um repositorio git.".into());
    }
    let old_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut cmd = Command::new("git");
    cmd.args(["pull", "--rebase", "--autostash"])
        .current_dir(&target);
    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail("error", format!("git nao encontrado: {e}")),
    };
    let out_thread = child
        .stdout
        .take()
        .map(|p| std::thread::spawn(move || drain_pipe(p)));
    let err_thread = child
        .stderr
        .take()
        .map(|p| std::thread::spawn(move || drain_pipe(p)));
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_thread.map(|t| t.join().unwrap_or_default()).unwrap_or_default();
                let stderr = err_thread.map(|t| t.join().unwrap_or_default()).unwrap_or_default();
                if status.success() {
                    let summary = format!(
                        "{} {}",
                        stdout
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .take(4)
                            .collect::<Vec<_>>()
                            .join(" | "),
                        stderr
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .take(4)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    );
                    let text = format!("{stdout} {stderr}").to_lowercase();
                    if text.contains("already up to date") || text.contains("atualizado") {
                        return fail("up_to_date", "Ja estava em dia.".into());
                    }
                    let changelog = if !old_head.is_empty() {
                        Command::new("git")
                            .args(["log", &format!("{old_head}..HEAD"), "--oneline", "-n", "5"])
                            .current_dir(&target)
                            .creation_flags(CREATE_NO_WINDOW)
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .map(|o| {
                                String::from_utf8_lossy(&o.stdout)
                                    .lines()
                                    .map(|l| l.trim().to_string())
                                    .filter(|l| !l.is_empty())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let msg = if !changelog.is_empty() {
                        format!("{} novo(s): {}", changelog.len(), changelog.join(" · "))
                    } else {
                        summary.trim().to_string()
                    };
                    return PullOutcome {
                        folder: dir.to_string(),
                        status: "updated".into(),
                        message: msg,
                        changelog,
                    };
                }
                let err = if stderr.is_empty() { stdout } else { stderr };
                return fail("error", err);
            }
            Ok(None) => {}
            Err(e) => return fail("error", e.to_string()),
        }
        if started.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            return fail("error", "Tempo limite esgotado no pull.".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn open_folder(path: &str) -> ActionResult {
    let Ok(p) = PathBuf::from(path).canonicalize() else {
        return ActionResult::fail(format!("Pasta não encontrada: {path}"));
    };
    // So abre pastas dentro das 3 pastas configuradas (GitHub, Hugging Face, Skills)
    let allowed = [
        crate::settings::github_dir(),
        crate::settings::hf_dir(),
        crate::settings::skills_dir(),
    ]
    .iter()
    .filter_map(|b| b.canonicalize().ok())
    .any(|b| p.starts_with(&b));
    if !allowed || !p.is_dir() {
        return ActionResult::fail("Acesso negado: fora das pastas configuradas.");
    }
    match Command::new("explorer.exe").arg(&p).spawn() {
        Ok(_) => ActionResult {
            success: true,
            already_exists: false,
            message: format!("Pasta aberta: {}", path),
            folder_name: None,
            folder_path: Some(path.into()),
        },
        Err(e) => ActionResult::fail(format!("Erro ao abrir pasta: {e}")),
    }
}

/// Junta `base` + caminho relativo vindo da internet sem deixar escapar da pasta ("..", "C:", barras soltas).
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    if rel.starts_with(['/', '\\']) || has_reparse_ancestor(base) {
        return None;
    }
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2"
                        | "3"
                        | "4"
                        | "5"
                        | "6"
                        | "7"
                        | "8"
                        | "9"
                        | "\u{b9}"
                        | "\u{b2}"
                        | "\u{b3}"
                )
            })
        });
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || reserved
            || part
                .chars()
                .any(|c| r#"<>:"|?*"#.contains(c) || c.is_control())
        {
            return None;
        }
        out.push(part);
        if has_reparse_ancestor(&out) {
            return None;
        }
    }
    Some(out)
}

/// Reject existing symlinks/junctions in a destination's ancestry before writing.
pub fn has_reparse_ancestor(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    path.ancestors()
        .any(|p| match std::fs::symlink_metadata(p) {
            Ok(m) => m.file_attributes() & 0x400 != 0,
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        })
}

/// Links externos abrem no navegador padrao; so http/https, nunca arquivos locais.
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("URL não permitida".into());
    }
    Command::new("explorer.exe")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_junction_ancestors() {
        let root = std::env::temp_dir().join(format!("gtr_junction_{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let link = root.join("link");
        let status = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .stdout(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .unwrap();
        assert!(status.success(), "cannot create junction test fixture");
        assert!(safe_join(&root, "link/escape.txt").is_none());
        assert!(safe_join(&link, "escape.txt").is_none());
        std::fs::remove_dir(&link).unwrap();
        std::fs::remove_dir(&outside).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    #[test]
    fn names_and_guards() {
        assert_eq!(pretty_name("agent-zero"), "Agent Zero");
        assert_eq!(pretty_name("fastapi"), "FastAPI");
        assert_eq!(sanitize(".."), "repositorio");
        assert_eq!(sanitize(r"..\..\Windows"), "Windows");
        assert!(name_matches("ArtCraft", "artcraft"));
        assert!(name_matches("Agent Zero", "agent_zero"));
        assert!(!name_matches("ArtCraft", "art"));
        assert!(!clone_repo("--upload-pack=calc.exe", "x", None).success);
        assert!(!clone_repo("https://evil.com/a/b", "x", None).success);
        assert!(!open_folder(r"C:\Windows").success);
        assert!(open_url("file:///C:/Windows/System32/calc.exe").is_err());
        let b = Path::new(r"E:\X");
        assert_eq!(
            safe_join(b, "skills/a/SKILL.md"),
            Some(PathBuf::from(r"E:\X\skills\a\SKILL.md"))
        );
        assert!(safe_join(b, "../evil.txt").is_none());
        assert!(safe_join(b, r"C:\Windows\x").is_none());
        assert!(safe_join(b, "").is_none());
        for invalid in [
            "/absolute",
            "a//b",
            "a/CON.txt",
            "LPT1",
            "COM\u{b9}.txt",
            "a. /b",
            "x.",
            "file:stream",
            "x/../y",
        ] {
            assert!(safe_join(b, invalid).is_none(), "{invalid}");
        }
        assert!(remote_matches(
            "url = https://github.com/a/repo.git",
            "a/repo"
        ));
        assert!(!remote_matches(
            "url = https://github.com/a/repository.git",
            "a/repo"
        ));
        assert!(!remote_matches(
            "url = https://evil.test/github.com/a/repo",
            "a/repo"
        ));
        let test_dirs = vec!["MCP".to_string(), "Other".to_string()];
        assert_eq!(
            find_downloaded(&test_dirs, "BrowserMCP/mcp", "mcp"),
            Some(&"MCP".to_string())
        );
    }

    /// Fixture local com repo bare: valida updated -> up_to_date -> erro sem rede.
    #[test]
    fn pull_repo_updates_and_detects_up_to_date() {
        let root = std::env::temp_dir().join(format!("gtr_pull_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let remote = root.join("remote.git");
        let local = root.join("repo");
        let other = root.join("other");
        let notgit = root.join("notgit");
        std::fs::create_dir_all(&notgit).unwrap();
        let git = |args: &[&str], cwd: &Path| {
            let out = Command::new("git")
                .args(args)
                .current_dir(cwd)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {:?} falhou: {}",
                args,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        };
        git(&["init", "--bare", "--quiet", "remote.git"], &root);
        let remote_str = remote.display().to_string();
        std::fs::create_dir_all(&local).unwrap();
        git(&["init", "--quiet"], &local);
        git(&["config", "user.email", "t@t.t"], &local);
        git(&["config", "user.name", "t"], &local);
        std::fs::write(local.join("a.txt"), "1").unwrap();
        git(&["add", "."], &local);
        git(&["commit", "--quiet", "-m", "inicial"], &local);
        git(&["remote", "add", "origin", &remote_str], &local);
        let branch = {
            let out = Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&local)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["push", "--quiet", "-u", "origin", &branch], &local);
        git(&["clone", "--quiet", &remote_str, "other"], &root);
        git(&["config", "user.email", "t@t.t"], &other);
        git(&["config", "user.name", "t"], &other);
        std::fs::write(other.join("b.txt"), "2").unwrap();
        git(&["add", "."], &other);
        git(&["commit", "--quiet", "-m", "segundo"], &other);
        git(&["push", "--quiet", "origin", &branch], &other);

        crate::settings::init(root.join("settings.json"));
        let d = root.display().to_string();
        crate::settings::save(&d, &d, &d, None).unwrap();

        let first = pull_repo("repo");
        assert_eq!(first.status, "updated", "mensagem: {}", first.message);
        let second = pull_repo("repo");
        assert_eq!(second.status, "up_to_date", "mensagem: {}", second.message);
        let in_sync = pull_repo("other");
        assert_eq!(in_sync.status, "up_to_date", "mensagem: {}", in_sync.message);
        let missing = pull_repo("nao-existe");
        assert_eq!(missing.status, "error");
        let not_a_repo = pull_repo("notgit");
        assert_eq!(not_a_repo.status, "error");

        let _ = std::fs::remove_dir_all(&root);
    }
}
