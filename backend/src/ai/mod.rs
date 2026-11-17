//! Integrasi AI lewat CLI (tanpa API key di aplikasi).
//!
//! `AiProvider` punya tiga implementasi: `ClaudeCli` (`claude -p ... --json-schema`),
//! `CodexCli` (`codex exec --output-schema ...`) dan `MockAi` (deterministik, untuk test /
//! kalau CLI tidak tersedia). Proses dijalankan langsung (tanpa `sh -c`), prompt dikirim
//! lewat stdin (bukan argumen command line), dengan timeout dan batas proses paralel (Semaphore).

pub mod mock;
pub mod numcheck;
pub mod tools;

use crate::error::AppResult;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::SqlitePool;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::Semaphore;

/// Konteks tambahan supaya provider mock bisa menjawab tanpa memahami bahasa alami penuh.
#[derive(Debug, Clone)]
pub enum AiPurpose {
    SelectTool { question: String, reference_date: chrono::NaiveDate },
    Summarize { result: tools::ToolResult },
    Explain { facts: tools::ToolResult },
}

impl AiPurpose {
    pub fn label(&self) -> &'static str {
        match self {
            AiPurpose::SelectTool { .. } => "select_tool",
            AiPurpose::Summarize { .. } => "summarize",
            AiPurpose::Explain { .. } => "explain",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiRequest {
    pub purpose: AiPurpose,
    pub prompt: String,
    pub schema: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("CLI AI tidak bisa dijalankan: {0}")]
    Spawn(String),
    #[error("CLI AI melewati batas waktu {0} detik")]
    Timeout(u64),
    #[error("CLI AI gagal: {0}")]
    Cli(String),
    #[error("Output AI tidak sesuai schema: {0}")]
    InvalidOutput(String),
}

/// Output mentah (untuk log) + JSON terstruktur.
pub type AiOutput = Result<(String, Value), AiError>;
pub type BoxFut<'a> = Pin<Box<dyn Future<Output = AiOutput> + Send + 'a>>;

pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete<'a>(&'a self, req: &'a AiRequest) -> BoxFut<'a>;
}

/// Jalankan proses langsung (tanpa shell). Prompt dikirim lewat stdin supaya input user tidak
/// pernah menjadi bagian dari command line (aman juga untuk `claude.cmd` di Windows).
async fn run_command(mut cmd: Command, stdin_data: &str, timeout_secs: u64) -> Result<std::process::Output, AiError> {
    use tokio::io::AsyncWriteExt;
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| AiError::Spawn(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data.as_bytes()).await.map_err(|e| AiError::Spawn(e.to_string()))?;
        drop(stdin);
    }
    match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(AiError::Spawn(e.to_string())),
        Err(_) => Err(AiError::Timeout(timeout_secs)),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

pub struct ClaudeCli {
    pub bin: String,
    pub timeout_secs: u64,
}

impl AiProvider for ClaudeCli {
    fn name(&self) -> &'static str {
        "claude"
    }
    fn complete<'a>(&'a self, req: &'a AiRequest) -> BoxFut<'a> {
        Box::pin(async move {
            let schema = serde_json::to_string(&req.schema).map_err(|e| AiError::InvalidOutput(e.to_string()))?;
            let mut cmd = Command::new(&self.bin);
            // `claude -p` tanpa argumen prompt membaca prompt dari stdin
            cmd.arg("-p").arg("--output-format").arg("json").arg("--json-schema").arg(&schema).arg("--tools").arg("");
            let out = run_command(cmd, &req.prompt, self.timeout_secs).await?;
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if stdout.trim().is_empty() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(AiError::Cli(truncate(stderr.trim(), 400)));
            }
            let v: Value = serde_json::from_str(stdout.trim()).map_err(|e| AiError::InvalidOutput(format!("stdout bukan JSON: {e}")))?;
            if v.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                let msg = v.get("result").and_then(Value::as_str).unwrap_or("is_error = true");
                return Err(AiError::Cli(truncate(msg, 400)));
            }
            let structured = match v.get("structured_output") {
                Some(s) if !s.is_null() => s.clone(),
                _ => {
                    // cadangan: sebagian versi CLI menaruh JSON di field `result`
                    let r = v.get("result").and_then(Value::as_str).unwrap_or_default();
                    serde_json::from_str(r).map_err(|_| AiError::InvalidOutput("structured_output kosong".into()))?
                }
            };
            Ok((stdout, structured))
        })
    }
}

pub struct CodexCli {
    pub bin: String,
    pub timeout_secs: u64,
}

impl AiProvider for CodexCli {
    fn name(&self) -> &'static str {
        "codex"
    }
    fn complete<'a>(&'a self, req: &'a AiRequest) -> BoxFut<'a> {
        Box::pin(async move {
            let dir = tempfile::tempdir().map_err(|e| AiError::Spawn(e.to_string()))?;
            let schema_path = dir.path().join("schema.json");
            let out_path = dir.path().join("out.json");
            let schema = serde_json::to_vec(&req.schema).map_err(|e| AiError::InvalidOutput(e.to_string()))?;
            tokio::fs::write(&schema_path, schema).await.map_err(|e| AiError::Spawn(e.to_string()))?;
            let mut cmd = Command::new(&self.bin);
            cmd.arg("exec")
                .arg("--skip-git-repo-check")
                .arg("-s")
                .arg("read-only")
                .arg("--output-schema")
                .arg(&schema_path)
                .arg("-o")
                .arg(&out_path)
                .arg("-") // prompt dibaca dari stdin
                .current_dir(dir.path());
            let out = run_command(cmd, &req.prompt, self.timeout_secs).await?;
            let text = tokio::fs::read_to_string(&out_path).await.map_err(|_| {
                let stderr = String::from_utf8_lossy(&out.stderr);
                AiError::Cli(format!("out.json tidak dibuat. {}", truncate(stderr.trim(), 400)))
            })?;
            let v: Value = serde_json::from_str(text.trim()).map_err(|e| AiError::InvalidOutput(format!("out.json bukan JSON: {e}")))?;
            Ok((text, v))
        })
    }
}

/// Layanan AI: membatasi paralelisme, mencoba ulang (maks. 2x), memvalidasi dengan serde,
/// dan mencatat setiap panggilan ke tabel `ai_calls`.
pub struct AiService {
    pub provider: Box<dyn AiProvider>,
    pub semaphore: Semaphore,
    pub db: SqlitePool,
}

impl AiService {
    pub fn new(provider: Box<dyn AiProvider>, max_parallel: usize, db: SqlitePool) -> Self {
        AiService { provider, semaphore: Semaphore::new(max_parallel.max(1)), db }
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    async fn log(&self, req: &AiRequest, raw: Option<&str>, err: Option<String>, ms: u128) -> AppResult<()> {
        sqlx::query("INSERT INTO ai_calls(provider, purpose, prompt, raw_output, ok, error, duration_ms) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(self.provider.name())
            .bind(req.purpose.label())
            .bind(&req.prompt)
            .bind(raw)
            .bind(err.is_none())
            .bind(err)
            .bind(ms as i64)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Panggil AI dan deserialisasi ke `T`. Gagal (CLI error / JSON tidak sesuai) -> coba ulang maks. 2x.
    pub async fn call<T: DeserializeOwned>(&self, req: &AiRequest) -> Result<T, AiError> {
        let mut last_err = AiError::Cli("tidak ada percobaan".into());
        for attempt in 0..3 {
            let permit = self.semaphore.acquire().await.map_err(|e| AiError::Spawn(e.to_string()))?;
            let started = Instant::now();
            let res = self.provider.complete(req).await;
            drop(permit);
            let ms = started.elapsed().as_millis();
            let parsed = match res {
                Ok((raw, v)) => match serde_json::from_value::<T>(v) {
                    Ok(t) => {
                        let _ = self.log(req, Some(&raw), None, ms).await;
                        return Ok(t);
                    }
                    Err(e) => {
                        let err = AiError::InvalidOutput(e.to_string());
                        let _ = self.log(req, Some(&raw), Some(err.to_string()), ms).await;
                        err
                    }
                },
                Err(e) => {
                    let _ = self.log(req, None, Some(e.to_string()), ms).await;
                    e
                }
            };
            tracing::warn!(attempt, error = %parsed, "panggilan AI gagal");
            // timeout / CLI tidak ada tidak akan membaik dengan dicoba ulang
            if matches!(parsed, AiError::Spawn(_) | AiError::Timeout(_)) {
                return Err(parsed);
            }
            last_err = parsed;
        }
        Err(last_err)
    }
}

/// Cari binary CLI yang bisa dijalankan (`<bin> --version`). Di Windows juga dicoba `.cmd`/`.exe`.
async fn resolve_bin(bin: &str) -> Option<(String, String)> {
    let mut candidates = vec![bin.to_string()];
    if cfg!(windows) && !bin.ends_with(".cmd") && !bin.ends_with(".exe") {
        candidates.push(format!("{bin}.cmd"));
        candidates.push(format!("{bin}.exe"));
    }
    for c in candidates {
        let mut cmd = Command::new(&c);
        cmd.arg("--version");
        if let Ok(out) = run_command(cmd, "", 20).await {
            if out.status.success() {
                return Some((c, String::from_utf8_lossy(&out.stdout).trim().to_string()));
            }
        }
    }
    None
}

/// Pilih provider dari AI_PROVIDER. Kalau CLI tidak ditemukan, jatuh ke mock (kecuali AI_STRICT=true)
/// supaya aplikasi tetap bisa dijalankan reviewer.
pub async fn build_provider(cfg: &crate::config::Config) -> Box<dyn AiProvider> {
    let (name, bin) = match cfg.ai_provider.as_str() {
        "claude" => ("claude", cfg.claude_bin.clone()),
        "codex" => ("codex", cfg.codex_bin.clone()),
        _ => return Box::new(mock::MockAi),
    };
    match resolve_bin(&bin).await {
        Some((path, version)) => {
            tracing::info!(provider = name, bin = %path, %version, "CLI AI ditemukan");
            if name == "claude" {
                Box::new(ClaudeCli { bin: path, timeout_secs: cfg.ai_timeout_secs })
            } else {
                Box::new(CodexCli { bin: path, timeout_secs: cfg.ai_timeout_secs })
            }
        }
        None if std::env::var("AI_STRICT").map(|v| v == "true").unwrap_or(false) => {
            tracing::error!(provider = name, bin = %bin, "CLI AI tidak ditemukan (AI_STRICT=true), panggilan AI akan gagal");
            if name == "claude" {
                Box::new(ClaudeCli { bin, timeout_secs: cfg.ai_timeout_secs })
            } else {
                Box::new(CodexCli { bin, timeout_secs: cfg.ai_timeout_secs })
            }
        }
        None => {
            tracing::warn!(provider = name, bin = %bin, "CLI AI tidak ditemukan, memakai provider mock. Set CLAUDE_BIN/CODEX_BIN atau AI_PROVIDER=mock");
            Box::new(mock::MockAi)
        }
    }
}
