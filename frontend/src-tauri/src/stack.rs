//! Análise de stack dos repositórios já baixados na pasta GitHub configurada.
//!
//! O que faz:
//! - Lê manifestos locais (package.json, pyproject.toml, requirements*.txt, Cargo.toml,
//!   go.mod, Gemfile, composer.json) e detecta linguagens/stacks e dependências.
//! - Cruza as dependências com uma tabela de conhecimento (aprendida do ecossistema
//!   Bianchini) e gera sugestões de download: repos/modelos complementares que faltam.
//! - Detecta pendências de Git LFS (arquivos baixados como ponteiros quando o git-lfs
//!   não está instalado).
//!
//! Nada baixa sozinho: as sugestões viram pedidos na fila "Pedidos dos agentes".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;

use crate::{db, local};

const MAX_DEP_LEN: usize = 64;
const MAX_LFS_SAMPLE_FILES: usize = 400;
const MAX_WALK_DEPTH: usize = 3;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "venv",
    ".venv",
    "env",
    "site-packages",
    "__pycache__",
    ".cache",
    ".huggingface",
    ".idea",
    ".vscode",
    ".gradle",
    ".mvn",
    ".tox",
    ".nox",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    "vendor",
    "bower_components",
    ".yarn",
    ".cargo",
    ".npm",
    ".pnpm-store",
    "DerivedData",
    "Pods",
];

const LFS_EXTS: &[&str] = &[
    "bin", "pth", "ckpt", "safetensors", "gguf", "onnx", "wav", "mp3", "flac", "m4a", "ogg",
    "mp4", "mov", "avi", "mkv", "webm", "zip", "tar", "gz", "bz2", "7z", "rar", "exe", "dll",
    "so", "lib", "wasm", "npy", "npz", "parquet", "pickle", "pkl", "joblib", "model", "pt",
    "h5", "hdf5", "tflite", "pb", "mindir", "vocab", "spm", "pak",
];

#[derive(Serialize, Clone, Default)]
pub struct LocalStackReport {
    pub folder: String,
    pub repo_id: String,
    pub stacks: Vec<String>,
    pub dependencies: Vec<String>,
    pub lfs_issue: bool,
}

#[derive(Serialize, Clone)]
pub struct StackSuggestion {
    pub id: String,
    /// "repo" | "model"
    pub kind: String,
    pub name: String,
    pub domain: String,
    pub reason: String,
    pub from_folders: Vec<String>,
    pub already_requested: bool,
}

#[derive(Serialize)]
pub struct StackAnalysis {
    pub reports: Vec<LocalStackReport>,
    pub suggestions: Vec<StackSuggestion>,
    pub scanned_folders: usize,
    pub lfs_folders: Vec<String>,
    pub omitted_downloaded: usize,
}

#[derive(Serialize)]
pub struct LfsFixReport {
    pub git_lfs_installed: bool,
    pub fixed_folders: Vec<String>,
    pub errors: Vec<String>,
    pub message: String,
}

/// Triggers distintos e pastas de evidência por domínio (separados: a
/// dependência que ativou não é evidência de pasta).
#[derive(Default)]
struct DomainHit {
    distinct: HashSet<String>,
    folders: HashSet<String>,
}

// ---------------------------------------------------------------------------
// Tabela de conhecimento: dependência normalizada -> sugestões (kind:id)
// ---------------------------------------------------------------------------

type Rule = (&'static str, &'static str, &'static str, &'static str); // (kind, id, domain, reason)

const RULES: &[(&str, &[Rule])] = &[
    // ---- Voz e STT ----
    ("funasr", &[("repo", "modelscope/FunASR", "Voz e STT", "Engine de fala usada pelo GPT-SoVITS; completa o reconhecimento multilíngue")]),
    ("cosyvoice", &[("repo", "FunAudioLLM/CosyVoice", "Voz e STT", "TTS zero-shot com clonagem; base do Voice PRO")]),
    ("chatterbox", &[("repo", "resemble-ai/chatterbox", "Voz e STT", "TTS multilíngue usado pelo PyVideoTrans na dublagem")]),
    ("zonos", &[("model", "Zyphra/Zonos-v0.1-hybrid", "Voz e STT", "Pesos do TTS híbrido Zonos (clonagem em tempo real)")]),
    ("csm", &[("model", "SesameAILabs/csm-1b", "Voz e STT", "Pesos do CSM, voz conversacional full-duplex")]),
    ("sherpaonnx", &[("repo", "k2-fsa/sherpa-onnx", "Voz e STT", "Inferência leve de STT/TTS em CPU, sem dependências pesadas")]),
    ("fasterwhisper", &[("repo", "SYSTRAN/faster-whisper", "Voz e STT", "Whisper acelerado com CTranslate2")]),
    ("whisperx", &[("repo", "m-bain/whisperX", "Voz e STT", "STT com alinhamento por palavra e diarização")]),
    ("pyannote", &[("model", "pyannote/speaker-diarization-3.1", "Voz e STT", "Pesos de diarização; só o VAD está baixado")]),
    ("pyannoteaudio", &[("model", "pyannote/speaker-diarization-3.1", "Voz e STT", "Pesos de diarização; só o VAD está baixado")]),
    ("whisper", &[("model", "openai/whisper-large-v3", "Voz e STT", "Pesos Whisper v3 para STT local de alta qualidade")]),
    ("f5tts", &[("repo", "SWivid/F5-TTS", "Voz e STT", "TTS não-autoregressivo rápido com clonagem")]),
    ("xtts", &[("model", "coqui/XTTS-v2", "Voz e STT", "Pesos do XTTS, clonagem em 16 idiomas")]),
    ("kokoro", &[("model", "hexgrad/Kokoro-82M", "Voz e STT", "Pesos do TTS leve Kokoro (82M)")]),
    ("gptsovits", &[("repo", "RVC-Boss/GPT-SoVITS", "Voz e STT", "TTS com poucos segundos de áudio de referência")]),
    ("fishspeech", &[("repo", "fishaudio/fish-speech", "Voz e STT", "TTS de referência do Fish Audio")]),
    ("demucs", &[("repo", "facebookresearch/demucs", "Voz e STT", "Separação de stems musical para limpeza de áudio")]),
    ("audiocraft", &[("repo", "facebookresearch/audiocraft", "Voz e STT", "Geração de música e efeitos (MusicGen/AudioGen)")]),
    ("musicgen", &[("model", "facebook/musicgen-small", "Voz e STT", "Pesos MusicGen; o Audiocraft está sem pesos")]),
    ("edgetts", &[("repo", "rany2/edge-tts", "Voz e STT", "TTS gratuito Edge direto do Python")]),
    ("piper", &[("repo", "rhasspy/piper", "Voz e STT", "TTS embarcado rápido para Raspberry Pi e CPU fraca")]),
    // ---- Agentes de IA ----
    ("langgraph", &[("repo", "langchain-ai/langgraph", "Agentes de IA", "Orquestração de agentes com grafos de estado")]),
    ("deepagents", &[("repo", "langchain-ai/deepagents", "Agentes de IA", "Agentes profundos com planejamento e subagentes")]),
    ("autogen", &[("repo", "ag2ai/ag2", "Agentes de IA", "Sucessor do AutoGen (ag2) para multiagentes")]),
    ("ag2", &[("repo", "ag2ai/ag2", "Agentes de IA", "Framework multiagente AG2")]),
    ("pydanticai", &[("repo", "pydantic/pydantic-ai", "Agentes de IA", "Agentes com tipagem forte via Pydantic")]),
    ("smolagents", &[("repo", "huggingface/smolagents", "Agentes de IA", "Agentes minimalistas da Hugging Face")]),
    ("crewai", &[("repo", "crewAIInc/crewAI", "Agentes de IA", "Times de agentes com papéis e tarefas")]),
    ("agno", &[("repo", "agno-agi/agno", "Agentes de IA", "Framework leve de agentes multimodais")]),
    ("openaiagents", &[("repo", "openai/openai-agents-python", "Agentes de IA", "SDK oficial de agentes da OpenAI")]),
    ("llamaindex", &[("repo", "run-llama/llama_index", "Agentes de IA", "RAG e agentes sobre dados próprios")]),
    ("llamaindexcore", &[("repo", "run-llama/llama_index", "Agentes de IA", "RAG e agentes sobre dados próprios")]),
    ("dspy", &[("repo", "stanfordnlp/dspy", "Agentes de IA", "Programação de prompts de forma declarativa")]),
    ("instructor", &[("repo", "instructor-ai/instructor", "Agentes de IA", "Saídas estruturadas confiáveis de LLMs")]),
    ("mem0", &[("repo", "mem0ai/mem0", "Agentes de IA", "Camada de memória para agentes")]),
    ("langfuse", &[("repo", "langfuse/langfuse", "Agentes de IA", "Observabilidade e avaliação de LLMs")]),
    ("litellm", &[("repo", "BerriAI/litellm", "Agentes de IA", "Gateway único para 100+ provedores de LLM")]),
    ("openhands", &[("repo", "All-Hands-AI/OpenHands", "Agentes de IA", "Agente de engenharia de software autônomo")]),
    ("aider", &[("repo", "Aider-AI/aider", "Agentes de IA", "Pair programming de IA no terminal")]),
    ("stagehand", &[("repo", "browserbase/stagehand", "Agentes de IA", "Atuação em navegador com linguagem natural")]),
    ("browseruse", &[("repo", "browser-use/browser-use", "Agentes de IA", "Agentes que controlam o navegador")]),
    ("crawl4ai", &[("repo", "unclecode/crawl4ai", "Agentes de IA", "Scraping pronto para LLMs")]),
    // ---- Rust ----
    ("tauri", &[("repo", "tauri-apps/tauri", "Rust e Desktop", "Framework desktop webview (stack do GitTrend Radar)")]),
    ("gpui", &[("repo", "zed-industries/zed", "Rust e Desktop", "UI nativa em Rust do editor Zed")]),
    ("egui", &[("repo", "emilk/egui", "Rust e Desktop", "GUI imediata em Rust")]),
    ("rocket", &[("repo", "rwf2/Rocket", "Rust e Desktop", "Web framework Rust com macros")]),
    ("actixweb", &[("repo", "actix/actix-web", "Rust e Desktop", "Web framework Rust de alta performance")]),
    ("bevy", &[("repo", "bevyengine/bevy", "Rust e Desktop", "Game engine em Rust")]),
    // ---- Go ----
    ("huma", &[("repo", "danielgtaylor/huma", "Go", "APIs HTTP com OpenAPI automática em Go")]),
    ("gin", &[("repo", "gin-gonic/gin", "Go", "Web framework Go mais popular")]),
    ("chi", &[("repo", "go-chi/chi", "Go", "Router Go leve e idiomático")]),
    ("cobra", &[("repo", "spf13/cobra", "Go", "CLIs em Go")]),
    ("bubbletea", &[("repo", "charmbracelet/bubbletea", "Go", "TUIs em Go do Charm")]),
    ("viper", &[("repo", "spf13/viper", "Go", "Configuração em Go")]),
    // ---- JavaScript / Frontend ----
    ("remotion", &[("repo", "remotion-dev/remotion", "Vídeo e Criativo", "Vídeo como código em React")]),
    ("motioncanvas", &[("repo", "motion-canvas/motion-canvas", "Vídeo e Criativo", "Animações programáticas")]),
    // ---- Python core ----
    ("vllm", &[("repo", "vllm-project/vllm", "Python Core", "Inferência de LLM em alta vazão")]),
    ("unsloth", &[("repo", "unslothai/unsloth", "Python Core", "Fine-tuning de LLMs 2x mais rápido")]),
    ("llamacpp", &[("repo", "ggml-org/llama.cpp", "Python Core", "Inferência GGUF local")]),
    ("llamacpppython", &[("repo", "ggml-org/llama.cpp", "Python Core", "Inferência GGUF local")]),
    ("ollama", &[("repo", "ollama/ollama", "Python Core", "Runtime local de LLMs (usado pelo app)")]),
    ("ray", &[("repo", "ray-project/ray", "Python Core", "Computação distribuída")]),
    ("polars", &[("repo", "pola-rs/polars", "Python Core", "DataFrames rápidos")]),
    ("duckdb", &[("repo", "duckdb/duckdb", "Python Core", "SQL analítico embarcado")]),
    // ---- Dados e Infra ----
    ("supabase", &[("repo", "supabase/supabase", "Infra Self-hosted", "Backend completo (Postgres+Auth+Storage)")]),
    ("supabasejs", &[("repo", "supabase/supabase", "Infra Self-hosted", "Backend completo (Postgres+Auth+Storage)")]),
    ("betterauth", &[("repo", "better-auth/better-auth", "Infra Self-hosted", "Autenticação TypeScript moderna")]),
    ("meilisearch", &[("repo", "meilisearch/meilisearch", "Infra Self-hosted", "Busca full-text instantânea")]),
    ("typesense", &[("repo", "typesense/typesense", "Infra Self-hosted", "Busca full-text com tipagem")]),
    ("qdrantclient", &[("repo", "qdrant/qdrant", "Infra Self-hosted", "Banco vetorial para RAG")]),
    ("chromadb", &[("repo", "chroma-core/chroma", "Infra Self-hosted", "Banco vetorial com API simples")]),
    ("falkordb", &[("repo", "FalkorDB/FalkorDB", "Infra Self-hosted", "Banco de grafos com Cypher")]),
    ("n8n", &[("repo", "n8n-io/n8n", "Infra Self-hosted", "Automação visual com 400+ integrações")]),
    ("activepieces", &[("repo", "activepieces/activepieces", "Infra Self-hosted", "Automação open source alternativa ao Zapier")]),
    ("coolify", &[("repo", "coollabsio/coolify", "Infra Self-hosted", "PaaS próprio para VPS")]),
    ("minio", &[("repo", "minio/minio", "Infra Self-hosted", "Storage compatível com S3")]),
    ("pocketbase", &[("repo", "pocketbase/pocketbase", "Infra Self-hosted", "Backend embarcado em um executável")]),
    ("appwrite", &[("repo", "appwrite/appwrite", "Infra Self-hosted", "Backend completo self-hosted")]),
    // ---- Segurança e RE ----
    ("frida", &[("repo", "frida/frida", "Segurança e RE", "Instrumentação dinâmica multiplataforma")]),
    ("pwntools", &[("repo", "Gallopsled/pwntools", "Segurança e RE", "Exploração de binários (CTF)")]),
    ("angr", &[("repo", "angr/angr", "Segurança e RE", "Análise binária simbólica")]),
    ("yara", &[("repo", "VirusTotal/yara", "Segurança e RE", "Regras de detecção de malware")]),
    ("yarapython", &[("repo", "VirusTotal/yara", "Segurança e RE", "Regras de detecção de malware")]),
    ("impacket", &[("repo", "fortra/impacket", "Segurança e RE", "Protocolos de rede para pentest")]),
    ("volatility3", &[("repo", "volatilityfoundation/volatility3", "Segurança e RE", "Forense de memória RAM")]),
    ("mitmproxy", &[("repo", "mitmproxy/mitmproxy", "Segurança e RE", "Interceptação de tráfego HTTP(S)")]),
    ("objection", &[("repo", "sensepost/objection", "Segurança e RE", "Exploração de apps mobile em runtime")]),
    ("nuclei", &[("repo", "projectdiscovery/nuclei", "Segurança e RE", "Scanner de vulnerabilidades por templates")]),
    ("jadx", &[("repo", "skylot/jadx", "Segurança e RE", "Decompilador Java/Android")]),
    ("apktool", &[("repo", "iBotPeaches/Apktool", "Segurança e RE", "Engenharia reversa de APKs")]),
    // ---- WhatsApp e Marketing ----
    ("baileys", &[("repo", "WhiskeySockets/Baileys", "WhatsApp e Marketing", "Cliente WhatsApp Web em Node (não oficial)")]),
    ("whatsappwebjs", &[("repo", "pedroslopez/whatsapp-web.js", "WhatsApp e Marketing", "API WhatsApp Web em Node")]),
    ("evolutionapi", &[("repo", "EvolutionAPI/evolution-api", "WhatsApp e Marketing", "API WhatsApp multi-tenant")]),
    ("instagrapi", &[("repo", "subzeroid/instagrapi", "WhatsApp e Marketing", "Automação de Instagram em Python")]),
    ("instaloader", &[("repo", "instaloader/instaloader", "WhatsApp e Marketing", "Download e análise de perfis do Instagram")]),
    ("tweepy", &[("repo", "tweepy/tweepy", "WhatsApp e Marketing", "API do X/Twitter em Python")]),
    ("discordpy", &[("repo", "Rapptz/discord.py", "WhatsApp e Marketing", "Bots de Discord em Python")]),
    ("telethon", &[("repo", "LonamiWebs/Telethon", "WhatsApp e Marketing", "Client Telegram em Python")]),
    // ---- Vídeo ----
    ("wav2lip", &[("repo", "Rudrabha/Wav2Lip", "Vídeo e Criativo", "Sincronização labial para dublagem")]),
    ("moviepy", &[("repo", "Zulko/moviepy", "Vídeo e Criativo", "Edição de vídeo programática")]),
    ("ffmpegpython", &[("repo", "kkroening/ffmpeg-python", "Vídeo e Criativo", "Bindings Python do FFmpeg")]),
    ("opencvpython", &[("repo", "opencv/opencv", "Vídeo e Criativo", "Visão computacional")]),
    ("cv2", &[("repo", "opencv/opencv", "Vídeo e Criativo", "Visão computacional")]),
    ("pyvideotrans", &[("repo", "jianchang512/pyvideotrans", "Vídeo e Criativo", "Tradução e dublagem de vídeos")]),
    ("nexrender", &[("repo", "inlife/nexrender", "Vídeo e Criativo", "Render distribuído do After Effects")]),
    ("autoeditor", &[("repo", "WyattBlue/auto-editor", "Vídeo e Criativo", "Edição automática por análise de áudio")]),
    // ---- Documentos ----
    ("docling", &[("repo", "docling-project/docling", "Documentos e RAG", "Extração layout-aware de PDF/DOCX")]),
    ("mineru", &[("repo", "opendatalab/MinerU", "Documentos e RAG", "Parsing de PDFs científicos de alta qualidade")]),
    ("markitdown", &[("repo", "microsoft/markitdown", "Documentos e RAG", "Conversão universal de arquivos para Markdown")]),
    ("unstructured", &[("repo", "Unstructured-IO/unstructured", "Documentos e RAG", "Pré-processamento de documentos para RAG")]),
    ("pymupdf", &[("repo", "pymupdf/PyMuPDF", "Documentos e RAG", "Manipulação de PDF em Python")]),
    ("fitz", &[("repo", "pymupdf/PyMuPDF", "Documentos e RAG", "Manipulação de PDF em Python")]),
    ("pythondocx", &[("repo", "python-openxml/python-docx", "Documentos e RAG", "Leitura/escrita de DOCX")]),
    ("tabulapy", &[("repo", "chezou/tabula-py", "Documentos e RAG", "Extração de tabelas de PDFs")]),
    // ---- Trading ----
    ("freqtrade", &[("repo", "freqtrade/freqtrade", "Trading e Finanças", "Bot de trading em produção com backtesting")]),
    ("openbb", &[("repo", "OpenBB-finance/OpenBB", "Trading e Finanças", "Terminal de pesquisa financeira")]),
    ("ccxt", &[("repo", "ccxt/ccxt", "Trading e Finanças", "Client unificado de 100+ exchanges")]),
    ("backtrader", &[("repo", "mementum/backtrader", "Trading e Finanças", "Backtesting de estratégias")]),
    ("yfinance", &[("repo", "ranaroussi/yfinance", "Trading e Finanças", "Dados do Yahoo Finance")]),
    ("talib", &[("repo", "TA-Lib/ta-lib-python", "Trading e Finanças", "Indicadores técnicos")]),
    ("chronos", &[("model", "amazon/chronos-t5-tiny", "Trading e Finanças", "Pesos Chronos para previsão de séries temporais")]),
];

// Domínios: quando 2+ dependências do cluster aparecem no disco, sugerir as
// peças essenciais que ainda não estão baixadas.
struct Domain {
    name: &'static str,
    triggers: &'static [&'static str],
    essentials: &'static [Rule],
}

const DOMAINS: &[Domain] = &[
    Domain {
        name: "Voz e STT",
        triggers: &["funasr", "cosyvoice", "chatterbox", "whisper", "whisperx", "fasterwhisper", "pyannote", "pyannoteaudio", "f5tts", "xtts", "kokoro", "fishspeech", "gptsovits", "sherpaonnx", "csm", "zonos", "edgetts", "piper", "demucs", "audiocraft", "musicgen"],
        essentials: &[
            ("repo", "livekit/agents", "Voz e STT", "Agentes de voz em tempo real (STT+LLM+TTS em um pipeline único)"),
            ("repo", "livekit/livekit", "Voz e STT", "Infra WebRTC para voz e vídeo em tempo real"),
            ("repo", "pipecat-ai/pipecat", "Voz e STT", "Framework Python de agentes conversacionais de voz"),
        ],
    },
    Domain {
        name: "Segurança e RE",
        triggers: &["frida", "pwntools", "angr", "yara", "yarapython", "impacket", "volatility3", "mitmproxy", "objection", "nuclei", "jadx", "apktool"],
        essentials: &[
            ("repo", "NationalSecurityAgency/ghidra", "Segurança e RE", "Descompilador da NSA; peça central do arsenal estático"),
            ("repo", "rizinorg/cutter", "Segurança e RE", "GUI de engenharia reversa"),
            ("repo", "MobSF/Mobile-Security-Framework-MobSF", "Segurança e RE", "Análise automática de apps Android/iOS"),
            ("repo", "mandiant/capa", "Segurança e RE", "Identificação de capacidades em binários"),
            ("repo", "mandiant/flare-vm", "Segurança e RE", "Máquina virtual completa para análise de malware"),
        ],
    },
    Domain {
        name: "Trading e Finanças",
        triggers: &["freqtrade", "openbb", "ccxt", "backtrader", "yfinance", "talib", "chronos"],
        essentials: &[
            ("repo", "virattt/ai-hedge-fund", "Trading e Finanças", "Sinais de trading por LLM"),
            ("repo", "TauricResearch/TradingAgents", "Trading e Finanças", "Simulação multiagente de trading"),
        ],
    },
    Domain {
        name: "Documentos e RAG",
        triggers: &["docling", "mineru", "markitdown", "unstructured", "pymupdf", "fitz", "pythondocx", "tabulapy", "langchain", "llamaindex", "llamaindexcore", "chromadb", "qdrantclient"],
        essentials: &[
            ("repo", "infiniflow/ragflow", "Documentos e RAG", "RAG completo com parsing profundo de documentos"),
            ("repo", "nlmatics/nlm-ingestor", "Documentos e RAG", "Parser universal para RAG"),
            ("repo", "deepdoctection/deepdoctection", "Documentos e RAG", "Layout e tabelas de documentos em Python"),
        ],
    },
    Domain {
        name: "Vídeo e Criativo",
        triggers: &["remotion", "motioncanvas", "moviepy", "ffmpegpython", "wav2lip", "opencvpython", "cv2", "pyvideotrans", "nexrender", "autoeditor"],
        essentials: &[
            ("repo", "harry0703/MoneyPrinterTurbo", "Vídeo e Criativo", "Vídeos curtos automatizados ponta a ponta"),
            ("repo", "comfyanonymous/ComfyUI", "Vídeo e Criativo", "Pipelines de geração por nós"),
            ("repo", "Wan-Video/Wan2.2", "Vídeo e Criativo", "Geração de vídeo open source Wan 2.2"),
            ("repo", "Lightricks/LTX-Video", "Vídeo e Criativo", "Geração de vídeo rápida LTX"),
        ],
    },
    Domain {
        name: "Agentes de IA",
        triggers: &["langgraph", "deepagents", "langchain", "langchaincore", "autogen", "ag2", "pydanticai", "smolagents", "crewai", "agno", "openaiagents", "llamaindex", "llamaindexcore", "dspy", "instructor", "mem0", "langfuse", "openhands", "aider", "browseruse", "stagehand", "crawl4ai", "playwright", "selenium"],
        essentials: &[
            ("repo", "All-Hands-AI/OpenHands", "Agentes de IA", "Agente de engenharia de software autônomo"),
            ("repo", "Aider-AI/aider", "Agentes de IA", "Pair programming de IA no terminal"),
            ("repo", "browser-use/browser-use", "Agentes de IA", "Agentes que controlam o navegador"),
        ],
    },
    Domain {
        name: "Infra Self-hosted",
        triggers: &["supabase", "supabasepy", "betterauth", "appwrite", "pocketbase", "meilisearch", "typesense", "qdrantclient", "chromadb", "falkordb", "redis", "redispy", "n8n", "activepieces", "coolify", "minio", "postgres", "psycopg2", "psycopg", "sqlalchemy", "alembic"],
        essentials: &[
            ("repo", "coollabsio/coolify", "Infra Self-hosted", "PaaS próprio para VPS"),
            ("repo", "appwrite/appwrite", "Infra Self-hosted", "Backend completo self-hosted"),
            ("repo", "pocketbase/pocketbase", "Infra Self-hosted", "Backend embarcado em um executável"),
            ("repo", "minio/minio", "Infra Self-hosted", "Storage compatível com S3"),
        ],
    },
];

// ---------------------------------------------------------------------------
// Manifestos
// ---------------------------------------------------------------------------

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_dep(raw: &str) -> String {
    let s = raw
        .trim()
        .split(|c: char| {
            c.is_whitespace() || matches!(c, '=' | '>' | '<' | '~' | '!' | ';' | ',' | '(' | '[' | '{')
        })
        .next()
        .unwrap_or_default();
    let s = s.trim_start_matches(|c: char| c == '"' || c == '\'' || c == '^' || c == '-');
    // Caminhos de módulo (Go, scoped npm): o casamento usa o último segmento.
    // Pontos ficam para o norm() remover (discord.py -> discordpy).
    let s = s.rsplit('/').next().unwrap_or(s);
    norm(s)
}

fn read_manifest(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn parse_package(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    let mut stacks = vec!["Node/JS".to_string()];
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return (stacks, deps);
    };
    for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        if let Some(obj) = v.get(section).and_then(|x| x.as_object()) {
            deps.extend(obj.keys().cloned());
        }
    }
    if deps.iter().any(|d| d == "react") {
        stacks.push("React".into());
    }
    if deps.iter().any(|d| d == "@tauri-apps/api") {
        stacks.push("Tauri".into());
    }
    (stacks, deps)
}

fn parse_pyproject(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    let stacks = vec!["Python".to_string()];
    let re_block = regex::Regex::new(r"(?s)dependencies\s*=\s*\[(.*?)\]").unwrap();
    let re_str = regex::Regex::new(r#""([^"]+)""#).unwrap();
    for cap in re_block.captures_iter(text) {
        for s in re_str.captures_iter(&cap[1]) {
            deps.push(s[1].to_string());
        }
    }
    (stacks, deps)
}

fn parse_requirements(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('-')
            || line.contains("://")
            || line.starts_with("git+")
        {
            continue;
        }
        let name = line
            .split(|c: char| {
                c.is_whitespace() || matches!(c, '=' | '>' | '<' | '~' | '!' | ';' | ',')
            })
            .next()
            .unwrap_or_default()
            .trim();
        if !name.is_empty() {
            deps.push(name.to_string());
        }
    }
    (vec!["Python".to_string()], deps)
}

fn parse_cargo(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]" || t == "[dev-dependencies]" || t == "[build-dependencies]";
            continue;
        }
        if !in_deps || t.is_empty() || t.starts_with('#') {
            continue;
        }
        let name = t
            .split('=')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        if !name.is_empty() && !name.contains(' ') {
            deps.push(name.to_string());
        }
    }
    (vec!["Rust".to_string()], deps)
}

fn parse_gomod(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    let mut in_import = false;
    for line in text.lines() {
        let t = line.trim();
        if t == "import (" {
            in_import = true;
            continue;
        }
        if in_import {
            if t == ")" {
                in_import = false;
            } else if let Some(pkg) = t.trim_matches('"').split_whitespace().next() {
                deps.push(pkg.to_string());
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("require ") {
            if let Some(pkg) = rest.split_whitespace().next() {
                deps.push(pkg.to_string());
            }
        }
    }
    (vec!["Go".to_string()], deps)
}

fn parse_gemfile(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("gem ") {
            if let Some(name) = rest.split(',').next() {
                deps.push(name.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    (vec!["Ruby".to_string()], deps)
}

fn parse_composer(text: &str) -> (Vec<String>, Vec<String>) {
    let mut deps = Vec::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return (vec!["PHP".to_string()], deps);
    };
    for section in ["require", "require-dev"] {
        if let Some(obj) = v.get(section).and_then(|x| x.as_object()) {
            deps.extend(obj.keys().cloned());
        }
    }
    (vec!["PHP".to_string()], deps)
}

fn walk_manifests(dir: &Path, depth: usize, out: &mut Vec<(Vec<String>, Vec<String>)>) {
    if depth > MAX_WALK_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk_manifests(&path, depth + 1, out);
            continue;
        }
        let parsed: Option<(Vec<String>, Vec<String>)> = match name.as_str() {
            "package.json" => read_manifest(&path).map(|t| parse_package(&t)),
            "pyproject.toml" => read_manifest(&path).map(|t| parse_pyproject(&t)),
            "cargo.toml" => read_manifest(&path).map(|t| parse_cargo(&t)),
            "go.mod" => read_manifest(&path).map(|t| parse_gomod(&t)),
            "gemfile" => read_manifest(&path).map(|t| parse_gemfile(&t)),
            "composer.json" => read_manifest(&path).map(|t| parse_composer(&t)),
            n if n.starts_with("requirements") && n.ends_with(".txt") => {
                read_manifest(&path).map(|t| parse_requirements(&t))
            }
            _ => None,
        };
        let Some((stacks, deps)) = parsed else {
            continue;
        };
        if !stacks.is_empty() || !deps.is_empty() {
            out.push((stacks, deps));
        }
    }
}

// ---------------------------------------------------------------------------
// Git LFS
// ---------------------------------------------------------------------------

fn git_lfs_available() -> bool {
    static LFS: OnceLock<bool> = OnceLock::new();
    *LFS.get_or_init(|| {
        std::process::Command::new("git")
            .args(["lfs", "version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn is_lfs_pointer(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() > 2048 {
        return false;
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !LFS_EXTS.contains(&ext.as_str()) {
        return false;
    }
    std::fs::read_to_string(path)
        .is_ok_and(|text| text.contains("git-lfs.github.com/spec/v1"))
}

fn lfs_pending(folder: &Path) -> bool {
    if git_lfs_available() {
        return false;
    }
    let ga = folder.join(".gitattributes");
    if !ga.is_file()
        || !std::fs::read_to_string(&ga).is_ok_and(|t| t.to_lowercase().contains("lfs"))
    {
        return false;
    }
    let mut budget = 0usize;
    sample_lfs_pointers(folder, 0, &mut budget)
}

fn sample_lfs_pointers(dir: &Path, depth: usize, budget: &mut usize) -> bool {
    if depth > MAX_WALK_DEPTH || *budget >= MAX_LFS_SAMPLE_FILES {
        return false;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if sample_lfs_pointers(&path, depth + 1, budget) {
                return true;
            }
        } else {
            *budget += 1;
            if is_lfs_pointer(&path) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Identificação do repo local
// ---------------------------------------------------------------------------

fn repo_id_of(folder: &Path) -> String {
    let Ok(config) = std::fs::read_to_string(folder.join(".git/config")) else {
        return String::new();
    };
    for line in config.lines() {
        let t = line.trim();
        let Some((key, value)) = t.split_once('=') else {
            continue;
        };
        if key.trim() != "url" {
            continue;
        }
        let value = value.trim().trim_end_matches('/');
        let remote = value
            .strip_prefix("https://github.com/")
            .or_else(|| value.strip_prefix("git@github.com:"));
        if let Some(remote) = remote {
            return remote.strip_suffix(".git").unwrap_or(remote).to_string();
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Análise principal
// ---------------------------------------------------------------------------

pub fn analyze(db_path: &Path) -> Result<StackAnalysis, String> {
    let base = PathBuf::from(crate::settings::github_dir());
    let dirs = local::existing_dirs();
    let downloaded_model_ids: HashSet<String> = crate::models::list_downloaded()
        .into_iter()
        .map(|m| m.id)
        .collect();
    let downloaded_dirs: Vec<String> = dirs.clone();

    let mut reports = Vec::new();
    let mut lfs_folders = Vec::new();
    let mut dep_folders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut domain_hits: HashMap<&'static str, DomainHit> = HashMap::new();

    let skip_folder = |dir: &str| -> bool {
        let n = norm(dir);
        dir.starts_with('_')
            || matches!(n.as_str(), "aimodels" | "huggingface" | "skills" | "refs" | "datasets" | "downloads")
    };

    for dir in dirs {
        if skip_folder(&dir) {
            continue;
        }
        let folder = base.join(&dir);
        if !folder.is_dir() {
            continue;
        }
        let mut report = LocalStackReport {
            folder: dir.clone(),
            repo_id: repo_id_of(&folder),
            ..Default::default()
        };
        let mut found: Vec<(Vec<String>, Vec<String>)> = Vec::new();
        walk_manifests(&folder, 0, &mut found);
        for (stacks, deps) in found {
            for s in stacks {
                if !report.stacks.contains(&s) {
                    report.stacks.push(s);
                }
            }
            for d in deps {
                let key = normalize_dep(&d);
                if key.len() < 3 || key.len() > MAX_DEP_LEN {
                    continue;
                }
                if !report.dependencies.contains(&key) {
                    report.dependencies.push(key.clone());
                }
                dep_folders
                    .entry(key.clone())
                    .or_default()
                    .push(dir.clone());
            }
        }
        report.lfs_issue = lfs_pending(&folder);
        if report.lfs_issue {
            lfs_folders.push(dir.clone());
        }
        reports.push(report);
    }

    let conn = db::open(db_path).ok();
    let requested: HashSet<(String, String)> = conn
        .as_ref()
        .and_then(|c| db::list_agent_requests(c, 500).ok())
        .map(|list| {
            list.into_iter()
                .filter(|r| r.status == "pending")
                .map(|r| (r.kind, r.target))
                .collect()
        })
        .unwrap_or_default();

    let is_downloaded = |kind: &str, id: &str| -> bool {
        if kind == "model" {
            return downloaded_model_ids.contains(id);
        }
        let name = id.rsplit('/').next().unwrap_or(id);
        downloaded_dirs.iter().any(|d| {
            let dn = norm(d);
            dn == norm(name) || dn == norm(&local::pretty_name(name))
        })
    };

    let mut by_id: BTreeMap<String, StackSuggestion> = BTreeMap::new();
    let mut omitted_downloaded = 0usize;

    let add = |by_id: &mut BTreeMap<String, StackSuggestion>,
                   kind: &str,
                   id: &str,
                   domain: &str,
                   reason: &str,
                   from_folders: Vec<String>,
                   omitted: &mut usize| {
        if is_downloaded(kind, id) {
            *omitted += 1;
            return;
        }
        let entry = by_id.entry(id.to_string()).or_insert_with(|| StackSuggestion {
            id: id.to_string(),
            kind: kind.to_string(),
            name: id.rsplit('/').next().unwrap_or(id).to_string(),
            domain: domain.to_string(),
            reason: reason.to_string(),
            from_folders: Vec::new(),
            already_requested: requested.contains(&(kind.to_string(), id.to_string())),
        });
        for f in from_folders {
            if !entry.from_folders.contains(&f) && entry.from_folders.len() < 6 {
                entry.from_folders.push(f);
            }
        }
    };

    for (dep, folders) in &dep_folders {
        if let Some(rule_list) = RULES.iter().find(|(d, _)| *d == dep.as_str()).map(|(_, r)| *r) {
            for (kind, id, domain, reason) in rule_list {
                add(
                    &mut by_id,
                    kind,
                    id,
                    domain,
                    reason,
                    folders.clone(),
                    &mut omitted_downloaded,
                );
            }
        }
        for domain in DOMAINS {
            if domain.triggers.contains(&dep.as_str()) {
                let hit = domain_hits.entry(domain.name).or_default();
                hit.distinct.insert(dep.clone());
                hit.folders.extend(folders.iter().cloned());
            }
        }
    }

    for domain in DOMAINS {
        let Some(hit) = domain_hits.get(domain.name) else {
            continue;
        };
        if hit.distinct.len() < 2 {
            continue;
        }
        let from_folders: Vec<String> = hit.folders.iter().cloned().collect();
        for (kind, id, dname, reason) in domain.essentials {
            add(
                &mut by_id,
                kind,
                id,
                dname,
                reason,
                from_folders.clone(),
                &mut omitted_downloaded,
            );
        }
    }

    let mut suggestions: Vec<StackSuggestion> = by_id.into_values().collect();
    suggestions.sort_by(|a, b| {
        b.from_folders
            .len()
            .cmp(&a.from_folders.len())
            .then_with(|| a.domain.cmp(&b.domain))
            .then_with(|| a.id.cmp(&b.id))
    });
    lfs_folders.sort();

    Ok(StackAnalysis {
        scanned_folders: reports.len(),
        reports,
        suggestions,
        lfs_folders,
        omitted_downloaded,
    })
}

// ---------------------------------------------------------------------------
// Correção de Git LFS
// ---------------------------------------------------------------------------

fn run_lfs_pull(folder: &str) -> Result<(), String> {
    let base = crate::settings::github_dir();
    let target = PathBuf::from(&base).join(folder);
    let out = std::process::Command::new("git")
        .args(["lfs", "pull"])
        .current_dir(&target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn fix_lfs(db_path: &Path) -> Result<LfsFixReport, String> {
    let analysis = analyze(db_path)?;
    let mut report = LfsFixReport {
        git_lfs_installed: git_lfs_available(),
        fixed_folders: Vec::new(),
        errors: Vec::new(),
        message: String::new(),
    };
    if !report.git_lfs_installed {
        let out = std::process::Command::new("winget")
            .args([
                "install",
                "--id",
                "GitHub.GitLFS",
                "-e",
                "--silent",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("winget indisponível: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "Instalação do Git LFS falhou. Rode manualmente: `winget install GitHub.GitLFS` ({})",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        report.git_lfs_installed = true;
    }
    if analysis.lfs_folders.is_empty() {
        report.message = "Git LFS instalado; nenhuma pasta com ponteiros pendentes.".into();
        return Ok(report);
    }
    for folder in &analysis.lfs_folders {
        match run_lfs_pull(folder) {
            Ok(()) => report.fixed_folders.push(folder.clone()),
            Err(e) => report.errors.push(format!("{folder}: {e}")),
        }
    }
    if report.errors.is_empty() {
        report.message = format!(
            "Git LFS corrigido em {} pasta(s).",
            report.fixed_folders.len()
        );
    } else {
        report.message = format!(
            "{} corrigida(s), {} com erro. Para as com erro: abra a pasta e rode `git lfs pull`.",
            report.fixed_folders.len(),
            report.errors.len()
        );
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dependency_names() {
        assert_eq!(normalize_dep("pydantic-ai>=0.1"), "pydanticai");
        assert_eq!(normalize_dep("  yara-python==4.5.1  "), "yarapython");
        assert_eq!(normalize_dep("discord.py"), "discordpy");
        assert_eq!(normalize_dep("redis"), "redis");
        assert_eq!(normalize_dep("reqwest = \"0.13\""), "reqwest");
    }

    #[test]
    fn parses_manifests() {
        let (stacks, deps) = parse_requirements("fastapi==0.115\nuvicorn[standard]\n# comentario\n");
        assert!(stacks.contains(&"Python".to_string()));
        assert!(deps.contains(&"fastapi".to_string()));
        assert!(deps.contains(&"uvicorn[standard]".to_string()));

        let (stacks, deps) = parse_cargo(
            "[package]\nname = \"x\"\n[dependencies]\nserde = \"1\"\nreqwest = { version = \"0.13\" }\n[dev-dependencies]\ntokio = \"1\"\n",
        );
        assert!(stacks.contains(&"Rust".to_string()));
        assert!(deps.contains(&"serde".to_string()));
        assert!(deps.contains(&"reqwest".to_string()));
        assert!(deps.contains(&"tokio".to_string()));

        let (stacks, deps) = parse_package(r#"{"dependencies":{"react":"19","@tauri-apps/api":"2"},"devDependencies":{"vite":"7"}}"#);
        assert!(stacks.contains(&"React".to_string()));
        assert!(stacks.contains(&"Tauri".to_string()));
        assert!(deps.contains(&"react".to_string()));
        assert!(deps.contains(&"vite".to_string()));

        let (stacks, deps) = parse_pyproject("[project]\ndependencies = [\"litellm>=1.0\", \"pydantic\"]\n");
        assert!(stacks.contains(&"Python".to_string()));
        assert!(deps.contains(&"litellm>=1.0".to_string()));
        assert!(deps.contains(&"pydantic".to_string()));

        let (stacks, deps) = parse_gomod("module x\n\ngo 1.22\n\nrequire github.com/danielgtaylor/huma v2.1.0\n");
        assert!(stacks.contains(&"Go".to_string()));
        assert!(deps.contains(&"github.com/danielgtaylor/huma".to_string()));

        let (stacks, deps) = parse_composer(r#"{"require":{"php":">=8.0","laravel/framework":"^11"}}"#);
        assert!(stacks.contains(&"PHP".to_string()));
        assert!(deps.contains(&"laravel/framework".to_string()));

        let (stacks, deps) = parse_gemfile("source 'https://rubygems.org'\ngem 'rails', '~> 7.0'\n");
        assert!(stacks.contains(&"Ruby".to_string()));
        assert!(deps.contains(&"rails".to_string()));
    }

    #[test]
    fn knowledge_table_has_valid_ids() {
        for (dep, list) in RULES {
            assert!(!dep.is_empty());
            for (kind, id, domain, reason) in *list {
                assert!(
                    *kind == "repo" || *kind == "model",
                    "kind inválido em {dep}"
                );
                assert_eq!(id.split('/').count(), 2, "id inválido: {id}");
                assert!(!domain.is_empty() && !reason.is_empty(), "regra vazia em {dep}");
            }
        }
        for domain in DOMAINS {
            for (kind, id, _, _) in domain.essentials {
                assert!(*kind == "repo" || *kind == "model");
                assert_eq!(id.split('/').count(), 2, "essencial inválido: {id}");
            }
        }
    }
}
