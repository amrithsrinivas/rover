mod api;
mod message;
mod state;
mod theme;
mod widgets;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{Alignment, Element, Length, Size, Subscription, Task};
use tokio_stream::StreamExt;

use api::client::RoverClient;
use message::{ClientRef, Message};
use rover_core::{ConnectionProfile, ConnectionProfileStore};
use rover_proto::v1::{AppDetailResponse, DeployEvent};
use state::DeviceState;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    iced::application(title, update, view)
        .theme(|_| theme::rover_theme())
        .subscription(subscription)
        .window_size(Size::new(1100.0, 750.0))
        .run_with(init)
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct RoverApp {
    pub devices: Vec<DeviceState>,
    pub active: usize,
    pub show_add: bool,
    pub addr: String,
    pub token: String,
    pub name: String,
    pub error: Option<String>,
    pub selected_app: Option<String>,
    pub app_detail: Option<AppDetailResponse>,
    pub log_entries: Vec<String>,
    pub deploy_open: bool,
    pub deploy_name: String,
    pub deploy_runtime: String,
    pub deploy_build: String,
    pub deploy_run: String,
    pub deploy_path: String,
    pub deploy_env_vars: Vec<(String, String)>,
    pub deploy_env_key: String,
    pub deploy_env_value: String,
    pub deploying: bool,
    pub deploy_log: Vec<String>,
    pub confirm_delete: Option<(String, String)>,
    pub confirm_device_delete: Option<usize>,
    pub env_key: String,
    pub env_value: String,
    pub toasts: Vec<String>,
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

fn title(app: &RoverApp) -> String {
    if let Some(d) = app.devices.get(app.active) {
        if d.connected {
            return format!("Rover — {}", d.profile.name);
        }
    }
    "Rover".into()
}

fn init() -> (RoverApp, Task<Message>) {
    let store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
    let devices: Vec<DeviceState> = store
        .profiles
        .into_iter()
        .map(DeviceState::from_profile)
        .collect();
    let show_add = devices.is_empty();

    // If there's an active profile, try reconnecting immediately
    let tasks: Vec<Task<Message>> = devices
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            d.profile.api_key.as_ref().map(|key| {
                let addr = d.profile.address.clone();
                let key = key.clone();
                let idx = i;
                Task::perform(
                    async move {
                        let mut client = RoverClient::connect(&addr)
                            .await
                            .map_err(|e| e.to_string())?;
                        client.set_api_key(&key);
                        // Verify connection with a simple info call
                        client.get_info().await.map_err(|e| e.to_string())?;
                        Ok(Arc::new(Mutex::new(client)))
                    },
                    move |result| match result {
                        Ok(client) => Message::DevConnected(idx, Some(client)),
                        Err(e) => Message::DevError(idx, e),
                    },
                )
            })
        })
        .collect();

    let app = RoverApp {
        devices,
        active: 0,
        show_add,
        addr: String::new(),
        token: String::new(),
        name: String::new(),
        error: None,
        selected_app: None,
        app_detail: None,
        log_entries: Vec::new(),
        deploy_open: false,
        deploy_name: String::new(),
        deploy_runtime: String::new(),
        deploy_build: String::new(),
        deploy_run: String::new(),
        deploy_path: String::new(),
        deploy_env_vars: Vec::new(),
        deploy_env_key: String::new(),
        deploy_env_value: String::new(),
        deploying: false,
        deploy_log: Vec::new(),
        confirm_delete: None,
        confirm_device_delete: None,
        env_key: String::new(),
        env_value: String::new(),
        toasts: Vec::new(),
    };

    (app, Task::batch(tasks))
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

fn subscription(_app: &RoverApp) -> Subscription<Message> {
    iced::time::every(Duration::from_secs(2)).map(|_| Message::Tick)
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

fn update(app: &mut RoverApp, message: Message) -> Task<Message> {
    match message {
        Message::Noop => Task::none(),

        Message::Tick => tick(app),

        // --- Device list ---
        Message::Select(i) => {
            if i < app.devices.len() {
                app.active = i;
                app.selected_app = None;
                app.app_detail = None;
                app.log_entries.clear();
                // Reconnect if we have an API key
                let d = &app.devices[i];
                if !d.connected && !d.connecting && d.profile.api_key.is_some() {
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
        Message::SetAddr(s) => {
            app.addr = s;
            Task::none()
        }
        Message::SetToken(s) => {
            app.token = s;
            Task::none()
        }
        Message::SetName(s) => {
            app.name = s;
            Task::none()
        }
        Message::Connect => {
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

        Message::DevAdded(name, client, api_key) => {
            let addr = app.addr.trim().to_string();
            let mut profile = ConnectionProfile::new(name.clone(), addr.clone());
            profile.api_key = Some(api_key.clone());
            profile.last_used = Some(chrono::Utc::now());

            // Save to disk
            let mut store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
            store.upsert(profile.clone());
            let _ = store.save_to_disk();

            let device = DeviceState {
                profile,
                client: Some(client),
                connected: true,
                info: None,
                metrics: None,
                apps: Vec::new(),
                connecting: false,
                err: None,
            };

            app.devices.push(device);
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

        Message::DevAddErr(e) => {
            app.error = Some(e);
            Task::none()
        }

        Message::DevConnected(idx, client) => {
            if let Some(d) = app.devices.get_mut(idx) {
                d.connected = client.is_some();
                d.client = client;
                d.connecting = false;
                d.err = None;
                if d.connected {
                    return Task::batch([refresh_data(app), refresh_apps(app)]);
                }
            }
            Task::none()
        }

        Message::DevError(idx, err) => {
            if let Some(d) = app.devices.get_mut(idx) {
                d.connecting = false;
                d.err = Some(err.clone());
            }
            if app.active == idx {
                app.toasts = vec![format!("Connection error: {err}")];
            }
            Task::none()
        }

        Message::Disconnect => {
            if let Some(d) = app.devices.get_mut(app.active) {
                d.connected = false;
                d.client = None;
                d.info = None;
                d.metrics = None;
                d.apps.clear();
                d.err = None;
            }
            app.selected_app = None;
            app.app_detail = None;
            app.log_entries.clear();
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

        Message::ConfirmDeleteDevice(idx) => {
            app.confirm_device_delete = None;
            if idx < app.devices.len() {
                let profile_id = app.devices[idx].profile.id.clone();
                // Delete from disk
                if let Ok(mut store) = ConnectionProfileStore::load_from_disk() {
                    store.remove(&profile_id);
                    let _ = store.save_to_disk();
                }
                // Remove from device list
                app.devices.remove(idx);
                // Adjust active index
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

        // --- Data refresh ---
        Message::Data(info, metrics) => {
            if let Some(d) = app.devices.get_mut(app.active) {
                d.info = Some(*info);
                d.metrics = Some(*metrics);
            }
            Task::none()
        }

        Message::Apps(apps_list) => {
            if let Some(d) = app.devices.get_mut(app.active) {
                // Check if selected app still exists
                if let Some(ref selected) = app.selected_app {
                    if !apps_list.iter().any(|a| a.app_id == *selected) {
                        app.selected_app = None;
                        app.app_detail = None;
                        app.log_entries.clear();
                    }
                }
                d.apps = apps_list;
            }
            Task::none()
        }

        // --- App detail ---
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

        Message::ConfirmDelete(app_id, _name) => {
            app.confirm_delete = None;
            let client = get_client(app);
            let aid = app_id.clone();
            Task::perform(
                async move {
                    if let Some(c) = client {
                        let mut c = c.lock().await;
                        c.delete_app(&aid).await.map_err(|e| e.to_string())
                    } else {
                        Err("Not connected".into())
                    }
                },
                move |result| match result {
                    Ok(()) => {
                        // Clear detail and go back to dashboard
                        Message::Back
                    }
                    Err(e) => Message::Toast(format!("Delete failed: {e}")),
                },
            )
        }

        // --- Deploy modal ---
        Message::OpenDeploy => {
            app.deploy_open = true;
            app.deploy_name.clear();
            app.deploy_runtime.clear();
            app.deploy_build.clear();
            app.deploy_run.clear();
            app.deploy_path.clear();
            app.deploy_env_vars.clear();
            app.deploy_env_key.clear();
            app.deploy_env_value.clear();
            app.deploying = false;
            app.deploy_log.clear();
            Task::none()
        }

        Message::CloseDeploy => {
            app.deploy_open = false;
            // Refresh apps after deploy might have happened
            refresh_apps(app)
        }

        Message::SetDName(s) => {
            app.deploy_name = s;
            Task::none()
        }

        Message::SetDRuntime(s) => {
            app.deploy_runtime = s.clone();
            // Auto-fill build/run defaults if they're empty or match previous runtime
            if !s.is_empty() {
                app.deploy_build = default_build_for(&s).to_string();
                app.deploy_run = default_run_for(&s).to_string();
            }
            Task::none()
        }

        Message::SetDBuild(s) => {
            app.deploy_build = s;
            Task::none()
        }
        Message::SetDRun(s) => {
            app.deploy_run = s;
            Task::none()
        }
        Message::SetDPath(s) => {
            app.deploy_path = s;
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

        Message::SetDEKey(s) => {
            app.deploy_env_key = s;
            Task::none()
        }
        Message::SetDEValue(s) => {
            app.deploy_env_value = s;
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

        Message::SubmitDeploy => {
            if app.deploy_path.is_empty() || app.deploying {
                return Task::none();
            }
            app.deploying = true;
            app.deploy_log.clear();

            let name = app.deploy_name.trim().to_string();
            let runtime = app.deploy_runtime.clone();
            let build_cmd = app.deploy_build.trim().to_string();
            let run_cmd = app.deploy_run.trim().to_string();
            let source_path = app.deploy_path.clone();
            let env_vars = app.deploy_env_vars.clone();
            let client = get_client(app);

            Task::perform(
                async move {
                    deploy_app(
                        client,
                        name,
                        runtime,
                        build_cmd,
                        run_cmd,
                        source_path,
                        env_vars,
                    )
                    .await
                },
                |result| match result {
                    Ok(events) => Message::DeployDone(events),
                    Err(e) => Message::DeployErr(e),
                },
            )
        }

        Message::DeployDone(events) => {
            app.deploying = false;
            // Collect formatted log lines from events
            for ev in &events {
                match &ev.event {
                    Some(rover_proto::v1::deploy_event::Event::Log(log)) => {
                        if log.is_stderr {
                            app.deploy_log.push(format!("[err] {}", log.line));
                        } else {
                            app.deploy_log.push(log.line.clone());
                        }
                    }
                    Some(rover_proto::v1::deploy_event::Event::Complete(complete)) => {
                        app.deploy_log
                            .push(format!("✅ Deployed — {}", complete.app_id));
                    }
                    Some(rover_proto::v1::deploy_event::Event::Error(err)) => {
                        app.deploy_log.push(format!("❌ {}", err.message));
                    }
                    Some(rover_proto::v1::deploy_event::Event::Progress(prog)) => {
                        app.deploy_log
                            .push(format!("[{:.0}%] {}", prog.percent, prog.stage));
                    }
                    None => {}
                }
            }
            Task::none()
        }

        Message::DeployErr(e) => {
            app.deploying = false;
            app.deploy_log.push(format!("❌ {e}"));
            Task::none()
        }

        // --- Env vars on detail ---
        Message::SetEKey(s) => {
            app.env_key = s;
            Task::none()
        }
        Message::SetEValue(s) => {
            app.env_value = s;
            Task::none()
        }
        Message::AddEnv => {
            let key = app.env_key.trim().to_string();
            let value = app.env_value.trim().to_string();
            if key.is_empty() {
                return Task::none();
            }
            let app_id = match &app.selected_app {
                Some(id) => id.clone(),
                None => return Task::none(),
            };
            let client = get_client(app);
            Task::perform(
                async move {
                    if let Some(c) = client {
                        let mut c = c.lock().await;
                        let mut vars = std::collections::HashMap::new();
                        vars.insert(key, value);
                        c.set_env(&app_id, vars).await.map_err(|e| e.to_string())
                    } else {
                        Err("Not connected".into())
                    }
                },
                |result| match result {
                    Ok(detail) => {
                        // Refresh detail after setting env
                        Message::Detail(Box::new(detail))
                    }
                    Err(e) => Message::Toast(e),
                },
            )
        }

        // --- Toasts ---
        Message::Toast(msg) => {
            // Keep only the most recent error — disconnection storms
            // would otherwise flood the UI with duplicate toasts.
            app.toasts = vec![msg];
            Task::none()
        }
        Message::Dismiss(i) => {
            if i < app.toasts.len() {
                app.toasts.remove(i);
            }
            Task::none()
        }
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn view(app: &RoverApp) -> Element<'_, Message> {
    let sidebar = widgets::sidebar::sidebar(app);

    let content = if app.devices.is_empty() && !app.show_add {
        // No devices at all
        container(
            text("Add a device to get started")
                .size(16)
                .color(theme::colors::TEXT_MUTED),
        )
        .center_x(iced::Length::Fill)
        .center_y(iced::Length::Fill)
        .padding(40)
        .into()
    } else if let Some(d) = app.devices.get(app.active) {
        if d.connecting {
            container(
                text("Connecting...")
                    .size(16)
                    .color(theme::colors::TEXT_MUTED),
            )
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
        } else if let Some(err) = &d.err {
            // Connection error
            let retry_btn = button(text("Retry").size(14))
                .style(iced::widget::button::primary)
                .on_press(Message::Select(app.active));

            container(
                column![
                    text("Connection failed")
                        .size(16)
                        .color(theme::colors::DANGER),
                    Space::with_height(8),
                    text(err).size(13).color(theme::colors::TEXT_MUTED),
                    Space::with_height(12),
                    retry_btn,
                ]
                .align_x(Alignment::Center)
                .spacing(0),
            )
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
        } else if !d.connected {
            container(
                text("Select a device and connect")
                    .size(16)
                    .color(theme::colors::TEXT_MUTED),
            )
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
        } else if app.selected_app.is_some() {
            // App detail view
            widgets::detail::app_detail(app)
        } else {
            // Dashboard
            widgets::dashboard::dashboard(app)
        }
    } else {
        // active index out of range — select first device
        container(
            text("Select a device and connect")
                .size(16)
                .color(theme::colors::TEXT_MUTED),
        )
        .center_x(iced::Length::Fill)
        .center_y(iced::Length::Fill)
        .into()
    };

    let status_bar = status_bar(app);

    let main_area = column![status_bar, content].spacing(0);

    let body = row![sidebar, main_area].spacing(0);

    // Deploy modal overlay
    let body_with_modal = if app.deploy_open {
        stack([body.into(), widgets::deploy::deploy_modal(app)]).into()
    } else {
        body.into()
    };

    // Device delete confirmation overlay
    let body_with_delete = if let Some(idx) = app.confirm_device_delete {
        if let Some(d) = app.devices.get(idx) {
            let name = d.profile.name.clone();
            let modal = container(
                column![
                    text(format!("Remove {name}?"))
                        .size(16)
                        .color(theme::colors::TEXT),
                    Space::with_height(8),
                    text("Saved API key will be deleted.")
                        .size(13)
                        .color(theme::colors::TEXT_MUTED),
                    Space::with_height(16),
                    row![
                        button(text("Cancel").size(13))
                            .style(iced::widget::button::secondary)
                            .on_press(Message::CancelDeleteDevice),
                        Space::with_width(8),
                        button(text("Remove").size(13))
                            .style(iced::widget::button::danger)
                            .on_press(Message::ConfirmDeleteDevice(idx)),
                    ]
                    .spacing(0),
                ]
                .padding(24),
            )
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(theme::colors::ELEVATED)),
                border: iced::Border {
                    color: theme::colors::BORDER,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Style::default()
            });

            stack([
                body_with_modal,
                container(modal.center_x(Length::Fill).center_y(Length::Fill))
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.0, 0.0, 0.0, 0.6,
                        ))),
                        ..container::Style::default()
                    })
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            ])
            .into()
        } else {
            body_with_modal
        }
    } else {
        body_with_modal
    };

    // Toast overlay
    if app.toasts.is_empty() {
        body_with_delete
    } else {
        let toasts: Vec<Element<Message>> = app
            .toasts
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                button(
                    container(
                        row![
                            text(msg).size(12).color(theme::colors::TEXT),
                            Space::with_width(8),
                            text("✕").size(11).color(theme::colors::TEXT_MUTED),
                        ]
                        .align_y(Alignment::Center)
                        .padding(10),
                    )
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(theme::colors::DANGER)),
                        border: iced::Border {
                            color: theme::colors::DANGER,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..container::Style::default()
                    }),
                )
                .style(iced::widget::button::text)
                .on_press(Message::Dismiss(i))
                .into()
            })
            .collect();

        stack([
            body_with_delete,
            container(column(toasts).spacing(6).padding(12))
                .width(Length::Fill)
                .into(),
        ])
        .into()
    }
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn status_bar(app: &RoverApp) -> Element<'_, Message> {
    let status_text = if let Some(d) = app.devices.get(app.active) {
        if d.connected {
            format!("● {} — {}", d.profile.name, d.profile.address)
        } else if d.connecting {
            format!("◌ Connecting to {}...", d.profile.name)
        } else if d.err.is_some() {
            format!("✕ {}", d.profile.name)
        } else {
            format!("○ {} — disconnected", d.profile.name)
        }
    } else if app.devices.is_empty() {
        "No devices".into()
    } else {
        "Select a device".into()
    };

    container(text(status_text).size(11).color(theme::colors::TEXT_MUTED))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::colors::SURFACE)),
            border: iced::Border {
                color: theme::colors::BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .padding(8)
        .width(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the client reference for the active device.
fn get_client(app: &RoverApp) -> Option<ClientRef> {
    app.devices.get(app.active).and_then(|d| d.client.clone())
}

/// 2-second tick handler — refreshes data for the active device.
fn tick(app: &mut RoverApp) -> Task<Message> {
    if let Some(d) = app.devices.get(app.active) {
        if d.connected {
            let mut tasks = vec![refresh_data(app), refresh_apps(app)];

            // If viewing an app detail, also fetch logs and refresh detail
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
    Task::perform(
        async move {
            if let Some(c) = client {
                let mut c = c.lock().await;
                let info = c.get_info().await?;
                let metrics = c.get_metrics().await?;
                Ok((Box::new(info), Box::new(metrics)))
            } else {
                Err("Not connected".into())
            }
        },
        |result| match result {
            Ok((info, metrics)) => Message::Data(info, metrics),
            Err(e) => Message::Toast(e),
        },
    )
}

fn refresh_apps(app: &RoverApp) -> Task<Message> {
    let client = get_client(app);
    Task::perform(
        async move {
            if let Some(c) = client {
                let mut c = c.lock().await;
                c.list_apps(100, 0).await
            } else {
                Err("Not connected".into())
            }
        },
        |result| match result {
            Ok(apps) => Message::Apps(apps),
            Err(e) => Message::Toast(e),
        },
    )
}

fn fetch_detail(app: &RoverApp, app_id: &str) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id.to_string();
    Task::perform(
        async move {
            if let Some(c) = client {
                let mut c = c.lock().await;
                c.get_app(&aid).await.map(Box::new)
            } else {
                Err("Not connected".into())
            }
        },
        |result| match result {
            Ok(detail) => Message::Detail(detail),
            Err(e) => Message::Toast(e),
        },
    )
}

fn fetch_logs(app: &RoverApp, app_id: &str) -> Task<Message> {
    let client = get_client(app);
    let aid = app_id.to_string();
    Task::perform(
        async move {
            if let Some(c) = client {
                let mut c = c.lock().await;
                let mut stream = c.stream_logs(&aid, 50).await.map_err(|e| e.to_string())?;
                let mut lines = Vec::new();
                // Collect up to 100 log lines
                use tokio_stream::StreamExt;
                while let Some(Ok(entry)) = stream.next().await {
                    if entry.is_stderr {
                        lines.push(format!("[err] {}", entry.line));
                    } else {
                        lines.push(entry.line.clone());
                    }
                    if lines.len() >= 100 {
                        break;
                    }
                }
                Ok(lines)
            } else {
                Err("Not connected".into())
            }
        },
        |result| match result {
            Ok(lines) => Message::Logs(lines),
            Err(e) => Message::Toast(e),
        },
    )
}

fn reconnect_device(app: &mut RoverApp, idx: usize) -> Task<Message> {
    let d = &mut app.devices[idx];
    d.connecting = true;
    d.err = None;

    let addr = d.profile.address.clone();
    let key = d.profile.api_key.clone().unwrap_or_default();

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
    Task::perform(
        async move {
            if let Some(c) = client {
                let mut c = c.lock().await;
                match action.as_str() {
                    "start" => c.start_app(&aid).await.map(Box::new),
                    "stop" => c.stop_app(&aid).await.map(Box::new),
                    "restart" => c.restart_app(&aid).await.map(Box::new),
                    _ => Err("unknown action".into()),
                }
            } else {
                Err("Not connected".into())
            }
        },
        |result| match result {
            Ok(detail) => Message::Detail(detail),
            Err(e) => Message::Toast(e),
        },
    )
}

fn default_build_for(runtime: &str) -> &'static str {
    match runtime {
        "python" => "pip install -r requirements.txt",
        "node" => "npm install",
        "go" => "go build -o app .",
        "rust" => "cargo build --release",
        _ => "",
    }
}

fn default_run_for(runtime: &str) -> &'static str {
    match runtime {
        "python" => "python main.py",
        "node" => "node index.js",
        "go" => "./app",
        "rust" => "./target/release/app",
        _ => "",
    }
}

/// Package source directory as tar.gz, build TOML manifest, and deploy via streaming RPC.
async fn deploy_app(
    client: Option<ClientRef>,
    name: String,
    runtime: String,
    build_cmd: String,
    run_cmd: String,
    source_path: String,
    env_vars: Vec<(String, String)>,
) -> Result<Vec<DeployEvent>, String> {
    let c = client.ok_or("Not connected")?;

    // 1. Build TOML manifest using toml::map::Map
    let runtime_proto = match runtime.as_str() {
        "python" => 1i32,
        "node" => 2,
        "go" => 3,
        "rust" => 4,
        _ => return Err(format!("Unknown runtime: {runtime}")),
    };

    let mut manifest_map = toml::map::Map::new();

    // [app]
    let mut app_section = toml::map::Map::new();
    app_section.insert("name".into(), toml::Value::String(name.clone()));
    app_section.insert("runtime".into(), toml::Value::String(runtime.clone()));
    manifest_map.insert("app".into(), toml::Value::Table(app_section));

    // [build]
    let mut build_section = toml::map::Map::new();
    build_section.insert("command".into(), toml::Value::String(build_cmd.clone()));
    manifest_map.insert("build".into(), toml::Value::Table(build_section));

    // [run]
    let mut run_section = toml::map::Map::new();
    run_section.insert("command".into(), toml::Value::String(run_cmd.clone()));
    manifest_map.insert("run".into(), toml::Value::Table(run_section));

    // [env]
    if !env_vars.is_empty() {
        let mut env_section = toml::map::Map::new();
        for (k, v) in &env_vars {
            env_section.insert(k.clone(), toml::Value::String(v.clone()));
        }
        manifest_map.insert("env".into(), toml::Value::Table(env_section));
    }

    let manifest_toml = toml::to_string_pretty(&toml::Value::Table(manifest_map))
        .map_err(|e| format!("TOML serialization error: {e}"))?;

    // 2. Package source directory as tar.gz
    let source_bytes = package_source(&source_path).await?;

    // 3. Build DeployRequest
    let req = rover_proto::v1::DeployRequest {
        name: name.clone(),
        runtime: runtime_proto,
        manifest_toml,
        source_archive: source_bytes,
    };

    // 4. Call deploy_stream and collect events
    let mut c = c.lock().await;
    let mut stream = c.deploy_stream(req).await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => events.push(ev),
            Err(e) => {
                events.push(DeployEvent {
                    event: Some(rover_proto::v1::deploy_event::Event::Error(
                        rover_proto::v1::DeployError {
                            message: e.to_string(),
                        },
                    )),
                });
                break;
            }
        }
    }

    Ok(events)
}

/// Recursively package a directory as tar.gz bytes, filtering out common ignore patterns.
async fn package_source(path: &str) -> Result<Vec<u8>, String> {
    let path = std::path::Path::new(path);
    if !path.is_dir() {
        return Err("Source path is not a directory".into());
    }

    let ignore_patterns: &[&str] = &[
        ".git",
        "target",
        "node_modules",
        "__pycache__",
        ".venv",
        "venv",
        ".DS_Store",
    ];

    let mut archive = tar::Builder::new(Vec::new());

    let base = path.to_path_buf();
    // Walk the directory
    fn walk(
        dir: &std::path::Path,
        base: &std::path::Path,
        archive: &mut tar::Builder<Vec<u8>>,
        ignore_patterns: &[&str],
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))? {
            let entry = entry.map_err(|e| format!("entry: {e}"))?;
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();

            // Check if should skip
            let first_component = name.as_ref();
            let should_skip = ignore_patterns.contains(&first_component) || name.starts_with('.');
            if should_skip {
                continue;
            }

            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("strip_prefix: {e}"))?;

            if path.is_dir() {
                // Add directory entry
                let dir_path = format!("{}/", relative.to_string_lossy());
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_mode(0o755);
                archive
                    .append_data(&mut header, dir_path, &mut std::io::empty())
                    .map_err(|e| format!("tar append dir error: {e}"))?;
                walk(&path, base, archive, ignore_patterns)?;
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

    walk(&base, &base, &mut archive, ignore_patterns)?;

    let tar_bytes = archive
        .into_inner()
        .map_err(|e| format!("tar finalize error: {e}"))?;

    // Compress with flate2 (gzip)
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&tar_bytes)
        .map_err(|e| format!("gzip write error: {e}"))?;
    let gz_bytes = encoder
        .finish()
        .map_err(|e| format!("gzip finish error: {e}"))?;

    Ok(gz_bytes)
}
