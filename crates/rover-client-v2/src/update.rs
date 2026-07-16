use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

use iced::Task;
use rover_core::{ConnectionProfile, ConnectionProfileStore};
use tokio_stream::StreamExt;

use crate::api::client::RoverClient;
use crate::app::{ClientRef, DeployJob, RoverApp, Screen, ServerState, Toast, ToastKind};
use crate::message::Message;

/// Handle every UI event and async result.
pub fn update(app: &mut RoverApp, message: Message) -> Task<Message> {
    match message {
        Message::Noop => Task::none(),
        Message::Tick => tick(app),

        // ── Server management ──────────────────────────────────────────
        Message::ManageServers => {
            app.show_manage_servers = true;
            app.show_add_form = false;
            Task::none()
        }
        Message::CloseManageServers => {
            app.show_manage_servers = false;
            Task::none()
        }
        Message::ShowAddForm => {
            app.show_add_form = true;
            app.show_manage_servers = false;
            Task::none()
        }
        Message::HideAddForm => {
            app.show_add_form = false;
            app.addr_input.clear();
            app.token_input.clear();
            app.name_input.clear();
            app.form_error = None;
            Task::none()
        }
        Message::SetAddr(value) => {
            app.addr_input = value;
            Task::none()
        }
        Message::SetToken(value) => {
            app.token_input = value;
            Task::none()
        }
        Message::SetServerName(value) => {
            app.name_input = value;
            Task::none()
        }
        Message::Connect => connect_server(app),
        Message::ServerAdded(name, client, api_key) => server_added(app, name, client, api_key),
        Message::ServerAddError(error) => {
            app.form_error = Some(error);
            Task::none()
        }
        Message::ServerConnected(idx, client) => {
            if let Some(server) = app.servers.get_mut(idx) {
                server.connected = client.is_some();
                server.client = client;
                server.connecting = false;
                server.error = None;
                if server.connected {
                    let mut tasks: Vec<Task<Message>> = Vec::new();
                    tasks.push(refresh_data(app, idx));
                    tasks.push(refresh_apps(app, idx));
                    return Task::batch(tasks);
                }
            }
            Task::none()
        }
        Message::ServerError(idx, err) => {
            if let Some(server) = app.servers.get_mut(idx) {
                server.connecting = false;
                server.error = Some(err.clone());
            }
            app.toasts = vec![Toast {
                message: format!("Server connection error: {err}"),
                kind: ToastKind::Error,
            }];
            Task::none()
        }
        Message::Disconnect(idx) => {
            if let Some(server) = app.servers.get_mut(idx) {
                server.connected = false;
                server.client = None;
                server.info = None;
                server.metrics = None;
                server.apps.clear();
                server.error = None;
            }
            app.rebuild_all_apps();
            if let Screen::AppDetail(_, si) = &app.screen {
                if *si == idx {
                    app.screen = Screen::Dashboard;
                    app.app_detail = None;
                    app.log_entries.clear();
                }
            }
            Task::none()
        }
        Message::Reconnect(idx) => {
            if let Some(server) = app.servers.get_mut(idx) {
                if !server.connected && !server.connecting && server.profile.api_key.is_some() {
                    server.connecting = true;
                    server.error = None;
                    return reconnect_server(app, idx);
                }
            }
            Task::none()
        }
        Message::StartRename(idx) => {
            if let Some(server) = app.servers.get(idx) {
                app.rename_value = server.profile.name.clone();
                app.editing_server = Some(idx);
            }
            Task::none()
        }
        Message::SetRenameValue(value) => {
            app.rename_value = value;
            Task::none()
        }
        Message::ConfirmRename(idx) => {
            let new_name = app.rename_value.trim().to_string();
            if !new_name.is_empty() && idx < app.servers.len() {
                app.servers[idx].profile.name = new_name.clone();
                let mut store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
                store.upsert(app.servers[idx].profile.clone());
                let _ = store.save_to_disk();
            }
            app.editing_server = None;
            app.rename_value.clear();
            app.rebuild_all_apps();
            Task::none()
        }
        Message::CancelRename => {
            app.editing_server = None;
            app.rename_value.clear();
            Task::none()
        }
        Message::ConfirmServerDelete(idx) => {
            if app.servers.get(idx).is_some() {
                app.confirm_server_delete = Some(idx);
            }
            Task::none()
        }
        Message::CancelServerDelete => {
            app.confirm_server_delete = None;
            Task::none()
        }
        Message::DeleteServer(idx) => delete_server(app, idx),

        // ── Data refresh ───────────────────────────────────────────────
        Message::ServerData(idx, info, metrics) => {
            if let Some(server) = app.servers.get_mut(idx) {
                server.info = Some(*info);
                server.metrics = Some(*metrics);
            }
            Task::none()
        }
        Message::ServerApps(idx, apps_list) => {
            // Auto-deselect if the viewed app no longer exists
            if let Screen::AppDetail(ref app_id, si) = app.screen {
                if si == idx && !apps_list.iter().any(|a| a.app_id == *app_id) {
                    app.screen = Screen::Dashboard;
                    app.app_detail = None;
                    app.log_entries.clear();
                }
            }
            if let Some(server) = app.servers.get_mut(idx) {
                server.apps = apps_list;
            }
            app.rebuild_all_apps();
            Task::none()
        }

        // ── App detail ────────────────────────────────────────────────
        Message::SelectApp(app_id, server_index) => {
            app.screen = Screen::AppDetail(app_id.clone(), server_index);
            app.app_detail_server = server_index;
            app.app_detail = None;
            app.log_entries.clear();
            fetch_detail(app, &app_id, server_index)
        }
        Message::AppDetail(detail) => {
            app.app_detail = Some(*detail);
            Task::none()
        }
        Message::LogLines(lines) => {
            app.log_entries = lines;
            Task::none()
        }
        Message::BackToDashboard => {
            app.screen = Screen::Dashboard;
            app.app_detail = None;
            app.log_entries.clear();
            Task::none()
        }
        Message::StartApp(app_id, si) => app_action(app, &app_id, si, "start"),
        Message::StopApp(app_id, si) => app_action(app, &app_id, si, "stop"),
        Message::RestartApp(app_id, si) => app_action(app, &app_id, si, "restart"),
        Message::DeleteApp(app_id, si) => {
            if let Some(detail) = &app.app_detail {
                if detail.app_id == app_id {
                    app.confirm_delete = Some((app_id, detail.name.clone(), si));
                }
            }
            Task::none()
        }
        Message::CancelDelete => {
            app.confirm_delete = None;
            Task::none()
        }
        Message::ConfirmDelete(app_id, _name, si) => confirm_delete_app(app, app_id, si),

        // ── Deploy ────────────────────────────────────────────────────
        Message::OpenDeploy => {
            app.deploy_open = true;
            app.deploy_target = None;
            app.deploy_name.clear();
            app.deploy_runtime.clear();
            app.deploy_build.clear();
            app.deploy_run.clear();
            app.deploy_path.clear();
            app.deploy_use_github = false;
            app.deploy_github_url.clear();
            app.deploy_env_vars.clear();
            app.deploy_env_key.clear();
            app.deploy_env_value.clear();
            Task::none()
        }
        Message::CloseDeploy => {
            app.deploy_open = false;
            // refresh all
            refresh_all(app)
        }
        Message::SetDeployTarget(idx) => {
            app.deploy_target = idx;
            Task::none()
        }
        Message::SetDeployName(value) => {
            app.deploy_name = value;
            Task::none()
        }
        Message::SetDeployRuntime(value) => {
            app.deploy_runtime = value.clone();
            if !value.is_empty() {
                let runtime: rover_core::Runtime = match value.parse() {
                    Ok(r) => r,
                    Err(_) => return Task::none(),
                };
                app.deploy_build = runtime.default_build_command().to_string();
                app.deploy_run = runtime.default_run_command().to_string();
            }
            Task::none()
        }
        Message::SetDeployBuild(value) => {
            app.deploy_build = value;
            Task::none()
        }
        Message::SetDeployRun(value) => {
            app.deploy_run = value;
            Task::none()
        }
        Message::ToggleGithub => {
            app.deploy_use_github = !app.deploy_use_github;
            Task::none()
        }
        Message::SetDeployGithubUrl(value) => {
            app.deploy_github_url = value;
            Task::none()
        }
        Message::SelectGithubToken(label) => {
            app.selected_github_token = label;
            Task::none()
        }
        Message::SetNewTokenLabel(value) => {
            app.new_token_label = value;
            Task::none()
        }
        Message::SetNewTokenValue(value) => {
            app.new_token_value = value;
            Task::none()
        }
        Message::SaveGithubToken => {
            let label = app.new_token_label.trim().to_string();
            let value = app.new_token_value.trim().to_string();
            if !label.is_empty() && !value.is_empty() {
                let token = crate::app::GithubToken::new(label.clone(), value);
                app.selected_github_token = Some(label);
                app.github_tokens.push(token);
                app.new_token_label.clear();
                app.new_token_value.clear();
                save_github_tokens(app);
            }
            Task::none()
        }
        Message::SetDeployPath(value) => {
            app.deploy_path = value;
            Task::none()
        }
        Message::SetEnvKey(value) => {
            app.deploy_env_key = value;
            Task::none()
        }
        Message::SetEnvValue(value) => {
            app.deploy_env_value = value;
            Task::none()
        }
        Message::AddEnvVar => {
            let key = app.deploy_env_key.trim().to_string();
            let value = app.deploy_env_value.trim().to_string();
            if !key.is_empty() {
                app.deploy_env_vars.push((key, value));
                app.deploy_env_key.clear();
                app.deploy_env_value.clear();
            }
            Task::none()
        }
        Message::RemoveEnvVar(i) => {
            if i < app.deploy_env_vars.len() {
                app.deploy_env_vars.remove(i);
            }
            Task::none()
        }
        Message::PickPath => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .pick_folder()
                    .await
                    .map(|handle| handle.path().to_string_lossy().to_string())
            },
            |result| match result {
                Some(path) => Message::SetDeployPath(path),
                None => Message::Noop,
            },
        ),
        Message::PickEnvFile => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .pick_file()
                    .await
                    .map(|handle| handle.path().to_string_lossy().to_string())
                    .and_then(|path| {
                        std::fs::read_to_string(&path).ok().map(|contents| {
                            let vars: Vec<(String, String)> = contents
                                .lines()
                                .filter_map(|line| {
                                    let line = line.trim();
                                    if line.is_empty() || line.starts_with('#') {
                                        return None;
                                    }
                                    let (k, v) = line.split_once('=')?;
                                    Some((k.trim().to_string(), v.trim().to_string()))
                                })
                                .collect();
                            (path, vars)
                        })
                    })
            },
            |result| match result {
                Some((path, vars)) => Message::EnvFilePicked(path, vars),
                None => Message::Noop,
            },
        ),
        Message::EnvFilePicked(_path, vars) => {
            let count = vars.len();
            for (k, v) in vars {
                app.deploy_env_vars.push((k, v));
            }
            app.toasts = vec![Toast {
                message: format!("Imported {count} env vars from .env file"),
                kind: ToastKind::Info,
            }];
            Task::none()
        }

        Message::SubmitDeploy => submit_deploy(app),
        Message::DeployStatus(deploy_id, status) => {
            if let Some(job) = app.find_deploy_mut(deploy_id) {
                job.status = status.clone();
                job.logs.push(format!("[{status}]"));
            }
            Task::none()
        }
        Message::DeployEvent(deploy_id, event) => deploy_event(app, deploy_id, event),
        Message::DeployStreamEnded(deploy_id) => deploy_stream_ended(app, deploy_id),
        Message::DeployError(deploy_id, error) => deploy_failed(app, deploy_id, error),
        Message::ToggleDeployLog(deploy_id) => {
            app.expanded_deploy = if app.expanded_deploy == Some(deploy_id) {
                None
            } else {
                Some(deploy_id)
            };
            Task::none()
        }
        Message::ClearFinishedDeploys => {
            app.deploy_jobs.retain(DeployJob::is_active);
            if let Some(expanded) = app.expanded_deploy {
                if !app.deploy_jobs.iter().any(|j| j.id == expanded) {
                    app.expanded_deploy = None;
                }
            }
            Task::none()
        }

        // ── Update commands ───────────────────────────────────────────
        Message::OpenUpdate(_app_id) => {
            if let Some(detail) = &app.app_detail {
                app.update_build = detail.build_command.clone();
                app.update_run = detail.run_command.clone();
            }
            app.update_open = true;
            Task::none()
        }
        Message::CloseUpdate => {
            app.update_open = false;
            Task::none()
        }
        Message::SetUpdateBuild(value) => {
            app.update_build = value;
            Task::none()
        }
        Message::SetUpdateRun(value) => {
            app.update_run = value;
            Task::none()
        }
        Message::ConfirmUpdate(app_id) => {
            app.update_open = false;
            let si = app.app_detail_server;
            let client = app.client_for(si);
            let aid = app_id;
            let build = app.update_build.trim().to_string();
            let run = app.update_run.trim().to_string();
            Task::perform(
                async move {
                    if let Some(client) = client {
                        let mut client = client.lock().await;
                        client
                            .update_app(
                                &aid,
                                if build.is_empty() { None } else { Some(build) },
                                if run.is_empty() { None } else { Some(run) },
                            )
                            .await
                            .map(Box::new)
                            .map_err(|e| e.to_string())
                    } else {
                        Err("Not connected".into())
                    }
                },
                move |result| {
                    if let Ok(detail) = result {
                        Message::AppDetail(detail)
                    } else {
                        Message::Error(result.unwrap_err())
                    }
                },
            )
        }

        // ── Notifications ─────────────────────────────────────────────
        Message::Info(message) => {
            app.toasts = vec![Toast {
                message,
                kind: ToastKind::Info,
            }];
            Task::none()
        }
        Message::Error(message) => {
            app.toasts = vec![Toast {
                message,
                kind: ToastKind::Error,
            }];
            Task::none()
        }
        Message::DismissToast(i) => {
            if i < app.toasts.len() {
                app.toasts.remove(i);
            }
            Task::none()
        }
        Message::Copy(text) => iced::clipboard::write(text),

        // ── Terminal ──────────────────────────────────────────────────
        Message::OpenTerminal(si) => {
            app.terminal_open = true;
            app.terminal_server = si;
            app.terminal_output.clear();
            app.terminal_input.clear();
            app.screen = Screen::Terminal(si);

            let client = app.client_for(si);
            let name = app.server_name_for(si);
            let (input_tx, input_rx) =
                tokio::sync::mpsc::channel::<rover_proto::v1::ShellInput>(64);
            let buffer = Arc::new(StdMutex::new(Vec::<String>::new()));

            app.terminal_sender = Some(input_tx);
            app.terminal_buffer = buffer.clone();

            // Spawn a persistent background task that reads shell output
            // into the shared buffer. The UI reads from buffer on each tick.
            tokio::spawn(async move {
                let c = match client {
                    Some(c) => c,
                    None => return,
                };
                let mut c = c.lock().await;
                let mut stream = match c.system_shell(input_rx).await {
                    Ok(s) => s,
                    Err(_) => return,
                };
                drop(c);
                use tokio_stream::StreamExt;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(output) => {
                            if let Ok(text) = String::from_utf8(output.data) {
                                let mut buf = buffer.lock().unwrap();
                                for line in text.lines() {
                                    buf.push(line.to_string());
                                }
                                if buf.len() > 500 {
                                    let trim = buf.len() - 500;
                                    buf.drain(0..trim);
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            Task::done(Message::Info(format!("Shell opened on {name}")))
        }
        Message::ShellStarted(_) => Task::none(),
        Message::ShellOutput(data) => Task::none(), // Handled via buffer in tick
        Message::ShellClosed => Task::none(),       // Stream ended, handled in bg task
        Message::SetTerminalInput(input) => {
            app.terminal_input = input;
            Task::none()
        }
        Message::SubmitShellCommand => {
            let cmd = app.terminal_input.clone();
            app.terminal_input.clear();
            if let Some(ref tx) = app.terminal_sender {
                let mut data = cmd.into_bytes();
                data.push(b'\n');
                let _ = tx.try_send(rover_proto::v1::ShellInput { data });
            }
            Task::none()
        }
        Message::CloseTerminal => {
            app.terminal_open = false;
            app.terminal_sender = None;
            app.terminal_output.clear();
            app.terminal_input.clear();
            app.terminal_buffer = Arc::new(StdMutex::new(Vec::new()));
            app.screen = Screen::Dashboard;
            Task::none()
        }
    }
}

// ── Connection flow ──────────────────────────────────────────────────────────

fn reconnect_server(app: &mut RoverApp, idx: usize) -> Task<Message> {
    let server = &mut app.servers[idx];
    let addr = server.profile.address.clone();
    let key = server.profile.api_key.clone().unwrap_or_default();

    Task::perform(
        async move {
            let mut client = RoverClient::connect(&addr)
                .await
                .map_err(|e| e.to_string())?;
            client.set_api_key(&key);
            client.get_info().await.map_err(|e| e.to_string())?;
            Ok(Arc::new(Mutex::new(client)))
        },
        move |result| match result {
            Ok(client) => Message::ServerConnected(idx, Some(client)),
            Err(e) => Message::ServerError(idx, e),
        },
    )
}

fn connect_server(app: &mut RoverApp) -> Task<Message> {
    let addr = app.addr_input.trim().to_string();
    let token = app.token_input.trim().to_string();
    let name = if app.name_input.trim().is_empty() {
        addr.clone()
    } else {
        app.name_input.trim().to_string()
    };

    if addr.is_empty() || token.is_empty() {
        app.form_error = Some("Address and pairing token are required".into());
        return Task::none();
    }

    app.form_error = None;
    let name_for_result = name.clone();
    Task::perform(
        async move {
            let mut client = RoverClient::connect(&addr)
                .await
                .map_err(|e| e.to_string())?;
            let pair_resp = client.pair(&token).await.map_err(|e| e.to_string())?;
            let api_key = pair_resp.api_key;
            let client = Arc::new(Mutex::new(client));
            Ok((name_for_result, client, api_key))
        },
        |result| match result {
            Ok((name, client, api_key)) => Message::ServerAdded(name, client, api_key),
            Err(e) => Message::ServerAddError(e),
        },
    )
}

fn server_added(
    app: &mut RoverApp,
    name: String,
    client: ClientRef,
    api_key: String,
) -> Task<Message> {
    let addr = app.addr_input.trim().to_string();
    let mut profile = ConnectionProfile::new(name, addr);
    profile.api_key = Some(api_key);
    profile.last_used = Some(chrono::Utc::now());

    let mut store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
    store.upsert(profile.clone());
    let _ = store.save_to_disk();

    let idx = app.servers.len();
    app.servers.push(ServerState {
        profile,
        client: Some(client),
        connected: true,
        connecting: false,
        error: None,
        info: None,
        metrics: None,
        apps: Vec::new(),
    });
    app.show_add_form = false;
    app.addr_input.clear();
    app.token_input.clear();
    app.name_input.clear();
    app.form_error = None;
    app.screen = Screen::Dashboard;
    app.app_detail = None;
    app.log_entries.clear();

    Task::batch([refresh_data(app, idx), refresh_apps(app, idx)])
}

fn delete_server(app: &mut RoverApp, idx: usize) -> Task<Message> {
    app.confirm_server_delete = None;
    if idx < app.servers.len() {
        let profile_id = app.servers[idx].profile.id.clone();
        if let Ok(mut store) = ConnectionProfileStore::load_from_disk() {
            store.remove(&profile_id);
            let _ = store.save_to_disk();
        }
        app.servers.remove(idx);
        // Fix up screen state
        if let Screen::AppDetail(_, si) = &app.screen {
            if *si == idx {
                app.screen = Screen::Dashboard;
                app.app_detail = None;
                app.log_entries.clear();
            }
        }
    }
    app.rebuild_all_apps();
    Task::none()
}

fn confirm_delete_app(app: &mut RoverApp, app_id: String, si: usize) -> Task<Message> {
    app.confirm_delete = None;
    let client = app.client_for(si);
    let aid = app_id;
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                client.delete_app(&aid).await.map_err(|e| e.to_string())
            } else {
                Err("Not connected".into())
            }
        },
        move |result| match result {
            Ok(()) => Message::BackToDashboard,
            Err(e) => Message::Error(format!("Delete failed: {e}")),
        },
    )
}

// ── Periodic tick — polls all connected servers ──────────────────────────────

fn tick(app: &mut RoverApp) -> Task<Message> {
    let mut tasks: Vec<Task<Message>> = Vec::new();

    for (idx, server) in app.servers.iter().enumerate() {
        if server.connected {
            tasks.push(refresh_data(app, idx));
            tasks.push(refresh_apps(app, idx));
        }
    }

    // Also refresh logs for the viewed app
    if let Screen::AppDetail(ref app_id, si) = app.screen.clone() {
        tasks.push(fetch_detail(app, app_id, si));
        tasks.push(fetch_logs(app, app_id, si));
    }

    // Copy terminal buffer to output
    if app.terminal_open {
        let mut buf = app.terminal_buffer.lock().unwrap();
        app.terminal_output.clone_from(&buf);
    }

    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

// ── Refresh all connected servers ────────────────────────────────────────────

fn refresh_all(app: &RoverApp) -> Task<Message> {
    let mut tasks: Vec<Task<Message>> = Vec::new();
    for (idx, server) in app.servers.iter().enumerate() {
        if server.connected {
            tasks.push(refresh_data(app, idx));
            tasks.push(refresh_apps(app, idx));
        }
    }
    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

// ── Data refresh tasks ───────────────────────────────────────────────────────

fn refresh_data(app: &RoverApp, idx: usize) -> Task<Message> {
    let client = app.client_for(idx);
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                let info = client.get_info().await.map_err(|e| e.to_string())?;
                let metrics = client.get_metrics().await.map_err(|e| e.to_string())?;
                Ok((Box::new(info), Box::new(metrics)))
            } else {
                Err(String::from("Not connected"))
            }
        },
        move |result: Result<_, String>| match result {
            Ok((info, metrics)) => Message::ServerData(idx, info, metrics),
            Err(e) => Message::Error(e),
        },
    )
}

fn refresh_apps(app: &RoverApp, idx: usize) -> Task<Message> {
    let client = app.client_for(idx);
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                client.list_apps(100, 0).await
            } else {
                Err(String::from("Not connected"))
            }
        },
        move |result: Result<_, String>| match result {
            Ok(apps) => Message::ServerApps(idx, apps),
            Err(e) => Message::Error(e),
        },
    )
}

fn fetch_detail(app: &RoverApp, app_id: &str, si: usize) -> Task<Message> {
    let client = app.client_for(si);
    let aid = app_id.to_string();
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                client.get_app(&aid).await.map(Box::new)
            } else {
                Err(String::from("Not connected"))
            }
        },
        move |result: Result<_, String>| match result {
            Ok(detail) => Message::AppDetail(detail),
            Err(e) => Message::Error(e),
        },
    )
}

fn fetch_logs(app: &RoverApp, app_id: &str, si: usize) -> Task<Message> {
    let client = app.client_for(si);
    let aid = app_id.to_string();
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                let mut stream = client
                    .stream_logs(&aid, 50)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut lines = Vec::new();
                while let Some(Ok(entry)) = stream.next().await {
                    let ts = entry.timestamp.map_or_else(
                        || String::from("--:--:--"),
                        |t| {
                            let secs = t.millis / 1000;
                            let h = (secs / 3600) % 24;
                            let m = (secs / 60) % 60;
                            let s = secs % 60;
                            format!("{h:02}:{m:02}:{s:02}")
                        },
                    );
                    lines.push(format!("{ts} {}", entry.line));
                    if lines.len() >= 100 {
                        break;
                    }
                }
                Ok(lines)
            } else {
                Err(String::from("Not connected"))
            }
        },
        move |result: Result<_, String>| match result {
            Ok(lines) => Message::LogLines(lines),
            Err(e) => Message::Error(e),
        },
    )
}

fn app_action(app: &RoverApp, app_id: &str, si: usize, action: &str) -> Task<Message> {
    let client = app.client_for(si);
    let aid = app_id.to_string();
    let action = action.to_string();
    Task::perform(
        async move {
            if let Some(client) = client {
                let mut client = client.lock().await;
                match action.as_str() {
                    "start" => client.start_app(&aid).await.map(Box::new),
                    "stop" => client.stop_app(&aid).await.map(Box::new),
                    "restart" => client.restart_app(&aid).await.map(Box::new),
                    _ => Err(String::from("unknown action")),
                }
            } else {
                Err(String::from("Not connected"))
            }
        },
        move |result: Result<_, String>| match result {
            Ok(detail) => Message::AppDetail(detail),
            Err(e) => Message::Error(e),
        },
    )
}

// ── Deploy flow ──────────────────────────────────────────────────────────────

fn submit_deploy(app: &mut RoverApp) -> Task<Message> {
    let use_github = app.deploy_use_github && !app.deploy_github_url.trim().is_empty();
    let Some(target) = app.deploy_target else {
        return Task::done(Message::Error("Select a deployment target server".into()));
    };
    if (!use_github && app.deploy_path.is_empty())
        || app.deploy_name.trim().is_empty()
        || app.deploy_runtime.is_empty()
    {
        return Task::none();
    }

    let deploy_id = app.next_deploy_id;
    app.next_deploy_id += 1;

    let name = app.deploy_name.trim().to_string();
    let runtime = app.deploy_runtime.clone();
    let build_cmd = app.deploy_build.trim().to_string();
    let run_cmd = app.deploy_run.trim().to_string();
    let source_path = if app.deploy_use_github {
        String::new()
    } else {
        app.deploy_path.clone()
    };
    let github_url = if app.deploy_use_github && !app.deploy_github_url.trim().is_empty() {
        Some(app.deploy_github_url.trim().to_string())
    } else {
        None
    };
    let github_token = if app.deploy_use_github {
        app.selected_github_token.as_ref().and_then(|label| {
            app.github_tokens
                .iter()
                .find(|t| &t.label == label)
                .map(|t| t.token.clone())
        })
    } else {
        None
    };
    let env_vars = app.deploy_env_vars.clone();
    let client = app.client_for(target);
    let server_name = app.server_name_for(target);

    app.deploy_jobs.push(DeployJob {
        id: deploy_id,
        name: name.clone(),
        runtime: runtime.clone(),
        source_path: source_path.clone(),
        status: String::from("packaging"),
        logs: vec![String::from("Packaging source...")],
        app_id: None,
        error: None,
        server_index: target,
        server_name: server_name.clone(),
    });
    app.expanded_deploy = Some(deploy_id);
    app.deploy_open = false;

    Task::batch([
        Task::done(Message::Info(format!("Deploying {name} to {server_name}"))),
        start_deploy_task(
            deploy_id,
            client,
            name,
            runtime,
            build_cmd,
            run_cmd,
            source_path,
            env_vars,
            github_url,
            github_token,
        ),
    ])
}

fn start_deploy_task(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
    github_url: Option<String>,
    github_token: Option<String>,
) -> Task<Message> {
    let (tx, rx) = tokio::sync::mpsc::channel(128);

    tokio::spawn(async move {
        run_deploy(
            deploy_id,
            client,
            name,
            runtime,
            build_cmd,
            run_cmd,
            source_path,
            env_vars,
            github_url,
            github_token,
            tx,
        )
        .await;
    });

    Task::run(tokio_stream::wrappers::ReceiverStream::new(rx), |m| m)
}

async fn run_deploy(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
    github_url: Option<String>,
    github_token: Option<String>,
    tx: tokio::sync::mpsc::Sender<Message>,
) {
    let result = run_deploy_inner(
        deploy_id,
        client,
        name,
        runtime,
        build_cmd,
        run_cmd,
        source_path,
        env_vars,
        github_url,
        github_token,
        tx.clone(),
    )
    .await;
    if let Err(e) = result {
        let _ = tx.send(Message::DeployError(deploy_id, e)).await;
    }
}

async fn run_deploy_inner(
    deploy_id: usize,
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
    github_url: Option<String>,
    github_token: Option<String>,
    tx: tokio::sync::mpsc::Sender<Message>,
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

    let source_bytes = if github_url.is_none() {
        package_source(&source_path).await?
    } else {
        Vec::new()
    };

    tx.send(Message::DeployStatus(deploy_id, String::from("sending")))
        .await
        .ok();

    let req = rover_proto::v1::DeployRequest {
        name,
        runtime: runtime_proto,
        manifest_toml,
        source_archive: source_bytes,
        github_url,
        github_token,
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

fn deploy_event(
    app: &mut RoverApp,
    deploy_id: usize,
    event: rover_proto::v1::DeployEvent,
) -> Task<Message> {
    let mut tasks = Vec::new();
    let mut refresh = false;
    if let Some(job) = app.find_deploy_mut(deploy_id) {
        if let Some(line) = format_deploy_event(&event) {
            job.logs.push(line);
        }
        match &event.event {
            Some(rover_proto::v1::deploy_event::Event::Complete(complete)) => {
                job.status = String::from("complete");
                job.app_id = Some(complete.app_id.clone());
                tasks.push(Task::done(Message::Info(format!("Deployed {}", job.name))));
                refresh = true;
            }
            Some(rover_proto::v1::deploy_event::Event::Error(err)) => {
                job.status = String::from("failed");
                job.error = Some(err.message.clone());
                tasks.push(Task::done(Message::Error(format!(
                    "Deploy {} failed: {}",
                    job.name, err.message
                ))));
                refresh = true;
            }
            Some(rover_proto::v1::deploy_event::Event::Progress(progress)) => {
                job.status = progress.stage.clone();
            }
            _ => {}
        }
    }
    if refresh {
        tasks.push(refresh_all(app));
    }
    Task::batch(tasks)
}

fn deploy_stream_ended(app: &mut RoverApp, deploy_id: usize) -> Task<Message> {
    let mut tasks = vec![refresh_all(app)];
    if let Some(job) = app.find_deploy_mut(deploy_id) {
        if job.is_active() {
            let msg = String::from("deploy stream ended unexpectedly");
            job.status = String::from("failed");
            job.error = Some(msg.clone());
            job.logs.push(format!("❌ {msg}"));
            tasks.push(Task::done(Message::Error(format!(
                "Deploy {} failed: {msg}",
                job.name
            ))));
        }
    }
    Task::batch(tasks)
}

fn deploy_failed(app: &mut RoverApp, deploy_id: usize, error: String) -> Task<Message> {
    if let Some(job) = app.find_deploy_mut(deploy_id) {
        job.status = String::from("failed");
        job.error = Some(error.clone());
        job.logs.push(format!("❌ {error}"));
    }
    Task::batch([
        Task::done(Message::Error(format!("Deploy failed: {error}"))),
        refresh_all(app),
    ])
}

fn format_deploy_event(event: &rover_proto::v1::DeployEvent) -> Option<String> {
    match &event.event {
        Some(rover_proto::v1::deploy_event::Event::Log(log)) => {
            if log.is_stderr {
                Some(format!("[err] {}", log.line))
            } else {
                Some(log.line.clone())
            }
        }
        Some(rover_proto::v1::deploy_event::Event::Complete(complete)) => {
            Some(format!("✅ Deployed — {}", complete.app_id))
        }
        Some(rover_proto::v1::deploy_event::Event::Error(err)) => {
            Some(format!("❌ {}", err.message))
        }
        Some(rover_proto::v1::deploy_event::Event::Progress(progress)) => {
            Some(format!("[{:.0}%] {}", progress.percent, progress.stage))
        }
        None => None,
    }
}

// ── Source packaging ─────────────────────────────────────────────────────────

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
    let always_ignore_suffix: &[&str] = &[".sqlite", ".sqlite3", ".db", ".duckdb", ".env"];

    let mut archive = tar::Builder::new(Vec::new());
    let root_rules = parse_gitignore(path);
    let base = path.to_path_buf();
    walk(
        &base,
        &base,
        &mut archive,
        always_ignore,
        always_ignore_suffix,
        &root_rules,
    )?;

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
    Exact(String, bool),
    Suffix(String, bool),
    Prefix(String, bool),
}

impl GitignoreRules {
    fn empty() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    fn matches(&self, name: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for pattern in &self.patterns {
            let hit = match pattern {
                GitignorePattern::Exact(value, _) => name == *value,
                GitignorePattern::Suffix(glob, _) => name.ends_with(glob),
                GitignorePattern::Prefix(glob, _) => {
                    is_dir && (name.starts_with(glob) || name == glob.trim_end_matches('/'))
                }
            };
            if hit {
                ignored = match pattern {
                    GitignorePattern::Exact(_, neg)
                    | GitignorePattern::Suffix(_, neg)
                    | GitignorePattern::Prefix(_, neg) => !neg,
                };
            }
        }
        ignored
    }
}

fn parse_gitignore(dir: &std::path::Path) -> GitignoreRules {
    let path = dir.join(".gitignore");
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return GitignoreRules::empty(),
    };
    let mut patterns = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let negated = trimmed.starts_with('!');
        let effective = if negated {
            trimmed[1..].trim()
        } else {
            trimmed
        };
        let effective = effective.strip_prefix('/').unwrap_or(effective);
        let pattern = if effective.ends_with('/') {
            GitignorePattern::Prefix(effective.to_string(), negated)
        } else if effective.starts_with("*.") {
            GitignorePattern::Suffix(effective[1..].to_string(), negated)
        } else {
            GitignorePattern::Exact(effective.to_string(), negated)
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
    always_ignore_suffix: &[&str],
    parent_rules: &GitignoreRules,
) -> Result<(), String> {
    let local_rules = parse_gitignore(dir);
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let is_dir = path.is_dir();

        if always_ignore.contains(&name.as_ref())
            || always_ignore_suffix.iter().any(|s| name.ends_with(&**s))
        {
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
            walk(
                &path,
                base,
                archive,
                always_ignore,
                always_ignore_suffix,
                parent_rules,
            )?;
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

// ── GitHub token persistence ─────────────────────────────────────────────────

fn save_github_tokens(app: &RoverApp) {
    let path = github_tokens_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let store = GithubTokenStore {
        tokens: app.github_tokens.clone(),
    };
    if let Ok(data) = serde_json::to_string_pretty(&store) {
        let _ = std::fs::write(&path, data);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct GithubTokenStore {
    tokens: Vec<crate::app::GithubToken>,
}

fn github_tokens_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join(".config")
        .join("rover")
        .join("github-tokens.json")
}

pub fn load_github_tokens() -> Vec<crate::app::GithubToken> {
    let path = github_tokens_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<GithubTokenStore>(&s).ok())
            .map(|store| store.tokens)
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}
