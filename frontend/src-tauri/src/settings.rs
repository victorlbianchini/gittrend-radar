use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub github_dir: String,
    pub hf_dir: String,
    pub skills_dir: String,
    pub github_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            github_dir: r"E:\REPOSITORIOS".into(),
            hf_dir: r"E:\REPOSITORIOS\HuggingFace".into(),
            skills_dir: r"E:\REPOSITORIOS\Skills".into(),
            github_token: String::new(),
        }
    }
}

/// O que a tela recebe: nunca o token inteiro.
#[derive(Serialize)]
pub struct PublicSettings {
    pub github_dir: String,
    pub hf_dir: String,
    pub skills_dir: String,
    pub token_hint: String,
}

// ponytail: estado global simples; o app tem uma janela e um arquivo de config
static STATE: LazyLock<RwLock<(PathBuf, Settings)>> =
    LazyLock::new(|| RwLock::new((PathBuf::new(), Settings::default())));

pub fn init(file: PathBuf) {
    let s = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
        .map(|mut s| {
            if let Some(encoded) = s.github_token.strip_prefix("dpapi:") {
                s.github_token = unprotect_token(encoded).unwrap_or_default();
            }
            s
        })
        .unwrap_or_default();
    *STATE.write().unwrap() = (file, s);
}

pub fn get() -> Settings {
    STATE.read().unwrap().1.clone()
}

pub fn public() -> PublicSettings {
    let s = get();
    let t = s.github_token.trim();
    PublicSettings {
        token_hint: if t.is_empty() {
            String::new()
        } else if t.chars().count() <= 4 {
            "\u{2022}\u{2022}\u{2022}\u{2022}".into()
        } else {
            format!(
                "••••{}",
                t.chars()
                    .skip(t.chars().count().saturating_sub(4))
                    .collect::<String>()
            )
        },
        github_dir: s.github_dir,
        hf_dir: s.hf_dir,
        skills_dir: s.skills_dir,
    }
}

fn valid_dir(p: &str) -> Result<String, String> {
    let p = p.trim();
    if !Path::new(p).is_absolute() {
        return Err(format!(
            "Use um caminho completo, como E:\\PASTA (recebido: \"{p}\")."
        ));
    }
    std::fs::create_dir_all(p).map_err(|e| format!("Não foi possível usar a pasta {p}: {e}"))?;
    Ok(p.to_string())
}

/// `token`: None mantém o atual, Some("") remove, Some(x) troca.
pub fn save(
    github_dir: &str,
    hf_dir: &str,
    skills_dir: &str,
    token: Option<String>,
) -> Result<PublicSettings, String> {
    let mut new = get();
    new.github_dir = valid_dir(github_dir)?;
    new.hf_dir = valid_dir(hf_dir)?;
    new.skills_dir = valid_dir(skills_dir)?;
    if let Some(t) = token {
        let t = t.trim().to_string();
        if !t.is_empty() && !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("Token do GitHub com formato inválido.".into());
        }
        new.github_token = t;
    }
    let mut st = STATE.write().unwrap();
    let mut stored = new.clone();
    if !stored.github_token.is_empty() {
        stored.github_token = format!("dpapi:{}", protect_token(&stored.github_token)?);
    }
    let data = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
    persist(&st.0, &data)?;
    st.1 = new;
    drop(st);
    Ok(public())
}

#[repr(C)]
struct DataBlob {
    len: u32,
    data: *mut u8,
}

#[link(name = "crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        input: *const DataBlob,
        description: *const u16,
        entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void,
        prompt: *mut std::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *const DataBlob,
        description: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void,
        prompt: *mut std::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

fn dpapi(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    let input = DataBlob {
        len: u32::try_from(bytes.len()).map_err(|_| "Token muito longo")?,
        data: bytes.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        len: 0,
        data: std::ptr::null_mut(),
    };
    // DPAPI binds the secret to this Windows user. UI is forbidden for background saves.
    // Input lives for the entire call; Windows owns output until LocalFree below.
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err("Windows nao conseguiu proteger ou recuperar o token.".into());
    }
    let result = if output.len == 0 {
        Vec::new()
    } else {
        // Successful DPAPI calls provide output.len readable bytes.
        unsafe { std::slice::from_raw_parts(output.data, output.len as usize).to_vec() }
    };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(result)
}

fn protect_token(token: &str) -> Result<String, String> {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in dpapi(token.as_bytes(), true)? {
        write!(encoded, "{byte:02x}").unwrap();
    }
    Ok(encoded)
}

fn unprotect_token(encoded: &str) -> Result<String, String> {
    if encoded.len() % 2 != 0 || !encoded.is_ascii() {
        return Err("Token protegido invalido.".into());
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&encoded[i..i + 2], 16)
                .map_err(|_| "Token protegido invalido.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(dpapi(&bytes, false)?).map_err(|_| "Token protegido invalido.".into())
}

fn persist(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = path.with_file_name(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    let _ = std::fs::remove_file(&tmp);
    result.map_err(|e| format!("Falha ao salvar configuracoes: {e}"))
}

pub fn github_dir() -> PathBuf {
    PathBuf::from(get().github_dir)
}
pub fn hf_dir() -> PathBuf {
    PathBuf::from(get().hf_dir)
}
pub fn skills_dir() -> PathBuf {
    PathBuf::from(get().skills_dir)
}

/// Retorna o token do Hugging Face em ordem de prioridade:
/// 1. Variável de ambiente HF_TOKEN
/// 2. Arquivo E:\REPOSITORIOS\AI_MODELS\token
/// 3. Cache oficial do Hugging Face (%USERPROFILE%\.cache\huggingface\token)
pub fn hf_token() -> Option<String> {
    if let Ok(t) = std::env::var("HF_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let models_token = Path::new(r"E:\REPOSITORIOS\AI_MODELS\token");
    if let Ok(t) = std::fs::read_to_string(models_token) {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        let cache_token = PathBuf::from(home)
            .join(".cache")
            .join("huggingface")
            .join("token");
        if let Ok(t) = std::fs::read_to_string(cache_token) {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_validates_and_masks_token() {
        let dir = std::env::temp_dir().join(format!("gtr_settings_{}", std::process::id()));
        init(dir.join("settings.json"));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.display().to_string();
        assert!(save("relativo", &d, &d, None).is_err());
        assert!(save(&d, &d, &d, Some("token com espaço".into())).is_err());
        let p = save(
            &d,
            &dir.join("hf").display().to_string(),
            &d,
            Some("ghp_ABCDEF1234".into()),
        )
        .unwrap();
        assert_eq!(p.token_hint, "••••1234");
        assert!(dir.join("hf").is_dir());
        let disk = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!disk.contains("ghp_ABCDEF1234"));
        assert!(disk.contains("dpapi:"));
        init(dir.join("settings.json"));
        assert_eq!(get().github_token, "ghp_ABCDEF1234");
        assert!(unprotect_token("zz").is_err());
        assert!(unprotect_token("abc").is_err());
        assert!(valid_dir(r"C:\").is_ok());
        assert!(
            persist(
                &dir.join("settings.json"),
                "{\"github_token\":\"\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\"}".as_bytes()
            )
            .is_ok()
        );
        init(dir.join("settings.json"));
        assert_eq!(get().github_token, "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}");
        assert_eq!(
            public().token_hint,
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{e9}\u{e9}\u{e9}\u{e9}"
        );
        STATE.write().unwrap().1.github_token = "abc".into();
        assert_eq!(public().token_hint, "\u{2022}\u{2022}\u{2022}\u{2022}");
        STATE.write().unwrap().1.github_token = "ghp_ABCDEF1234".into();
        // None mantem o token
        save(&d, &d, &d, None).unwrap();
        assert_eq!(get().github_token, "ghp_ABCDEF1234");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
