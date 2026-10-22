//! Coleta e busca de datasets publicos do Hugging Face.
use crate::models::{self, HfModel, ModelPage};
use reqwest::blocking::Client;
use std::path::PathBuf;

/// Detecta a modalidade e categoria do dataset a partir de tags.
fn detect_dataset_group(tags: &[String]) -> (&'static str, &'static str) {
    for tag in tags {
        let t = tag.to_lowercase();
        if t.starts_with("modality:tabular")
            || t == "tabular"
            || t.contains("parquet")
            || t.contains("csv")
            || t.contains("structured")
        {
            return ("Tabular", "tabular");
        }
        if t.starts_with("modality:text")
            || t.starts_with("task_categories:text-generation")
            || t.starts_with("task_categories:summarization")
            || t.starts_with("task_categories:translation")
            || t == "nlp"
            || t.contains("instruct")
            || t.contains("conversational")
        {
            return ("Texto / NLP", "texto");
        }
        if t.starts_with("modality:image")
            || t.starts_with("task_categories:text-to-image")
            || t.starts_with("task_categories:image-classification")
            || t.contains("vision")
            || t.contains("multimodal")
        {
            return ("Visão / Imagem", "visao");
        }
        if t.starts_with("modality:audio")
            || t.starts_with("task_categories:speech")
            || t.starts_with("task_categories:audio")
            || t.contains("voice")
            || t.contains("tts")
        {
            return ("Áudio / Voz", "audio");
        }
        if t.contains("benchmark")
            || t.contains("evaluation")
            || t.contains("eval")
            || t.contains("leaderboard")
        {
            return ("Benchmark", "benchmark");
        }
    }
    ("Dataset", "geral")
}

/// Limpa e extrai uma descricao legivel a partir do payload bruto do Hugging Face.
fn extract_clean_description(raw_desc: &str, name: &str, task_label: &str) -> String {
    let meaningful_lines: Vec<String> = raw_desc
        .lines()
        .map(|l| {
            l.trim()
                .trim_matches('\t')
                .trim_start_matches('#')
                .trim()
                .to_string()
        })
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("http")
                && !l.starts_with("<")
                && !l.starts_with("|")
                && !l.starts_with("---")
        })
        .collect();

    if meaningful_lines.is_empty() {
        return format!("{name}; modalidade: {task_label}");
    }

    let mut combined = String::new();
    for line in meaningful_lines.iter().take(4) {
        if combined.len() + line.len() > 300 {
            if combined.is_empty() {
                combined = line.chars().take(300).collect();
            }
            break;
        }
        if !combined.is_empty() {
            combined.push(' ');
        }
        combined.push_str(line);
    }

    if combined.trim().is_empty() {
        format!("{name}; modalidade: {task_label}")
    } else {
        combined.trim().to_string()
    }
}

pub fn parse(json: &serde_json::Value, today: chrono::NaiveDate) -> Vec<HfModel> {
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let (author, name) = id
                .split_once('/')
                .map(|(a, n)| (a.to_string(), n.to_string()))
                .unwrap_or(("".into(), id.clone()));
            let tags: Vec<String> = m["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect();
            let (task_label, group) = detect_dataset_group(&tags);
            let created_at = m["createdAt"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|date| date.with_timezone(&chrono::Local).date_naive().to_string())
                .unwrap_or_default();
            let is_new = chrono::NaiveDate::parse_from_str(&created_at, "%Y-%m-%d")
                .is_ok_and(|d| (0..=14).contains(&(today - d).num_days()));
            let dl = find_downloaded(&id);
            let raw_desc = m["description"].as_str().unwrap_or_default();
            let description = extract_clean_description(raw_desc, &name, task_label);
            Some(HfModel {
                url: format!("https://huggingface.co/datasets/{id}"),
                likes: m["likes"].as_i64().unwrap_or(0),
                downloads: m["downloads"].as_i64().unwrap_or(0),
                trending_score: m["trendingScore"].as_f64().unwrap_or(0.0).round() as i64,
                id,
                author,
                name,
                task: "dataset".into(),
                task_label: task_label.into(),
                group: group.to_string(),
                created_at,
                is_new,
                summary: Some(description),
                is_downloaded: dl.is_some(),
                downloaded_path: dl.map(|p| p.display().to_string()),
            })
        })
        .collect()
}

/// Busca datasets publicos no Hugging Face com ordenacao e paginacao por cursor.
pub fn search(
    http: &Client,
    query: &str,
    sort: &str,
    cursor: Option<&str>,
) -> Result<ModelPage, String> {
    let sort = match sort {
        "downloads" | "likes" | "createdAt" | "trendingScore" => sort,
        _ => "trendingScore",
    };
    let mut req = http.get("https://huggingface.co/api/datasets").query(&[
        ("search", query.trim()),
        ("sort", sort),
        ("direction", "-1"),
        ("limit", "50"),
    ]);
    if let Some(c) = cursor.filter(|c| !c.is_empty()) {
        req = req.query(&[("cursor", c)]);
    }
    let resp = req
        .send()
        .map_err(|e| format!("Hugging Face Datasets indisponível: {e}"))?;
    let next_cursor = resp
        .headers()
        .get("link")
        .and_then(|l| l.to_str().ok())
        .and_then(models::next_cursor);
    let json: serde_json::Value = resp
        .error_for_status()
        .and_then(|r| r.json())
        .map_err(|e| format!("Hugging Face Datasets respondeu com erro: {e}"))?;
    Ok(ModelPage {
        models: parse(&json, chrono::Local::now().date_naive()),
        next_cursor,
    })
}

pub fn find_downloaded(id: &str) -> Option<PathBuf> {
    let sanitized = crate::local::sanitize(&id.replace('/', "__"));
    // Procura na subpasta datasets ou na raiz de E:\REPOSITORIOS\HuggingFace
    let hf_datasets = crate::settings::hf_dir().join("datasets").join(&sanitized);
    if hf_datasets.is_dir() {
        return Some(hf_datasets);
    }
    if let Some(folder) = crate::local::safe_join(&crate::settings::hf_dir(), &sanitized) {
        if folder.is_dir() {
            return Some(folder);
        }
    }
    // Procura na subpasta datasets em AI_MODELS
    let ai_datasets = PathBuf::from(r"E:\REPOSITORIOS\AI_MODELS\datasets").join(&sanitized);
    if ai_datasets.is_dir() {
        return Some(ai_datasets);
    }
    let hub_name = format!("datasets--{}", id.replace('/', "--"));
    let hub_path = PathBuf::from(r"E:\REPOSITORIOS\AI_MODELS").join(&hub_name);
    if hub_path.is_dir() {
        return Some(hub_path);
    }
    None
}

pub fn list_downloaded() -> Vec<HfModel> {
    let mut list = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let check_dir = |dir: PathBuf, list: &mut Vec<HfModel>, seen: &mut std::collections::HashSet<String>| {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let id = if let Some(stripped) = name.strip_prefix("datasets--") {
                    stripped.replace("--", "/")
                } else if name.contains("__") {
                    name.replace("__", "/")
                } else {
                    continue;
                };
                if seen.insert(id.clone()) {
                    let (author, model_name) = id
                        .split_once('/')
                        .map(|(a, n)| (a.to_string(), n.to_string()))
                        .unwrap_or(("".into(), id.clone()));
                    list.push(HfModel {
                        id: id.clone(),
                        author,
                        name: model_name,
                        task: "dataset".into(),
                        task_label: "Dataset Local".into(),
                        group: "geral".into(),
                        likes: 0,
                        downloads: 0,
                        trending_score: 0,
                        created_at: String::new(),
                        is_new: false,
                        url: format!("https://huggingface.co/datasets/{id}"),
                        summary: None,
                        is_downloaded: true,
                        downloaded_path: Some(path.display().to_string()),
                    });
                }
            }
        }
    };

    check_dir(crate::settings::hf_dir().join("datasets"), &mut list, &mut seen_ids);
    check_dir(PathBuf::from(r"E:\REPOSITORIOS\AI_MODELS\datasets"), &mut list, &mut seen_ids);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_description_handles_empty_and_messy_payloads() {
        let messy = "\n\t\n\t\t\n\t\n\t\n\t\tFDE-Gym: The Last Mile of AI Delivery\n\t\n\nFDE-Gym tests whether an agent can do the work of a forward-deployed engineer.";
        let res = extract_clean_description(messy, "FDE-Gym", "Benchmark");
        assert!(res.contains("FDE-Gym: The Last Mile"));
        assert!(res.contains("forward-deployed engineer"));

        let empty = "   \n\t\t\n   ";
        let res_empty = extract_clean_description(empty, "test-data", "Tabular");
        assert_eq!(res_empty, "test-data; modalidade: Tabular");
    }

    #[test]
    fn parses_datasets_payload() {
        let json = serde_json::json!([
            {
                "id": "datasocial/tiktok-5.6B-videos",
                "likes": 353,
                "downloads": 22810,
                "trendingScore": 105,
                "tags": ["modality:tabular", "parquet"],
                "createdAt": "2026-10-03T11:14:37.000Z",
                "description": "\n\t\n\t\tContact\nTelegram: @hashfunction\nTikTok scraper source code"
            }
        ]);
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let parsed = parse(&json, today);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "tiktok-5.6B-videos");
        assert_eq!(parsed[0].group, "tabular");
        assert_eq!(parsed[0].task_label, "Tabular");
        assert!(parsed[0].summary.is_some());
        assert!(parsed[0].url.contains("huggingface.co/datasets/"));
    }
}
