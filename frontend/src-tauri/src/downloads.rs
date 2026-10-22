use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use reqwest::blocking::Client;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Job {
    pub id: String,
    pub label: String,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub finished: bool,
    pub success: bool,
    pub message: String,
    pub folder_path: String,
}

// ponytail: fila global em memoria; some ao fechar o app (o que ja foi gravado em disco fica)
static JOBS: LazyLock<Mutex<BTreeMap<String, Job>>> = LazyLock::new(Default::default);

pub fn list() -> Vec<Job> {
    JOBS.lock().unwrap().values().cloned().collect()
}

pub fn is_running(id: &str) -> bool {
    JOBS.lock().unwrap().get(id).is_some_and(|j| !j.finished)
}

pub fn try_start(id: &str, label: &str, total: u64, folder: &Path) -> Result<(), String> {
    let mut jobs = JOBS.lock().unwrap();
    if jobs.get(id).is_some_and(|j| !j.finished) {
        return Err("Este download ja esta em andamento.".into());
    }
    jobs.insert(
        id.into(),
        Job {
            id: id.into(),
            label: label.into(),
            total_bytes: total,
            folder_path: folder.display().to_string(),
            ..Default::default()
        },
    );
    Ok(())
}

fn update(id: &str, f: impl FnOnce(&mut Job)) {
    if let Some(j) = JOBS.lock().unwrap().get_mut(id) {
        f(j);
    }
}

pub fn finish(id: &str, success: bool, message: String) {
    update(id, |j| {
        j.finished = true;
        j.success = success;
        j.message = message;
        j.current_file.clear();
    });
}

/// Baixa `url` para `dest` em streaming (sem carregar o arquivo na memoria), somando o progresso no job.
/// Grava em `.part` e renomeia no fim: arquivo pela metade nunca fica com o nome final.
pub fn fetch_to_file(
    http: &Client,
    job: &str,
    url: &str,
    dest: &Path,
    auth: Option<&str>,
) -> Result<(), String> {
    if crate::local::has_reparse_ancestor(dest) || dest.exists() {
        return Err(format!(
            "Destino ja existe ou e inseguro: {}",
            dest.display()
        ));
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Nao foi possivel criar {}: {e}", dir.display()))?;
    }
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    update(job, |j| j.current_file = name.clone());
    let mut req = http
        .get(url)
        .timeout(std::time::Duration::from_secs(60 * 60));
    if let Some(t) = auth {
        req = req.bearer_auth(t);
    }
    let mut resp = req.send().map_err(|e| format!("{name}: {e}"))?;
    match resp.status().as_u16() {
        200 => {}
        401 | 403 => {
            return Err(format!(
                "{name}: acesso negado (modelo restrito ou pendente de aprovação manual no Hugging Face)"
            ));
        }
        404 => return Err(format!("{name}: arquivo não encontrado")),
        s => return Err(format!("{name}: servidor respondeu {s}")),
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = dest.with_file_name(format!(
        ".gtr-{}-{}.part",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("{name}: {e}"))?;
    let result = (|| {
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = resp
                .read(&mut buf)
                .map_err(|e| format!("{name}: download interrompido ({e})"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("{name}: falha ao gravar ({e})"))?;
            update(job, |j| {
                j.done_bytes = j.done_bytes.saturating_add(n as u64)
            });
        }
        out.sync_all().map_err(|e| format!("{name}: {e}"))?;
        drop(out);
        if crate::local::has_reparse_ancestor(dest) {
            return Err(format!("{name}: destino inseguro"));
        }
        // A hard link publishes the completed file atomically, failing if dest exists.
        // Unlike rename, it can never replace a user's existing file.
        std::fs::hard_link(&tmp, dest).map_err(|e| format!("{name}: {e}"))
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn streams_and_preserves_existing_files_and_cleans_partial() {
        let root = std::env::temp_dir().join(format!("gtr_fetch_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (body, length) in [(b"hello".as_slice(), 5), (b"short".as_slice(), 100)] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let client = Client::builder().no_proxy().build().unwrap();
        let dest = root.join("model.bin");
        try_start("test-stream", "test", 5, &root).unwrap();
        assert!(try_start("test-stream", "duplicate", 5, &root).is_err());
        fetch_to_file(&client, "test-stream", &url, &dest, None).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        assert!(fetch_to_file(&client, "test-stream", &url, &dest, None).is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        let broken = root.join("broken.bin");
        assert!(fetch_to_file(&client, "test-stream", &url, &broken, None).is_err());
        assert!(!broken.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        server.join().unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }
}
