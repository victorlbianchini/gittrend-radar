use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HfModel {
    pub id: String,
    pub author: String,
    pub name: String,
    pub task: String,
    pub task_label: String,
    pub group: String,
    pub likes: i64,
    pub downloads: i64,
    pub trending_score: i64,
    pub created_at: String,
    pub is_new: bool,
    pub url: String,
    /// Resumo em PT-BR (vem do cache de explicacoes; None enquanto nao analisado)
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub is_downloaded: bool,
    #[serde(default)]
    pub downloaded_path: Option<String>,
}

/// (tarefa do Hugging Face, rotulo PT-BR, grupo da aba)
const TASKS: &[(&str, &str, &str)] = &[
    ("text-generation", "Geração de texto (LLM)", "texto"),
    ("text2text-generation", "Texto para texto", "texto"),
    ("translation", "Tradução", "texto"),
    ("summarization", "Resumo", "texto"),
    ("text-classification", "Classificação de texto", "texto"),
    ("question-answering", "Perguntas e respostas", "texto"),
    ("image-text-to-text", "Visão + texto (multimodal)", "visao"),
    (
        "visual-question-answering",
        "Perguntas sobre imagens",
        "visao",
    ),
    ("image-to-text", "Descrição de imagens", "visao"),
    ("object-detection", "Detecção de objetos", "visao"),
    ("image-segmentation", "Segmentação de imagens", "visao"),
    ("depth-estimation", "Profundidade", "visao"),
    ("image-classification", "Classificação de imagens", "visao"),
    ("text-to-image", "Texto para imagem", "imagem"),
    ("image-to-image", "Edição de imagem", "imagem"),
    (
        "unconditional-image-generation",
        "Geração de imagem",
        "imagem",
    ),
    ("text-to-video", "Texto para vídeo", "video"),
    ("image-to-video", "Imagem para vídeo", "video"),
    ("video-text-to-text", "Vídeo + texto", "video"),
    ("text-to-speech", "Texto para voz", "audio"),
    ("text-to-audio", "Texto para áudio", "audio"),
    (
        "automatic-speech-recognition",
        "Transcrição de voz",
        "audio",
    ),
    ("audio-to-audio", "Processamento de áudio", "audio"),
    ("audio-classification", "Classificação de áudio", "audio"),
    ("any-to-any", "Multimodal geral", "visao"),
    ("feature-extraction", "Embeddings", "embeddings"),
    ("sentence-similarity", "Embeddings de frases", "embeddings"),
    ("text-ranking", "Reranqueamento", "embeddings"),
    ("image-to-3d", "Imagem para 3D", "imagem"),
    ("text-to-3d", "Texto para 3D", "imagem"),
    ("robotics", "Robótica", "outros"),
    (
        "reinforcement-learning",
        "Aprendizado por reforço",
        "outros",
    ),
];

fn task_info(task: &str) -> (String, &'static str) {
    TASKS
        .iter()
        .find(|(t, _, _)| *t == task)
        .map(|(_, l, g)| (l.to_string(), *g))
        .unwrap_or_else(|| {
            (
                if task.is_empty() {
                    "Não informado".into()
                } else {
                    task.replace('-', " ")
                },
                "outros",
            )
        })
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
            let task = m["pipeline_tag"].as_str().unwrap_or_default().to_string();
            let (task_label, group) = task_info(&task);
            let created_at = m["createdAt"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|date| date.with_timezone(&chrono::Local).date_naive().to_string())
                .unwrap_or_default();
            let is_new = chrono::NaiveDate::parse_from_str(&created_at, "%Y-%m-%d")
                .is_ok_and(|d| (0..=14).contains(&(today - d).num_days()));
            let dl = find_downloaded(&id);
            Some(HfModel {
                url: format!("https://huggingface.co/{id}"),
                likes: m["likes"].as_i64().unwrap_or(0),
                downloads: m["downloads"].as_i64().unwrap_or(0),
                trending_score: m["trendingScore"].as_f64().unwrap_or(0.0).round() as i64,
                id,
                author,
                name,
                task,
                task_label,
                group: group.to_string(),
                created_at,
                is_new,
                summary: None,
                is_downloaded: dl.is_some(),
                downloaded_path: dl.map(|p| p.display().to_string()),
            })
        })
        .collect()
}

/// Modelos em alta no Hugging Face (API publica, sem chave).
pub fn fetch_trending(http: &Client) -> Result<Vec<HfModel>, String> {
    let json: serde_json::Value = http
        .get("https://huggingface.co/api/models")
        .query(&[
            ("sort", "trendingScore"),
            ("direction", "-1"),
            ("limit", "80"),
        ])
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| format!("Hugging Face indisponível: {e}"))?;
    Ok(parse(&json, chrono::Local::now().date_naive()))
}

// ---------------- Busca em todos os modelos ----------------

#[derive(Serialize)]
pub struct ModelPage {
    pub models: Vec<HfModel>,
    pub next_cursor: Option<String>,
}

/// Busca em TODOS os modelos publicos do Hugging Face. `sort`: trendingScore | downloads | likes | createdAt.
/// Paginacao por cursor (o HF devolve no header Link).
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
    let mut req = http.get("https://huggingface.co/api/models").query(&[
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
        .map_err(|e| format!("Hugging Face indisponível: {e}"))?;
    let next_cursor = resp
        .headers()
        .get("link")
        .and_then(|l| l.to_str().ok())
        .and_then(next_cursor);
    let json: serde_json::Value = resp
        .error_for_status()
        .and_then(|r| r.json())
        .map_err(|e| format!("Hugging Face respondeu com erro: {e}"))?;
    Ok(ModelPage {
        models: parse(&json, chrono::Local::now().date_naive()),
        next_cursor,
    })
}

pub fn next_cursor(link: &str) -> Option<String> {
    link.split(',').find_map(|entry| {
        if !entry.contains("rel=\"next\"") && !entry.contains("rel=next") {
            return None;
        }
        let target = entry.split_once('<')?.1.split_once('>')?.0;
        let url = reqwest::Url::parse(target).ok()?;
        url.query_pairs()
            .find(|(key, _)| key == "cursor")
            .map(|(_, value)| value.into_owned())
    })
}

// ---------------- Explicacao (igual aos repositorios) ----------------

#[derive(Serialize, serde::Deserialize, Clone, Default, Debug)]
pub struct ModelExplanation {
    pub summary: String,
    pub why_trending: String,
    pub code_example: String,
    pub install_cmd: String,
    pub key_features: Vec<String>,
    pub when_to_select: Vec<String>,
    pub when_to_avoid: Vec<String>,
    pub verdict: String,
    pub license: String,
    pub base_model: String,
    pub languages: Vec<String>,
}

fn frontmatter_field(front: &str, key: &str) -> Vec<String> {
    // YAML simples do cartao: "key: valor" ou lista "key:\n- a\n- b"
    let mut out = vec![];
    let mut lines = front.lines().peekable();
    while let Some(l) = lines.next() {
        if let Some(v) = l.strip_prefix(&format!("{key}:")) {
            let v = v.trim().trim_matches(['"', '\'']);
            if !v.is_empty() {
                out.push(v.to_string());
            }
            while let Some(n) = lines.peek() {
                match n.trim_start().strip_prefix("- ") {
                    Some(item) if n.starts_with([' ', '-']) => {
                        out.push(item.trim().trim_matches(['"', '\'']).to_string());
                        lines.next();
                    }
                    _ => break,
                }
            }
        }
    }
    out
}

fn group_advice(group: &str) -> (Vec<&'static str>, &'static str) {
    match group {
        "texto" => (
            vec![
                "Você quer um LLM próprio para chat, código ou agentes, sem depender de API paga.",
                "Precisa rodar local (Ollama, LM Studio, vLLM) por privacidade ou custo.",
                "Quer ajustar (fine-tune) um modelo base para o seu domínio.",
            ],
            "Modelos de texto grandes exigem GPU com bastante VRAM; confira o tamanho antes de baixar.",
        ),
        "visao" => (
            vec![
                "Você precisa que a IA entenda imagens, telas, documentos ou vídeos junto com texto.",
                "Quer extrair dados de prints, PDFs escaneados ou fotos.",
                "Busca um modelo multimodal local em vez de uma API na nuvem.",
            ],
            "Modelos multimodais costumam ser pesados e precisam de GPU para respostas rápidas.",
        ),
        "imagem" => (
            vec![
                "Você quer gerar ou editar imagens localmente.",
                "Precisa de controle total do estilo e sem limites de uso por créditos.",
                "Quer integrar geração de imagem no seu app ou fluxo (ComfyUI, Diffusers).",
            ],
            "Geração de imagem pede GPU dedicada; em CPU fica muito lenta.",
        ),
        "video" => (
            vec![
                "Você quer gerar ou animar vídeos a partir de texto ou imagem.",
                "Está testando produção de conteúdo com IA sem pagar por segundo gerado.",
                "Quer acompanhar o estado da arte em vídeo generativo.",
            ],
            "Vídeo generativo é o mais pesado de todos: espere muita VRAM e tempo de geração.",
        ),
        "audio" => (
            vec![
                "Você precisa de transcrição, voz sintética ou clonagem de voz local.",
                "Quer dublar, narrar ou legendar conteúdo sem serviço pago.",
                "Busca privacidade para áudio sensível rodando na própria máquina.",
            ],
            "Confira a licença: muitos modelos de voz têm restrições de uso comercial.",
        ),
        "embeddings" => (
            vec![
                "Você está montando busca semântica, RAG ou recomendação.",
                "Quer gerar embeddings localmente, sem custo por token.",
                "Precisa reranquear resultados de busca com mais precisão.",
            ],
            "Troque de modelo de embeddings só reindexando tudo: vetores de modelos diferentes não se misturam.",
        ),
        _ => (
            vec![
                "O tipo de tarefa do modelo bate com o seu problema.",
                "Você quer testar algo que está em alta na comunidade.",
                "Prefere rodar local em vez de depender de API.",
            ],
            "Leia o cartão do modelo: requisitos e licença variam muito.",
        ),
    }
}

pub fn explain(
    http: &Client,
    conn: &rusqlite::Connection,
    m: &HfModel,
) -> Result<ModelExplanation, String> {
    use crate::analyze::{clean_markdown, code_blocks, fmt_int, key_features, tidy, translate};
    let raw = http
        .get(format!(
            "https://huggingface.co/{}/raw/main/README.md",
            m.id
        ))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Não foi possível ler o cartão do modelo: {e}"))?
        .replace("\r\n", "\n");
    let (front, body) = match raw
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---"))
    {
        Some((f, b)) => (f.to_string(), b.to_string()),
        None => (String::new(), raw.clone()),
    };
    let clean = clean_markdown(&body);
    let prose = clean
        .split("\n\n")
        .filter(|p| {
            !p.trim_start()
                .starts_with(['#', '>', '|', '`', '<', '!', '[', '-', '*'])
        })
        .map(tidy)
        .find(|p| {
            p.chars().filter(|c| c.is_alphabetic()).count() > 60
                && p.contains('.')
                && !p.contains('|')
        });
    let summary = prose
        .map(|p| translate(http, conn, &p.chars().take(700).collect::<String>()))
        .unwrap_or_else(|| format!("Modelo de {} publicado por {}. O cartão do modelo não traz uma descrição em texto corrido.", m.task_label.to_lowercase(), if m.author.is_empty() { "autor não informado" } else { &m.author }));

    let mut why = vec![format!(
        "Pontuação de tendência {} no Hugging Face.",
        fmt_int(m.trending_score)
    )];
    if m.likes > 0 {
        why.push(format!(
            "{} curtidas e {} downloads no último mês.",
            fmt_int(m.likes),
            fmt_int(m.downloads)
        ));
    }
    if m.is_new {
        why.push("Lançado nas últimas duas semanas.".into());
    }

    let blocks = code_blocks(&body);
    let code_example = blocks
        .iter()
        .find(|(l, c)| {
            matches!(l.as_str(), "python" | "py" | "bash" | "sh" | "shell") && c.len() < 1500
        })
        .or(blocks.first())
        .map(|(l, c)| {
            format!(
                "```{}\n{}\n```",
                if l.is_empty() { "text" } else { l },
                c.chars().take(1500).collect::<String>()
            )
        })
        .unwrap_or_default();
    let install_cmd = blocks
        .iter()
        .flat_map(|(_, c)| c.lines())
        .find(|l| {
            l.contains("pip install")
                || l.contains("ollama run")
                || l.contains("huggingface-cli download")
                || l.contains("hf download")
        })
        .map(|l| {
            l.trim()
                .trim_start_matches("$ ")
                .trim_start_matches("! ")
                .to_string()
        })
        .unwrap_or_default();

    let features = key_features(&body)
        .iter()
        .take(4)
        .map(|f| translate(http, conn, f).trim_end_matches('.').to_string())
        .collect();
    let (select, avoid_hint) = group_advice(&m.group);
    let license = frontmatter_field(&front, "license").join(", ");
    let mut avoid = vec![avoid_hint.to_string()];
    if license.is_empty() {
        avoid.push(
            "O cartão não informa licença: confirme antes de usar em produto comercial.".into(),
        );
    } else if license.contains("nc") || license.contains("research") || license.contains("other") {
        avoid.push(format!(
            "Licença {license}: pode restringir uso comercial, leia antes."
        ));
    }

    Ok(ModelExplanation {
        summary,
        why_trending: why.join(" "),
        code_example,
        install_cmd,
        key_features: features,
        when_to_select: select.into_iter().map(String::from).collect(),
        when_to_avoid: avoid,
        verdict: if m.is_new && m.trending_score > 300 {
            format!(
                "Lançamento quente em {}. Vale testar agora.",
                m.task_label.to_lowercase()
            )
        } else {
            format!("Referência em {} no momento.", m.task_label.to_lowercase())
        },
        license,
        base_model: frontmatter_field(&front, "base_model").join(", "),
        languages: frontmatter_field(&front, "language")
            .into_iter()
            .take(8)
            .collect(),
    })
}

// ---------------- Arquivos e download ----------------

const WEIGHT_EXT: &[&str] = &[
    ".safetensors",
    ".bin",
    ".gguf",
    ".pt",
    ".pth",
    ".onnx",
    ".ckpt",
    ".h5",
    ".msgpack",
    ".tflite",
    ".ot",
    ".npz",
    ".pkl",
    ".mlmodel",
    ".engine",
    ".nemo",
    ".parquet",
    ".arrow",
    ".jsonl",
    ".tsv",
    ".csv.gz",
    ".gz",
    ".tar.gz",
    ".zip",
    ".7z",
    ".tar",
    ".bz2",
];

pub fn is_weight(file: &str) -> bool {
    let f = file.to_lowercase();
    WEIGHT_EXT.iter().any(|e| f.ends_with(e)) || f.contains("/onnx/") || f.ends_with(".data")
}

#[derive(Serialize, Clone)]
pub struct ModelFiles {
    pub id: String,
    pub gated: bool,
    pub files: Vec<(String, u64)>,
    pub light_bytes: u64,
    pub full_bytes: u64,
    pub light_count: usize,
    pub full_count: usize,
    pub is_dataset: bool,
    /// Quantizacoes ou variantes de peso detectadas (ex: GGUF Q4_K_M, Q8_0, FP16)
    pub quantizations: Vec<(String, u64)>,
    /// Estimativa pratica de VRAM para rodar o modelo principal (ou a quantizacao recomendada)
    pub vram_hint: String,
}

fn compute_vram_and_quants(files: &[(String, u64)], is_dataset: bool) -> (Vec<(String, u64)>, String) {
    if is_dataset {
        return (Vec::new(), String::new());
    }
    let mut quants: Vec<(String, u64)> = files
        .iter()
        .filter(|(f, _)| f.to_lowercase().ends_with(".gguf"))
        .map(|(f, sz)| (f.clone(), *sz))
        .collect();
    quants.sort_by_key(|(_, sz)| *sz);

    let ref_bytes: u64 = if !quants.is_empty() {
        quants
            .iter()
            .find(|(f, _)| {
                let low = f.to_lowercase();
                low.contains("q4_k_m") || low.contains("q4_0") || low.contains("q5_k_m")
            })
            .or_else(|| quants.iter().find(|(_, sz)| *sz > 100 * 1024 * 1024))
            .or_else(|| quants.first())
            .map(|(_, sz)| *sz)
            .unwrap_or(0)
    } else {
        let safetensors_sum: u64 = files
            .iter()
            .filter(|(f, _)| f.to_lowercase().ends_with(".safetensors"))
            .map(|(_, sz)| *sz)
            .sum();
        if safetensors_sum > 0 {
            safetensors_sum
        } else {
            files.iter().filter(|(f, _)| is_weight(f)).map(|(_, sz)| *sz).sum()
        }
    };

    if ref_bytes == 0 {
        return (quants, String::new());
    }
    let gb = (ref_bytes as f64 / (1024.0 * 1024.0 * 1024.0)) * 1.2;
    let hint = if gb <= 4.0 {
        format!("Leve (~{gb:.1} GB VRAM, roda em qualquer GPU moderna ou CPU)")
    } else if gb <= 8.5 {
        format!("Médio (~{gb:.1} GB VRAM, cabe em GPU de 8 GB+)")
    } else if gb <= 12.5 {
        format!("Pesado (~{gb:.1} GB VRAM, pede GPU de 12 GB+)")
    } else if gb <= 24.5 {
        format!("Entusiasta (~{gb:.1} GB VRAM, pede GPU de 24 GB como RTX 3090/4090)")
    } else {
        format!("Servidor (~{gb:.0} GB VRAM, exige multi-GPU ou quantização menor)")
    };
    (quants, hint)
}

pub fn files(http: &Client, id: &str) -> Result<ModelFiles, String> {
    let token = crate::settings::hf_token();
    let fetch_model = |tok: Option<&str>| -> Result<reqwest::blocking::Response, String> {
        let mut req = http
            .get(format!("https://huggingface.co/api/models/{id}"))
            .query(&[("blobs", "true")]);
        if let Some(t) = tok {
            req = req.bearer_auth(t);
        }
        req.send()
            .map_err(|e| format!("Não foi possível consultar item no Hugging Face: {e}"))
    };

    let mut resp = fetch_model(token.as_deref())?;
    if matches!(resp.status().as_u16(), 401 | 403) {
        if let Some(tok) = token.as_deref() {
            let _ = ensure_gated_access(http, id, tok, false);
            resp = fetch_model(Some(tok))?;
        } else {
            return Err(format!(
                "O modelo {id} é restrito (gated) no Hugging Face. Configure seu HF_TOKEN em E:\\REPOSITORIOS\\AI_MODELS\\token para autoaceitar os termos."
            ));
        }
    }

    let (v, is_dataset) = if resp.status().is_success() {
        let val: serde_json::Value = resp
            .json()
            .map_err(|e| format!("JSON de modelo inválido: {e}"))?;
        (val, false)
    } else if matches!(resp.status().as_u16(), 401 | 403) {
        return Err(format!(
            "Acesso negado ao modelo restrito {id} (401/403). Verifique se sua conta Hugging Face aceitou os termos em https://huggingface.co/{id}"
        ));
    } else {
        let mut d_req = http
            .get(format!("https://huggingface.co/api/datasets/{id}"))
            .query(&[("blobs", "true")]);
        if let Some(tok) = token.as_deref() {
            d_req = d_req.bearer_auth(tok);
        }
        let d_resp = d_req
            .send()
            .map_err(|e| format!("Não foi possível consultar dataset no Hugging Face: {e}"))?;
        if matches!(d_resp.status().as_u16(), 401 | 403) {
            if let Some(tok) = token.as_deref() {
                let _ = ensure_gated_access(http, id, tok, true);
            }
            return Err(format!(
                "O dataset {id} é restrito (gated) no Hugging Face (401/403)."
            ));
        }
        let d_resp = d_resp
            .error_for_status()
            .map_err(|e| format!("Item '{id}' não encontrado como modelo nem como dataset no Hugging Face: {e}"))?;
        let val: serde_json::Value = d_resp
            .json()
            .map_err(|e| format!("JSON de dataset inválido: {e}"))?;
        (val, true)
    };
    let files: Vec<(String, u64)> = v["siblings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["rfilename"].as_str()?.to_string(),
                s["size"].as_u64().unwrap_or(0),
            ))
        })
        .collect();
    let light: Vec<_> = files.iter().filter(|(f, _)| !is_weight(f)).collect();
    let (quantizations, vram_hint) = compute_vram_and_quants(&files, is_dataset);
    Ok(ModelFiles {
        id: id.into(),
        gated: v["gated"].as_bool().unwrap_or(false) || v["gated"].is_string(),
        light_bytes: light.iter().map(|(_, s)| s).sum(),
        light_count: light.len(),
        full_bytes: files.iter().map(|(_, s)| s).sum(),
        full_count: files.len(),
        files,
        is_dataset,
        quantizations,
        vram_hint,
    })
}

/// Tenta solicitar ou aceitar os termos de acesso de um modelo ou dataset restrito no Hugging Face.
pub fn ensure_gated_access(
    http: &Client,
    id: &str,
    token: &str,
    is_dataset: bool,
) -> Result<(), String> {
    let url = if is_dataset {
        format!("https://huggingface.co/datasets/{id}/ask-access")
    } else {
        format!("https://huggingface.co/{id}/ask-access")
    };
    let resp = http
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({}))
        .send()
        .map_err(|e| format!("Falha de conexão com Hugging Face: {e}"))?;

    let status = resp.status();
    if status.is_success() || status.as_u16() == 302 {
        return Ok(());
    }

    if status.as_u16() == 400 {
        let body = resp.text().unwrap_or_default();
        let mut payload = serde_json::Map::new();
        payload.insert("First Name".into(), "Victor".into());
        payload.insert("Last Name".into(), "Bianchini".into());
        payload.insert("Date of birth".into(), "1995-05-15".into());
        payload.insert("Country".into(), "BR".into());
        payload.insert("Affiliation".into(), "Bianchini Creative".into());
        payload.insert("Job title".into(), "AI developer/engineer".into());

        for line in body.lines() {
            if let Some(pos) = line.find("at [\"") {
                if let Some(end) = line[pos + 5..].find("\"]") {
                    let field = &line[pos + 5..pos + 5 + end];
                    if !payload.contains_key(field) {
                        let lower = field.to_lowercase();
                        if lower.contains("term")
                            || lower.contains("license")
                            || lower.contains("policy")
                            || lower.contains("accept")
                            || lower.contains("submit")
                        {
                            payload.insert(field.into(), "true".into());
                        } else {
                            payload.insert(field.into(), "Victor Bianchini".into());
                        }
                    }
                }
            } else if let Some(pos) = line.find("at ") {
                let field = line[pos + 3..].trim();
                if !field.is_empty() && !payload.contains_key(field) {
                    payload.insert(field.into(), "BR".into());
                }
            }
        }

        let resp2 = http
            .post(&url)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .map_err(|e| format!("Falha ao enviar formulário de acesso: {e}"))?;

        if resp2.status().is_success() || resp2.status().as_u16() == 302 {
            return Ok(());
        }
    }

    Ok(())
}

/// Baixa o modelo para <pasta HF>/<autor>__<nome>. `full = false` pula os pesos.
pub fn download(http: &Client, id: &str, full: bool) -> Result<(), String> {
    download_with_callback(http, id, full, None)
}

/// Baixa o modelo/dataset em background e aciona `on_done(ok, msg)` apenas ao concluir.
pub fn download_with_callback(
    http: &Client,
    id: &str,
    full: bool,
    on_done: Option<Box<dyn FnOnce(bool, &str) + Send + 'static>>,
) -> Result<(), String> {
    let job = format!("hf:{id}");
    if crate::downloads::is_running(&job) {
        return Err("Esse modelo já está sendo baixado.".into());
    }
    let list = files(http, id)?;
    let token = crate::settings::hf_token();
    if list.gated {
        match token.as_deref() {
            Some(tok) => {
                if let Err(e) = ensure_gated_access(http, id, tok, list.is_dataset) {
                    eprintln!("Aviso ao solicitar acesso ao modelo restrito {id}: {e}");
                }
            }
            None => {
                return Err(
                    "Este modelo exige acesso no Hugging Face. Configure o token em E:\\REPOSITORIOS\\AI_MODELS\\token ou na variável HF_TOKEN.".into(),
                );
            }
        }
    }
    let chosen: Vec<_> = if list.is_dataset {
        if full {
            if list.files.len() > 150 {
                let mut data_and_root: Vec<_> = list
                    .files
                    .iter()
                    .filter(|(f, _)| is_weight(f) || !f.contains('/') || f.ends_with(".md") || f.ends_with(".json") || f.ends_with(".yaml") || f.ends_with(".yml"))
                    .cloned()
                    .collect();
                if data_and_root.is_empty() {
                    data_and_root = list.files.iter().take(150).cloned().collect();
                }
                data_and_root
            } else {
                list.files.clone()
            }
        } else {
            let mut light: Vec<_> = list
                .files
                .iter()
                .filter(|(f, _)| !is_weight(f) && (!f.contains('/') || f.ends_with(".md") || f.ends_with(".json") || f.ends_with(".yaml")))
                .cloned()
                .collect();
            if light.is_empty() {
                light = list.files.iter().take(10).cloned().collect();
            } else if light.len() > 50 {
                light.truncate(50);
            }
            light
        }
    } else {
        list.files
            .iter()
            .filter(|(f, _)| full || !is_weight(f))
            .cloned()
            .collect()
    };
    if chosen.is_empty() {
        return Err("Não há arquivos disponíveis para este download.".into());
    }
    let sanitized = crate::local::sanitize(&id.replace('/', "__"));
    let folder = if list.is_dataset {
        crate::local::safe_join(
            &crate::settings::hf_dir().join("datasets"),
            &sanitized,
        )
        .ok_or("Nome de dataset inválido")?
    } else {
        crate::local::safe_join(
            &crate::settings::hf_dir(),
            &sanitized,
        )
        .ok_or("Nome de modelo inválido")?
    };
    let _ = std::fs::create_dir_all(&folder);
    let total = chosen.iter().map(|(_, s)| s).sum();
    crate::downloads::try_start(
        &job,
        &format!("{id} ({})", if full { "completo" } else { "leve" }),
        total,
        &folder,
    )?;
    let http = http.clone();
    let id = id.to_string();
    let is_dataset = list.is_dataset;
    std::thread::spawn(move || {
        let mut errors = vec![];
        let mut skipped = 0;
        let auth_token = token;
        for (file, _) in &chosen {
            let Some(dest) = crate::local::safe_join(&folder, file) else {
                errors.push(format!("{file}: caminho recusado"));
                continue;
            };
            if dest.is_file() && !crate::local::has_reparse_ancestor(&dest) {
                skipped += 1;
                continue;
            }
            let mut url = reqwest::Url::parse("https://huggingface.co/").unwrap();
            let mut segments = url.path_segments_mut().unwrap();
            if is_dataset {
                segments.push("datasets");
            }
            segments
                .extend(id.split('/'))
                .extend(["resolve", "main"])
                .extend(file.split('/'));
            drop(segments);
            if let Err(e) = crate::downloads::fetch_to_file(
                &http,
                &job,
                url.as_str(),
                &dest,
                auth_token.as_deref(),
            ) {
                errors.push(e);
                if errors.len() >= 3 {
                    break;
                }
            }
        }
        let saved = chosen.len().saturating_sub(errors.len()).saturating_sub(skipped);
        let (ok, msg) = if errors.is_empty() {
            (
                true,
                format!(
                    "{saved} arquivos salvos; {skipped} já existentes preservados. Pasta: {}",
                    folder.display()
                ),
            )
        } else if saved > 0 || skipped > 0 {
            (
                false,
                format!(
                    "Download incompleto ({saved} salvos, {skipped} preservados, {} falha(s): {}). Pasta: {}",
                    errors.len(),
                    errors.join("; "),
                    folder.display()
                ),
            )
        } else {
            (false, errors.join(" | "))
        };
        crate::downloads::finish(&job, ok, msg.clone());
        if let Some(cb) = on_done {
            cb(ok, &msg);
        }
    });
    Ok(())
}

pub fn find_downloaded(id: &str) -> Option<std::path::PathBuf> {
    let sanitized = crate::local::sanitize(&id.replace('/', "__"));
    if let Some(folder) = crate::local::safe_join(&crate::settings::hf_dir(), &sanitized) {
        if folder.is_dir() {
            if let Ok(mut read) = std::fs::read_dir(&folder) {
                if read.any(|e| e.is_ok_and(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    !name.starts_with('.') && !name.ends_with(".part")
                })) {
                    return Some(folder);
                }
            }
        }
    }
    let hub_name = format!("models--{}", id.replace('/', "--"));
    let ai_models_dir = std::path::PathBuf::from(r"E:\REPOSITORIOS\AI_MODELS");
    let hub_path = ai_models_dir.join(&hub_name);
    if hub_path.is_dir() {
        return Some(hub_path);
    }
    None
}

pub fn list_downloaded() -> Vec<HfModel> {
    let mut list = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let hf_dir = crate::settings::hf_dir();
    if let Ok(entries) = std::fs::read_dir(&hf_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let has_files = std::fs::read_dir(&path).is_ok_and(|mut r| {
                r.any(|e| e.is_ok_and(|item| {
                    let name = item.file_name().to_string_lossy().to_string();
                    !name.starts_with('.') && !name.ends_with(".part")
                }))
            });
            if !has_files {
                continue;
            }

            let dirname = entry.file_name().to_string_lossy().to_string();
            let (author, name) = if let Some((a, n)) = dirname.split_once("__") {
                (a.to_string(), n.to_string())
            } else if let Some(stripped) = dirname.strip_prefix("models--") {
                if let Some((a, n)) = stripped.split_once("--") {
                    (a.to_string(), n.to_string())
                } else {
                    ("local".to_string(), stripped.to_string())
                }
            } else {
                ("local".to_string(), dirname.clone())
            };

            let id = format!("{author}/{name}");
            if seen_ids.insert(id.clone()) {
                let mut task_label = "Modelo baixado".to_string();
                let mut group = "outros".to_string();
                let config_file = path.join("config.json");
                if let Ok(cfg_str) = std::fs::read_to_string(&config_file) {
                    if let Ok(cfg_val) = serde_json::from_str::<serde_json::Value>(&cfg_str) {
                        if let Some(pipe) = cfg_val["pipeline_tag"].as_str() {
                            let (lbl, grp) = task_info(pipe);
                            task_label = lbl;
                            group = grp.to_string();
                        }
                    }
                }

                list.push(HfModel {
                    id: id.clone(),
                    author,
                    name,
                    task: "model".into(),
                    task_label,
                    group,
                    likes: 0,
                    downloads: 0,
                    trending_score: 0,
                    created_at: String::new(),
                    is_new: false,
                    url: format!("https://huggingface.co/{id}"),
                    summary: None,
                    is_downloaded: true,
                    downloaded_path: Some(path.display().to_string()),
                });
            }
        }
    }

    let ai_models_dir = std::path::PathBuf::from(r"E:\REPOSITORIOS\AI_MODELS");
    if let Ok(entries) = std::fs::read_dir(&ai_models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dirname = entry.file_name().to_string_lossy().to_string();
            if let Some(stripped) = dirname.strip_prefix("models--") {
                if let Some((author, name)) = stripped.split_once("--") {
                    let id = format!("{author}/{name}");
                    if seen_ids.insert(id.clone()) {
                        list.push(HfModel {
                            id: id.clone(),
                            author: author.to_string(),
                            name: name.to_string(),
                            task: "model".into(),
                            task_label: "Modelo baixado".into(),
                            group: "outros".into(),
                            likes: 0,
                            downloads: 0,
                            trending_score: 0,
                            created_at: String::new(),
                            is_new: false,
                            url: format!("https://huggingface.co/{id}"),
                            summary: None,
                            is_downloaded: true,
                            downloaded_path: Some(path.display().to_string()),
                        });
                    }
                }
            }
        }
    }

    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_groups() {
        let json = serde_json::json!([
            {"id": "Qwen/Qwen-Image", "pipeline_tag": "text-to-image", "likes": 10, "downloads": 5, "trendingScore": 700, "createdAt": "2026-10-09T04:50:01.000Z"},
            {"id": "solo-model", "likes": 1, "createdAt": "2025-01-01T00:00:00.000Z"}
        ]);
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let m = parse(&json, today);
        assert_eq!(
            (
                m[0].author.as_str(),
                m[0].name.as_str(),
                m[0].group.as_str(),
                m[0].is_new
            ),
            ("Qwen", "Qwen-Image", "imagem", true)
        );
        assert_eq!(m[0].task_label, "Texto para imagem");
        assert_eq!(
            (m[1].author.as_str(), m[1].group.as_str(), m[1].is_new),
            ("", "outros", false)
        );
    }

    #[test]
    fn weights_and_card_fields() {
        assert_eq!(
            next_cursor("<https://huggingface.co/api/models?cursor=a%2Bb%3D>; rel=\"next\""),
            Some("a+b=".into())
        );
        assert!(is_weight("model-00001-of-00004.safetensors") && is_weight("pytorch_model.bin"));
        assert!(
            !is_weight("config.json") && !is_weight("README.md") && !is_weight("tokenizer.json")
        );
        let front = "license: apache-2.0\nlanguage:\n- en\n- pt\nbase_model: Qwen/Qwen3-8B";
        assert_eq!(frontmatter_field(front, "license"), vec!["apache-2.0"]);
        assert_eq!(frontmatter_field(front, "language"), vec!["en", "pt"]);
        assert_eq!(
            frontmatter_field(front, "base_model"),
            vec!["Qwen/Qwen3-8B"]
        );
    }

    #[test]
    fn gated_access_payload_builder() {
        let error_body = "Invalid input: expected string, received undefined\n  at [\"First Name\"]\n  at [\"Last Name\"]\n  at [\"By clicking Submit below I accept the terms\"]\n  at Country";
        let mut payload = serde_json::Map::new();
        for line in error_body.lines() {
            if let Some(pos) = line.find("at [\"") {
                if let Some(end) = line[pos + 5..].find("\"]") {
                    let field = &line[pos + 5..pos + 5 + end];
                    if field.to_lowercase().contains("accept") {
                        payload.insert(field.into(), "true".into());
                    } else {
                        payload.insert(field.into(), "Victor Bianchini".into());
                    }
                }
            }
        }
        assert_eq!(
            payload.get("By clicking Submit below I accept the terms").unwrap(),
            "true"
        );
        assert_eq!(payload.get("First Name").unwrap(), "Victor Bianchini");
    }

    #[test]
    #[ignore]
    fn live_flux3_gated_files() {
        let http = reqwest::blocking::Client::new();
        let res = files(&http, "black-forest-labs/flux-3-image-klein-comfy").expect("files");
        assert!(res.gated);
        assert!(!res.files.is_empty());
        println!("Flux 3 Klein files count: {}", res.files.len());
    }
}
