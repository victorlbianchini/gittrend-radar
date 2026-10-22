//! Opt-in network roundtrip. Run alone because settings are process-global:
//! cargo test live_download_roundtrip -- --ignored --nocapture --test-threads=1
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let Ok(temp) = std::env::temp_dir().canonicalize() else {
            return;
        };
        let Ok(root) = self.0.canonicalize() else {
            return;
        };
        if root == temp || !root.starts_with(&temp) || crate::local::has_reparse_ancestor(&root) {
            eprintln!("Cleanup refused for {}", root.display());
            return;
        }
        // A timed-out background transfer may still be writing. Preserve its evidence.
        if crate::downloads::list()
            .iter()
            .any(|job| !job.finished && Path::new(&job.folder_path).starts_with(&self.0))
        {
            eprintln!(
                "Background download remains active; preserving {}",
                root.display()
            );
            return;
        }
        if let Err(error) = std::fs::remove_dir_all(&root) {
            eprintln!(
                "Could not clean smoke artifacts {}: {error}",
                root.display()
            );
        }
    }
}

fn wait_for_job(id: &str) -> crate::downloads::Job {
    let started = Instant::now();
    loop {
        if let Some(job) = crate::downloads::list()
            .into_iter()
            .find(|job| job.id == id)
        {
            if job.finished {
                assert!(job.success, "{id} failed: {}", job.message);
                return job;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "Timed out waiting for {id}; jobs: {:?}",
            crate::downloads::list()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn assert_no_weights(path: &Path) {
    for entry in std::fs::read_dir(path).expect("read isolated HF directory") {
        let path = entry.expect("HF directory entry").path();
        if path.is_dir() {
            assert_no_weights(&path);
        } else {
            assert!(
                !crate::models::is_weight(&path.to_string_lossy()),
                "Light download unexpectedly saved weights: {}",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "real GitHub, Hugging Face and translation network; run alone with --test-threads=1"]
fn live_download_roundtrip() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "gtr_catalog_smoke_{}_{}",
        std::process::id(),
        unique
    ));
    std::fs::create_dir(&root).expect("create unique smoke root");
    let scratch = Scratch(root.canonicalize().expect("canonical smoke root"));
    let github = scratch.0.join("github");
    let hf = scratch.0.join("hf");
    let skills = scratch.0.join("skills");
    crate::settings::init(scratch.0.join("settings.json"));
    crate::settings::save(
        &github.display().to_string(),
        &hf.display().to_string(),
        &skills.display().to_string(),
        Some(String::new()),
    )
    .expect("save isolated settings (no production token)");
    assert_eq!(crate::settings::github_dir(), github);
    assert_eq!(crate::settings::hf_dir(), hf);
    assert_eq!(crate::settings::skills_dir(), skills);

    let clone = crate::local::clone_repo(
        "https://github.com/octocat/Hello-World",
        "Hello-World",
        None,
    );
    assert!(clone.success, "GitHub clone failed: {}", clone.message);
    let clone_path = PathBuf::from(clone.folder_path.expect("clone destination"));
    assert!(clone_path.starts_with(&github), "clone escaped smoke root");
    assert!(clone_path.join(".git").is_dir());
    assert!(
        !std::fs::read_to_string(clone_path.join("README"))
            .expect("read cloned README")
            .trim()
            .is_empty()
    );
    let dirs = crate::local::existing_dirs();
    assert!(crate::local::find_downloaded(&dirs, "octocat/Hello-World", "Hello-World").is_some());
    println!(
        "GitHub: clone, README and remote identity verified in {}",
        clone_path.display()
    );

    let http = crate::github::client();
    let model = "hf-internal-testing/tiny-random-bert";
    crate::models::download(&http, model, false).expect("start HF light download");
    let model_job = wait_for_job(&format!("hf:{model}"));
    let model_path = PathBuf::from(model_job.folder_path);
    assert!(model_path.starts_with(&hf), "model escaped smoke root");
    // This public fixture has no README; verify its actual configuration instead.
    let config = std::fs::read_to_string(model_path.join("config.json"))
        .expect("read downloaded HF config.json");
    let config: serde_json::Value = serde_json::from_str(&config).expect("HF config must be JSON");
    assert_eq!(config["model_type"], "bert");
    assert_no_weights(&model_path);
    assert!(!model_path.join("pytorch_model.bin").exists());
    println!(
        "Hugging Face: light configuration downloaded; weights absent in {}",
        model_path.display()
    );
    let before = std::fs::read(model_path.join("config.json")).unwrap();
    let info = crate::models::files(&http, model).unwrap();
    assert!(
        info.full_bytes < 40 * 1024 * 1024,
        "fixture unexpectedly large"
    );
    crate::models::download(&http, model, true).expect("start HF full download");
    wait_for_job(&format!("hf:{model}"));
    assert!(model_path.join("model.safetensors").is_file());
    assert_eq!(
        std::fs::read(model_path.join("config.json")).unwrap(),
        before
    );
    println!("Hugging Face: full weights downloaded; existing config preserved");

    let conn = crate::db::open(&scratch.0.join("smoke.db")).expect("isolated SQLite");
    let skill = crate::skills::Skill {
        id: "anthropics/skills::theme-factory".into(),
        name: "theme-factory".into(),
        repo: "anthropics/skills".into(),
        branch: "HEAD".into(),
        path: "skills/theme-factory".into(),
        ..Default::default()
    };
    let (skill, explanation) =
        crate::skills::explain(&http, &conn, skill).expect("explain public skill from SKILL.md");
    assert!(!explanation.preview.trim().is_empty());
    assert!(explanation.files.iter().any(|name| name == "SKILL.md"));
    let expected_files = explanation.files.clone();
    let skill_id = skill.id.clone();
    let download = crate::skills::download(&http, skill).expect("download skill folder");
    assert!(
        download.success,
        "Skill download failed: {}",
        download.message
    );
    let skill_job = wait_for_job(&format!("skill:{skill_id}"));
    let skill_path = PathBuf::from(skill_job.folder_path);
    assert!(skill_path.starts_with(&skills), "skill escaped smoke root");
    let markdown =
        std::fs::read_to_string(skill_path.join("SKILL.md")).expect("read downloaded SKILL.md");
    assert!(!markdown.trim().is_empty());
    for file in &expected_files {
        assert!(
            skill_path.join(file).is_file(),
            "Missing skill artifact {file}"
        );
    }
    println!(
        "Skills: SKILL.md explained and {} artifacts downloaded in {}; no skill content executed",
        expected_files.len(),
        skill_path.display()
    );
    drop(conn);
    drop(scratch);
}
