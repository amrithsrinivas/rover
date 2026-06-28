use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use iced::Task;

use crate::message::{ClientRef, Message};

/// Default build command for a runtime selected in the deploy form.
pub fn default_build_for(runtime: &str) -> &'static str {
    match runtime {
        "python" => "pip install -r requirements.txt",
        "node" => "npm install",
        "go" => "go build -o app .",
        "rust" => "cargo build --release",
        _ => "",
    }
}

/// Default run command for a runtime selected in the deploy form.
pub fn default_run_for(runtime: &str) -> &'static str {
    match runtime {
        "python" => "python main.py",
        "node" => "node index.js",
        "go" => "./app",
        "rust" => "./target/release/app",
        _ => "",
    }
}

/// Start a background deploy and return a task that forwards progress messages to Iced.
pub fn start_deploy_task(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
) -> Task<Message> {
    let (tx, rx) = mpsc::channel(128);

    tokio::spawn(async move {
        run_deploy_task(
            deploy_id,
            client,
            name,
            runtime,
            build_cmd,
            run_cmd,
            source_path,
            env_vars,
            tx,
        )
        .await;
    });

    Task::run(tokio_stream::wrappers::ReceiverStream::new(rx), |message| {
        message
    })
}

async fn run_deploy_task(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
    tx: mpsc::Sender<Message>,
) {
    let result = run_deploy_task_inner(
        deploy_id,
        client,
        name,
        runtime,
        build_cmd,
        run_cmd,
        source_path,
        env_vars,
        tx.clone(),
    )
    .await;

    if let Err(e) = result {
        let _ = tx.send(Message::DeployErr(deploy_id, e)).await;
    }
}

async fn run_deploy_task_inner(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
    tx: mpsc::Sender<Message>,
) -> Result<(), String> {
    let c = client.ok_or_else(|| String::from("Not connected"))?;

    let runtime_proto = match runtime.as_str() {
        "python" => 1i32,
        "node" => 2,
        "go" => 3,
        "rust" => 4,
        _ => return Err(format!("Unknown runtime: {runtime}")),
    };

    let mut manifest_map = toml::map::Map::new();

    let mut app_section = toml::map::Map::new();
    app_section.insert("name".into(), toml::Value::String(name.clone()));
    app_section.insert("runtime".into(), toml::Value::String(runtime.clone()));
    manifest_map.insert("app".into(), toml::Value::Table(app_section));

    let mut build_section = toml::map::Map::new();
    build_section.insert("command".into(), toml::Value::String(build_cmd));
    manifest_map.insert("build".into(), toml::Value::Table(build_section));

    let mut run_section = toml::map::Map::new();
    run_section.insert("command".into(), toml::Value::String(run_cmd));
    manifest_map.insert("run".into(), toml::Value::Table(run_section));

    if !env_vars.is_empty() {
        let mut env_section = toml::map::Map::new();
        for (key, value) in &env_vars {
            env_section.insert(key.clone(), toml::Value::String(value.clone()));
        }
        manifest_map.insert("env".into(), toml::Value::Table(env_section));
    }

    let manifest_toml = toml::to_string_pretty(&toml::Value::Table(manifest_map))
        .map_err(|e| format!("TOML serialization error: {e}"))?;

    let source_bytes = package_source(&source_path).await?;

    tx.send(Message::DeployStatus(deploy_id, String::from("sending")))
        .await
        .ok();

    let req = rover_proto::v1::DeployRequest {
        name,
        runtime: runtime_proto,
        manifest_toml,
        source_archive: source_bytes,
    };

    let mut stream = {
        let mut c = c.lock().await;
        c.deploy_stream(req).await?
    };

    tx.send(Message::DeployStatus(deploy_id, String::from("building")))
        .await
        .ok();

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => {
                let _ = tx.send(Message::DeployEvent(deploy_id, ev)).await;
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    let _ = tx.send(Message::DeployStreamEnded(deploy_id)).await;
    Ok(())
}

/// Recursively package a directory as tar.gz bytes, respecting `.gitignore` files.
async fn package_source(path: &str) -> Result<Vec<u8>, String> {
    let path = std::path::Path::new(path);
    if !path.is_dir() {
        return Err("Source path is not a directory".into());
    }

    let always_ignore: &[&str] = &[
        ".git",
        "target",
        "node_modules",
        "__pycache__",
        ".venv",
        "venv",
        ".DS_Store",
    ];

    let mut archive = tar::Builder::new(Vec::new());
    let root_rules = parse_gitignore(path);
    let base = path.to_path_buf();

    walk(&base, &base, &mut archive, always_ignore, &root_rules)?;

    let tar_bytes = archive
        .into_inner()
        .map_err(|e| format!("tar finalize error: {e}"))?;

    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&tar_bytes)
        .map_err(|e| format!("gzip write error: {e}"))?;
    encoder
        .finish()
        .map_err(|e| format!("gzip finish error: {e}"))
}

#[derive(Debug, Clone)]
struct GitignoreRules {
    patterns: Vec<GitignorePattern>,
}

#[derive(Debug, Clone)]
enum GitignorePattern {
    Exact(String),
    Suffix(String),
    Prefix(String),
}

impl GitignoreRules {
    fn empty() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    fn matches(&self, name: &str, is_dir: bool) -> bool {
        for pattern in &self.patterns {
            match pattern {
                GitignorePattern::Exact(value) => {
                    if name == value {
                        return true;
                    }
                }
                GitignorePattern::Suffix(glob) => {
                    if name.ends_with(glob) {
                        return true;
                    }
                }
                GitignorePattern::Prefix(glob) => {
                    if is_dir && name.starts_with(glob) {
                        return true;
                    }
                    if is_dir && name == glob.trim_end_matches('/') {
                        return true;
                    }
                }
            }
        }
        false
    }
}

fn parse_gitignore(dir: &std::path::Path) -> GitignoreRules {
    let path = dir.join(".gitignore");
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(_) => return GitignoreRules::empty(),
    };

    let mut patterns = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let pattern = if trimmed.ends_with('/') {
            GitignorePattern::Prefix(trimmed.to_string())
        } else if trimmed.starts_with("*.") {
            GitignorePattern::Suffix(trimmed[1..].to_string())
        } else {
            GitignorePattern::Exact(trimmed.to_string())
        };
        patterns.push(pattern);
    }

    GitignoreRules { patterns }
}

fn walk(
    dir: &std::path::Path,
    base: &std::path::Path,
    archive: &mut tar::Builder<Vec<u8>>,
    always_ignore: &[&str],
    parent_rules: &GitignoreRules,
) -> Result<(), String> {
    let local_rules = parse_gitignore(dir);

    for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let is_dir = path.is_dir();

        if always_ignore.contains(&name.as_ref()) {
            continue;
        }

        if is_dir && name.starts_with('.') {
            continue;
        }

        if local_rules.matches(&name, is_dir) || parent_rules.matches(&name, is_dir) {
            continue;
        }

        let relative = path
            .strip_prefix(base)
            .map_err(|e| format!("strip_prefix: {e}"))?;

        if is_dir {
            let dir_path = format!("{}/", relative.to_string_lossy());
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            archive
                .append_data(&mut header, dir_path, &mut std::io::empty())
                .map_err(|e| format!("tar append dir error: {e}"))?;
            walk(&path, base, archive, always_ignore, parent_rules)?;
        } else if path.is_file() {
            let data = std::fs::read(&path).map_err(|e| format!("read file: {e}"))?;
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            archive
                .append_data(
                    &mut header,
                    relative.to_string_lossy().as_ref(),
                    &mut std::io::Cursor::new(data),
                )
                .map_err(|e| format!("tar append file error: {e}"))?;
        }
    }
    Ok(())
}
