use std::sync::Arc;
use tokio::sync::Mutex;

use iced::Task;
use rover_core::{ConnectionProfile, ConnectionProfileStore};
use tokio_stream::StreamExt;

use crate::api::client::RoverClient;
use crate::app::{DeployState, RoverApp, ToastKind, ToastState};
use crate::deploy_job::{default_build_for, default_run_for};
use crate::message::{ClientRef, Message};
use crate::state::DeviceState;

/// Handle every UI event and async result.
pub fn update(app: &mut RoverApp, message: Message) -> Task<Message> {
    match message {
        Message::Noop => Task::none(),
        Message::Tick => tick(app),

        Message::Select(i) => {
            if i < app.devices.len() {
                app.active = i;
                app.selected_app = None;
                app.app_detail = None;
                app.log_entries.clear();
                let device = &app.devices[i];
                if !device.connected && !device.connecting && device.profile.api_key.is_some() {
                    return reconnect_device(app, i);
                }
            }
            Task::none()
        }
        Message::ShowAdd => {
            app.show_add = true;
            Task::none()
        }
        Message::HideAdd => {
            app.show_add = false;
            app.addr.clear();
            app.token.clear();
            app.name.clear();
            app.error = None;
            Task::none()
        }
        Message::SetAddr(value) => {
            app.addr = value;
            Task::none()
        }
        Message::SetToken(value) => {
            app.token = value;
            Task::none()
        }
        Message::SetName(value) => {
            app.name = value;
            Task::none()
        }
        Message::Connect => connect_device(app),
        Message::DevAdded(name, client, api_key) => device_added(app, name, client, api_key),
        Message::DevAddErr(error) => {
            app.error = Some(error);
            Task::none()
        }
        Message::DevConnected(idx, client) => {
            if let Some(device) = app.devices.get_mut(idx) {
                device.connected = client.is_some();
                device.client = client;
                device.connecting = false;
                device.err = None;
                if device.connected {
                    return Task::batch([refresh_data(app), refresh_apps(app)]);
                }
            }
            Task::none()
        }
        Message::DevError(idx, err) => {
            let name = if let Some(device) = app.devices.get_mut(idx) {
                device.connecting = false;
                device.err = Some(err.clone());
                device.profile.name.clone()
            } else {
                String::from("unknown")
            };
            if app.active == idx {
                app.toasts = vec![ToastState {
                    message: format!("Connection error ({name}): {err}"),
                    kind: ToastKind::Error,
                }];
            }
            Task::none()
        }
        Message::Disconnect => {
            if let Some(device) = app.devices.get_mut(app.active) {
                device.connected = false;
                device.client = None;
                device.info = None;
                device.metrics = None;
                device.apps.clear();
                device.err = None;
            }
            app.selected_app = None;
            app.app_detail = None;
            app.log_entries.clear();
            Task::none()
        }
        Message::StartRename(idx) => {
            if let Some(device) = app.devices.get(idx) {
                app.rename_value = device.profile.name.clone();
                app.editing_device = Some(idx);
            }
            Task::none()
        }
        Message::SetRenameValue(value) => {
            app.rename_value = value;
            Task::none()
        }
        Message::ConfirmRename(idx) => {
            let new_name = app.rename_value.trim().to_string();
            if !new_name.is_empty() && idx < app.devices.len() {
                app.devices[idx].profile.name = new_name.clone();
                let mut store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
                store.upsert(app.devices[idx].profile.clone());
                let _ = store.save_to_disk();
            }
            app.editing_device = None;
            app.rename_value.clear();
            Task::none()
        }
        Message::CancelRename => {
            app.editing_device = None;
            app.rename_value.clear();
            Task::none()
        }
        Message::DeleteDevice(idx) => {
            if app.devices.get(idx).is_some() {
                app.confirm_device_delete = Some(idx);
            }
            Task::none()
        }
        Message::CancelDeleteDevice => {
            app.confirm_device_delete = None;
            Task::none()
        }
        Message::ConfirmDeleteDevice(idx) => confirm_delete_device(app, idx),

        Message::Data(info, metrics) => {
            if let Some(device) = app.devices.get_mut(app.active) {
                device.info = Some(*info);
                device.metrics = Some(*metrics);
            }
            Task::none()
        }
        Message::Apps(apps_list) => {
            if let Some(device) = app.devices.get_mut(app.active) {
                if let Some(ref selected) = app.selected_app {
                    if !apps_list.iter().any(|app| app.app_id == *selected) {
                        app.selected_app = None;
                        app.app_detail = None;
                        app.log_entries.clear();
                    }
                }
                device.apps = apps_list;
            }
            Task::none()
        }

        Message::SelectApp(app_id) => {
            app.selected_app = Some(app_id.clone());
            app.app_detail = None;
            app.log_entries.clear();
            fetch_detail(app, &app_id)
        }
        Message::Detail(detail) => {
            app.app_detail = Some(*detail);
            Task::none()
        }
        Message::Logs(lines) => {
            app.log_entries = lines;
            Task::none()
        }
        Message::Back => {
            app.selected_app = None;
            app.app_detail = None;
            app.log_entries.clear();
            Task::none()
        }
        Message::Start(app_id) => app_action(app, &app_id, "start"),
        Message::Stop(app_id) => app_action(app, &app_id, "stop"),
        Message::Restart(app_id) => app_action(app, &app_id, "restart"),
        Message::Delete(app_id) => {
            if let Some(detail) = &app.app_detail {
                if detail.app_id == app_id {
                    app.confirm_delete = Some((app_id, detail.name.clone()));
                }
            }
            Task::none()
        }
        Message::CancelDelete => {
            app.confirm_delete = None;
            Task::none()
        }
        Message::ConfirmDelete(app_id, _name) => confirm_delete_app(app, app_id),

        Message::OpenDeploy => {
            app.deploy_open = true;
            app.deploy_name.clear();
            app.deploy_runtime.clear();
            app.deploy_build.clear();
            app.deploy_run.clear();
            app.deploy_path.clear();
            app.deploy_use_github = false;
            app.deploy_github_url.clear();
            app.deploy_env_file.clear();
            app.deploy_env_vars.clear();
            app.deploy_env_key.clear();
            app.deploy_env_value.clear();
            Task::none()
        }
        Message::CloseDeploy => {
            app.deploy_open = false;
            refresh_apps(app)
        }
        Message::SetDName(value) => {
            app.deploy_name = value;
            Task::none()
        }
        Message::SetDRuntime(value) => {
            app.deploy_runtime = value.clone();
            if !value.is_empty() {
                app.deploy_build = default_build_for(&value).to_string();
                app.deploy_run = default_run_for(&value).to_string();
            }
            Task::none()
        }
        Message::SetDBuild(value) => {
            app.deploy_build = value;
            Task::none()
        }
        Message::SetDRun(value) => {
            app.deploy_run = value;
            Task::none()
        }
        Message::ToggleGithub => {
            app.deploy_use_github = !app.deploy_use_github;
            Task::none()
        }
        Message::SetDGithubUrl(value) => {
            app.deploy_github_url = value;
            Task::none()
        }
        Message::SelectGithubToken(idx) => {
            app.selected_github_token = idx;
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
                let token = crate::github_tokens::GithubToken::new(label.clone(), value);
                app.selected_github_token = Some(label);
                app.github_tokens.push(token);
                app.new_token_label.clear();
                app.new_token_value.clear();
                let store = crate::github_tokens::GithubTokenStore {
                    tokens: app.github_tokens.clone(),
                };
                store.save();
            }
            Task::none()
        }
        Message::SetDPath(value) => {
            app.deploy_path = value;
            Task::none()
        }
        Message::SetDEnvFile(value) => {
            app.deploy_env_file = value;
            Task::none()
        }
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
        Message::EnvFilePicked(path, vars) => {
            app.deploy_env_file = path;
            let count = vars.len();
            for (k, v) in vars {
                app.deploy_env_vars.push((k, v));
            }
            app.toasts = vec![ToastState {
                message: format!("Imported {count} env vars"),
                kind: ToastKind::Info,
            }];
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
                Some(path) => Message::SetDPath(path),
                None => Message::Noop,
            },
        ),
        Message::SetDEKey(value) => {
            app.deploy_env_key = value;
            Task::none()
        }
        Message::SetDEValue(value) => {
            app.deploy_env_value = value;
            Task::none()
        }
        Message::AddDEVar => {
            let key = app.deploy_env_key.trim().to_string();
            let value = app.deploy_env_value.trim().to_string();
            if !key.is_empty() {
                app.deploy_env_vars.push((key, value));
                app.deploy_env_key.clear();
                app.deploy_env_value.clear();
            }
            Task::none()
        }
        Message::RemoveDEVar(i) => {
            if i < app.deploy_env_vars.len() {
                app.deploy_env_vars.remove(i);
            }
            Task::none()
        }

        Message::SubmitDeploy => crate::deploy_update::submit_deploy(app),
        Message::DeployStatus(deploy_id, status) => {
            if let Some(deploy) = app.find_deploy_mut(deploy_id) {
                deploy.status = status.clone();
                deploy.logs.push(format!("[{status}]"));
            }
            Task::none()
        }
        Message::DeployEvent(deploy_id, event) => {
            crate::deploy_update::deploy_event(app, deploy_id, event)
        }
        Message::DeployStreamEnded(deploy_id) => {
            crate::deploy_update::deploy_stream_ended(app, deploy_id)
        }
        Message::DeployErr(deploy_id, error) => {
            crate::deploy_update::deploy_failed(app, deploy_id, error)
        }
        Message::ToggleDeployLog(deploy_id) => {
            app.expanded_deploy = if app.expanded_deploy == Some(deploy_id) {
                None
            } else {
                Some(deploy_id)
            };
            Task::none()
        }
        Message::ClearFinishedDeploys => {
            app.active_deploys.retain(DeployState::is_active);
            if let Some(expanded) = app.expanded_deploy {
                if !app
                    .active_deploys
                    .iter()
                    .any(|deploy| deploy.id == expanded)
                {
                    app.expanded_deploy = None;
                }
            }
            Task::none()
        }

        // --- Update commands modal ---
        Message::OpenUpdate(_app_id) => {
            // Prefill from current detail
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
            let client = get_client(app);
            let aid = app_id;
            let dev_name = active_device_name(app);
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
                move |result| match result {
                    Ok(detail) => Message::Detail(detail),
                    Err(e) => Message::Toast(format!("{dev_name}: {e}")),
                },
            )
        }

        Message::Info(message) => {
            app.toasts = vec![ToastState {
                message,
                kind: ToastKind::Info,
            }];
            Task::none()
        }
        Message::Toast(message) => {
            app.toasts = vec![ToastState {
                message,
                kind: ToastKind::Error,
            }];
            Task::none()
        }
        Message::Dismiss(i) => {
            if i < app.toasts.len() {
                app.toasts.remove(i);
            }
            Task::none()
        }
        Message::Copy(text) => iced::clipboard::write(text),
    }
}

fn connect_device(app: &mut RoverApp) -> Task<Message> {
    let addr = app.addr.trim().to_string();
    let token = app.token.trim().to_string();
    let name = if app.name.trim().is_empty() {
        addr.clone()
    } else {
        app.name.trim().to_string()
    };

    if addr.is_empty() || token.is_empty() {
        app.error = Some("Address and pairing token are required".into());
        return Task::none();
    }

    app.error = None;
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
            Ok((name, client, api_key)) => Message::DevAdded(name, client, api_key),
            Err(e) => Message::DevAddErr(e),
        },
    )
}

fn device_added(
    app: &mut RoverApp,
    name: String,
    client: ClientRef,
    api_key: String,
) -> Task<Message> {
    let addr = app.addr.trim().to_string();
    let mut profile = ConnectionProfile::new(name, addr);
    profile.api_key = Some(api_key);
    profile.last_used = Some(chrono::Utc::now());

    let mut store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
    store.upsert(profile.clone());
    let _ = store.save_to_disk();

    app.devices.push(DeviceState {
        profile,
        client: Some(client),
        connected: true,
        info: None,
        metrics: None,
        apps: Vec::new(),
        connecting: false,
        err: None,
    });
    app.active = app.devices.len() - 1;
    app.show_add = false;
    app.addr.clear();
    app.token.clear();
    app.name.clear();
    app.error = None;
    app.selected_app = None;
    app.app_detail = None;
    app.log_entries.clear();

    Task::batch([refresh_data(app), refresh_apps(app)])
}

fn confirm_delete_device(app: &mut RoverApp, idx: usize) -> Task<Message> {
    app.confirm_device_delete = None;
    if idx < app.devices.len() {
        let profile_id = app.devices[idx].profile.id.clone();
        if let Ok(mut store) = ConnectionProfileStore::load_from_disk() {
            store.remove(&profile_id);
            let _ = store.save_to_disk();
        }
        app.devices.remove(idx);
        if app.devices.is_empty() {
            app.active = 0;
            app.selected_app = None;
            app.app_detail = None;
        } else if app.active >= app.devices.len() {
            app.active = app.devices.len() - 1;
        }
    }
    Task::none()
}

fn confirm_delete_app(app: &RoverApp, app_id: String) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id;
    let dev_name = active_device_name(app);
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
            Ok(()) => Message::Back,
            Err(e) => Message::Toast(format!("{dev_name}: Delete failed: {e}")),
        },
    )
}

pub(crate) fn get_client(app: &RoverApp) -> Option<ClientRef> {
    app.devices
        .get(app.active)
        .and_then(|device| device.client.clone())
}

fn active_device_name(app: &RoverApp) -> String {
    app.devices
        .get(app.active)
        .map(|device| device.profile.name.clone())
        .unwrap_or_else(|| "unknown".into())
}

fn tick(app: &mut RoverApp) -> Task<Message> {
    if let Some(device) = app.devices.get(app.active) {
        if device.connected {
            let mut tasks = vec![refresh_data(app), refresh_apps(app)];
            if let Some(ref app_id) = app.selected_app.clone() {
                tasks.push(fetch_detail(app, app_id));
                tasks.push(fetch_logs(app, app_id));
            }
            return Task::batch(tasks);
        }
    }
    Task::none()
}

fn refresh_data(app: &RoverApp) -> Task<Message> {
    let client = get_client(app);
    let dev_name = active_device_name(app);
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
            Ok((info, metrics)) => Message::Data(info, metrics),
            Err(e) => Message::Toast(format!("{dev_name}: {e}")),
        },
    )
}

pub(crate) fn refresh_apps(app: &RoverApp) -> Task<Message> {
    let client = get_client(app);
    let dev_name = active_device_name(app);
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
            Ok(apps) => Message::Apps(apps),
            Err(e) => Message::Toast(format!("{dev_name}: {e}")),
        },
    )
}

fn fetch_detail(app: &RoverApp, app_id: &str) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id.to_string();
    let dev_name = active_device_name(app);
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
            Ok(detail) => Message::Detail(detail),
            Err(e) => Message::Toast(format!("{dev_name}: {e}")),
        },
    )
}

fn fetch_logs(app: &RoverApp, app_id: &str) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id.to_string();
    let dev_name = active_device_name(app);
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
            Ok(lines) => Message::Logs(lines),
            Err(e) => Message::Toast(format!("{dev_name}: {e}")),
        },
    )
}

fn reconnect_device(app: &mut RoverApp, idx: usize) -> Task<Message> {
    let device = &mut app.devices[idx];
    device.connecting = true;
    device.err = None;

    let addr = device.profile.address.clone();
    let key = device.profile.api_key.clone().unwrap_or_default();

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
            Ok(client) => Message::DevConnected(idx, Some(client)),
            Err(e) => Message::DevError(idx, e),
        },
    )
}

fn app_action(app: &RoverApp, app_id: &str, action: &str) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id.to_string();
    let action = action.to_string();
    let dev_name = active_device_name(app);
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
            Ok(detail) => Message::Detail(detail),
            Err(e) => Message::Toast(format!("{dev_name}: {e}")),
        },
    )
}
