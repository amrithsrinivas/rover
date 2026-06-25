mod api;
mod message;
mod theme;

use api::client::RoverClient;
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Size, Task};
use message::Message;
use rover_proto::v1::{
    AppDetailResponse, AppSummary, DeployEvent, LogEntry, ServerInfo, ServerMetrics,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connections,
    Dashboard,
    AppDetail,
    Deploy,
    Terminal,
}

pub struct RoverApp {
    pub screen: Screen,
    pub client: Option<Arc<Mutex<RoverClient>>>,
    pub connected: bool,
    pub connection_error: Option<String>,
    pub profiles: rover_core::ConnectionProfileStore,
    pub server_info: Option<ServerInfo>,
    pub metrics: Option<ServerMetrics>,
    pub apps: Vec<AppSummary>,
    pub selected_app: Option<String>,
    pub app_detail: Option<AppDetailResponse>,
    pub address_input: String,
    pub token_input: String,
    pub profile_name: String,
    pub deploy_name: String,
    pub deploy_build_cmd: String,
    pub deploy_run_cmd: String,
    pub deploy_runtime: String,
    pub deploy_app_type: String,
    pub deploy_source_path: String,
    pub deploying: bool,
    pub deploy_log: Vec<String>,
    pub env_key_input: String,
    pub env_value_input: String,
    pub env_secret: bool,
    pub toasts: Vec<String>,
    // Log streaming
    pub log_entries: Vec<String>,
    pub log_app_id: Option<String>,
    // Loading state
    pub loading: bool,
    // Reconnect
    pub reconnect_attempts: u32,
    pub reconnect_addr: Option<String>,
    pub reconnect_key: Option<String>,
    // Terminal
    pub terminal_input: String,
    pub terminal_output: Vec<String>,
}

impl RoverApp {
    fn new() -> (Self, Task<Message>) {
        let profiles = rover_core::ConnectionProfileStore::load_from_disk().unwrap_or_default();
        let screen = if profiles.profiles.is_empty() {
            Screen::Connections
        } else {
            Screen::Dashboard
        };
        (
            Self {
                screen,
                client: None,
                connected: false,
                connection_error: None,
                profiles,
                server_info: None,
                metrics: None,
                apps: vec![],
                selected_app: None,
                app_detail: None,
                address_input: String::new(),
                token_input: String::new(),
                profile_name: String::new(),
                deploy_name: String::new(),
                deploy_build_cmd: String::new(),
                deploy_run_cmd: String::new(),
                deploy_runtime: "python".into(),
                deploy_app_type: "service".into(),
                deploy_source_path: String::new(),
                deploying: false,
                deploy_log: vec![],
                env_key_input: String::new(),
                env_value_input: String::new(),
                env_secret: false,
                toasts: vec![],
                log_entries: vec![],
                log_app_id: None,
                loading: false,
                reconnect_attempts: 0,
                reconnect_addr: None,
                reconnect_key: None,
                terminal_input: String::new(),
                terminal_output: vec![],
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        if self.connected {
            "Rover — Connected".into()
        } else if self.connection_error.is_some() {
            "Rover — Error".into()
        } else {
            "Rover".into()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(s) => {
                if s != Screen::AppDetail {
                    self.log_app_id = None;
                    self.log_entries.clear();
                }
                self.screen = s;
                self.app_detail = None;
                self.selected_app = None;
            }
            Message::SetAddressInput(v) => self.address_input = v,
            Message::SetTokenInput(v) => self.token_input = v,
            Message::SetProfileName(v) => self.profile_name = v,

            Message::Connect => {
                let (addr, token, name) = (
                    self.address_input.clone(),
                    self.token_input.clone(),
                    self.profile_name.clone(),
                );
                self.connection_error = None;
                return Task::perform(connect_and_pair(addr, token, name), |r| match r {
                    Ok((c, k)) => Message::ConnectionSuccess(c, k),
                    Err(e) => Message::ConnectionError(e),
                });
            }
            Message::ConnectWithKey(addr, key) => {
                self.connection_error = None;
                self.reconnect_addr = Some(addr.clone());
                self.reconnect_key = Some(key.clone());
                return Task::perform(connect_with_key(addr, key), |r| match r {
                    Ok(c) => Message::ConnectionSuccess(c, String::new()),
                    Err(e) => Message::ConnectionError(e),
                });
            }
            Message::ConnectionSuccess(client_arc, api_key) => {
                let api_key_for_reconnect = api_key.clone();
                if !api_key.is_empty() {
                    self.profiles.upsert(rover_core::ConnectionProfile {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: self.profile_name.clone(),
                        address: self.address_input.clone(),
                        api_key: Some(api_key),
                        last_used: Some(chrono::Utc::now()),
                    });
                    self.profiles.save_to_disk().ok();
                }
                self.client = Some(client_arc);
                self.connected = true;
                self.connection_error = None;
                self.reconnect_attempts = 0;
                self.reconnect_addr = Some(self.address_input.clone());
                self.reconnect_key = Some(api_key_for_reconnect);
                self.address_input.clear();
                self.token_input.clear();
                self.screen = Screen::Dashboard;
                return Task::batch(vec![
                    Task::done(Message::Refresh),
                    Task::done(Message::RefreshApps),
                ]);
            }
            Message::ConnectionError(e) => {
                self.connection_error = Some(e);
                self.loading = false;
                self.connected = false;
                if self.reconnect_attempts < 3 && self.reconnect_addr.is_some() {
                    self.reconnect_attempts += 1;
                    return Task::perform(
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)),
                        |_| Message::RetryConnect,
                    );
                }
            }
            Message::Disconnect => {
                self.client = None;
                self.connected = false;
                self.server_info = None;
                self.metrics = None;
                self.apps.clear();
                self.screen = Screen::Connections;
            }
            Message::DeleteProfile(id) => {
                self.profiles.remove(&id);
                self.profiles.save_to_disk().ok();
            }

            Message::Refresh => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move {
                            let mut cl = c2.lock().await;
                            Ok((
                                Box::new(cl.get_info().await?),
                                Box::new(cl.get_metrics().await?),
                            ))
                        },
                        |r: Result<(Box<ServerInfo>, Box<ServerMetrics>), String>| match r {
                            Ok((i, m)) => Message::DataRefreshed(i, m),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }
            Message::RefreshApps => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move { c2.lock().await.list_apps().await.map(|r| r.apps) },
                        |r| match r {
                            Ok(a) => Message::AppsRefreshed(a),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }
            Message::DataRefreshed(info, metrics) => {
                self.server_info = Some(*info);
                self.metrics = Some(*metrics);
            }
            Message::AppsRefreshed(apps) => self.apps = apps,

            Message::SelectApp(id) => {
                let app_id = id.clone();
                self.selected_app = Some(app_id.clone());
                self.screen = Screen::AppDetail;
                self.log_app_id = Some(app_id.clone());
                self.log_entries.clear();
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    let c3 = c.clone();
                    let id_detail = app_id.clone();
                    let id_log = app_id;
                    return Task::batch(vec![
                        Task::perform(
                            async move { c2.lock().await.get_app(&id_detail).await.map(Box::new) },
                            |r| match r {
                                Ok(d) => Message::AppDetailLoaded(d),
                                Err(e) => Message::ToastError(e),
                            },
                        ),
                        Task::perform(
                            async move {
                                let mut rx =
                                    c3.lock().await.stream_logs(&id_log, true, 200).await?;
                                let mut lines = vec![];
                                while let Some(Ok(entry)) = rx.recv().await {
                                    lines.push(format!(
                                        "{} {}",
                                        if entry.is_stderr { "[stderr]" } else { "" },
                                        entry.line
                                    ));
                                }
                                Ok(lines)
                            },
                            |r: Result<Vec<String>, String>| match r {
                                Ok(lines) => Message::LogLinesReceived(lines),
                                Err(e) => Message::ToastError(e),
                            },
                        ),
                    ]);
                }
            }
            Message::AppDetailLoaded(d) => self.app_detail = Some(*d),
            Message::LogLinesReceived(lines) => self.log_entries = lines,

            Message::StartApp(id) => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move { c2.lock().await.start_app(&id).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::AppDetailLoaded(d),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }
            Message::StopApp(id) => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move { c2.lock().await.stop_app(&id).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::AppDetailLoaded(d),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }
            Message::RestartApp(id) => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move { c2.lock().await.restart_app(&id).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::AppDetailLoaded(d),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }
            Message::DeleteApp(id) => {
                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    let id2 = id;
                    return Task::perform(
                        async move {
                            let mut cl = c2.lock().await;
                            cl.delete_app(&id2).await?;
                            cl.list_apps().await.map(|r| r.apps)
                        },
                        |r: Result<Vec<AppSummary>, String>| match r {
                            Ok(a) => Message::AppsRefreshed(a),
                            Err(e) => Message::ToastError(e),
                        },
                    );
                }
            }

            // --- Deploy ---
            Message::Deploy => {
                self.screen = Screen::Deploy;
                self.deploy_name.clear();
                self.deploy_build_cmd = "pip install -r requirements.txt".into();
                self.deploy_run_cmd = "python3 main.py".into();
                self.deploy_runtime = "python".into();
                self.deploy_app_type = "service".into();
                self.deploy_source_path.clear();
                self.deploying = false;
                self.deploy_log.clear();
            }
            Message::SetDeployName(v) => self.deploy_name = v,
            Message::SetDeployBuildCmd(v) => self.deploy_build_cmd = v,
            Message::SetDeployRunCmd(v) => self.deploy_run_cmd = v,
            Message::SetDeployRuntime(v) => self.deploy_runtime = v,
            Message::SetDeployAppType(v) => self.deploy_app_type = v,
            Message::SetDeploySourcePath(v) => self.deploy_source_path = v,
            Message::PickSourceDirectory => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(|h| h.path().display().to_string())
                    },
                    |r| match r {
                        Some(p) => Message::SetDeploySourcePath(p),
                        None => Message::Noop,
                    },
                );
            }

            Message::SubmitDeploy => {
                if self.deploying || self.deploy_source_path.is_empty() {
                    return Task::none();
                }
                self.deploying = true;
                self.deploy_log.clear();
                self.deploy_log.push("Packaging source...".into());

                let manifest = format!(
                    "[app]\nname = \"{}\"\nruntime = \"{}\"\ntype = \"{}\"\n\n[build]\ncommand = \"{}\"\n\n[run]\ncommand = \"{}\"\n",
                    self.deploy_name,
                    self.deploy_runtime,
                    self.deploy_app_type,
                    self.deploy_build_cmd,
                    self.deploy_run_cmd,
                );
                let source_path = self.deploy_source_path.clone();

                if let Some(ref c) = self.client {
                    let c2 = c.clone();
                    return Task::perform(
                        async move {
                            let archive = package_source(&source_path)
                                .map_err(|e| format!("packaging failed: {e}"))?;
                            let rt = runtime_to_proto(&manifest);
                            let at = app_type_to_proto(&manifest);
                            let mut rx = c2
                                .lock()
                                .await
                                .deploy_stream("app".into(), rt, at, manifest, archive)
                                .await?;
                            let mut events = vec![];
                            while let Some(event) = rx.recv().await {
                                match event {
                                    Ok(DeployEvent { event: Some(e) }) => events.push(e),
                                    Err(e) => {
                                        events.push(rover_proto::v1::deploy_event::Event::Error(
                                            rover_proto::v1::DeployError { message: e },
                                        ));
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            Ok(events)
                        },
                        |r: Result<Vec<rover_proto::v1::deploy_event::Event>, String>| match r {
                            Ok(events) => Message::DeployStreamDone(events),
                            Err(e) => Message::DeployError(e),
                        },
                    );
                }
            }
            Message::DeployStreamDone(events) => {
                for e in &events {
                    match e {
                        rover_proto::v1::deploy_event::Event::Log(l) => {
                            self.deploy_log.push(format!(
                                "{}{}",
                                if l.is_stderr { "[err] " } else { "" },
                                l.line
                            ));
                        }
                        rover_proto::v1::deploy_event::Event::Complete(c) => {
                            self.deploy_log
                                .push(format!("✅ Deployed! App ID: {}", c.app_id));
                        }
                        rover_proto::v1::deploy_event::Event::Error(e) => {
                            self.deploy_log.push(format!("❌ {}", e.message));
                        }
                        _ => {}
                    }
                }
                self.deploying = false;
                return Task::batch(vec![
                    Task::done(Message::RefreshApps),
                    Task::done(Message::Navigate(Screen::Dashboard)),
                ]);
            }
            Message::DeployComplete => {
                self.deploying = false;
            }
            Message::DeployError(e) => {
                self.deploying = false;
                self.deploy_log.push(format!("❌ {e}"));
            }

            Message::SaveProfile(name, addr) => {
                self.profiles
                    .upsert(rover_core::ConnectionProfile::new(name, addr));
                self.profiles.save_to_disk().ok();
            }
            Message::SetEnvKey(v) => self.env_key_input = v,
            Message::SetEnvValue(v) => self.env_value_input = v,
            Message::SetEnvSecret(v) => self.env_secret = v,
            Message::AddEnvVar => {
                if let (Some(app_id), Some(c)) = (&self.selected_app, &self.client) {
                    let id = app_id.clone();
                    let key = self.env_key_input.clone();
                    let value = self.env_value_input.clone();
                    let is_secret = self.env_secret;
                    let c2 = c.clone();
                    self.env_key_input.clear();
                    self.env_value_input.clear();
                    self.env_secret = false;
                    return Task::perform(
                        async move {
                            if is_secret {
                                c2.lock().await.set_secret(&id, &key, &value).await?;
                            } else {
                                let mut vars = std::collections::HashMap::new();
                                vars.insert(key, value);
                                c2.lock().await.set_env(&id, vars).await?;
                            }
                            c2.lock().await.get_app(&id).await.map(Box::new)
                        },
                        |r: Result<Box<AppDetailResponse>, String>| match r {
                            Ok(d) => Message::EnvVarAdded(d),
                            Err(e) => Message::ToastError(format!("env var: {e}")),
                        },
                    );
                }
            }
            Message::EnvVarAdded(d) => {
                self.app_detail = Some(*d);
            }

            Message::Tick => {
                if self.connected {
                    return Task::batch(vec![
                        Task::done(Message::Refresh),
                        Task::done(Message::RefreshApps),
                    ]);
                }
            }
            Message::ToastError(m) => self.toasts.push(format!("Error: {m}")),
            Message::ToastInfo(m) => self.toasts.push(m),
            Message::DismissToast(i) => {
                if i < self.toasts.len() {
                    self.toasts.remove(i);
                }
            }
            Message::RetryConnect => {
                if let (Some(addr), Some(key)) =
                    (self.reconnect_addr.take(), self.reconnect_key.take())
                {
                    self.reconnect_attempts += 1;
                    return Task::done(Message::ConnectWithKey(addr, key));
                }
            }
            Message::SetTerminalInput(v) => self.terminal_input = v,
            Message::SendTerminalInput => {
                let cmd = self.terminal_input.clone();
                self.terminal_output.push(format!("> {cmd}"));
                self.terminal_input.clear();
            }
            Message::TerminalOutput(s) => self.terminal_output.push(s),
            Message::LoadLogs => {}
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar();
        let content: Element<_> = match self.screen {
            Screen::Connections => self.connections_screen(),
            Screen::Dashboard => self.dashboard_screen(),
            Screen::AppDetail => self.app_detail_screen(),
            Screen::Deploy => self.deploy_screen(),
            Screen::Terminal => self.terminal_screen(),
        };
        let main = row![sidebar, content];
        if self.toasts.is_empty() {
            container(main)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            let tc: Vec<Element<_>> = self
                .toasts
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    button(text(m).size(12))
                        .on_press(Message::DismissToast(i))
                        .into()
                })
                .collect();
            let ov = container(column(tc).spacing(4)).padding(8);
            iced::widget::stack([
                container(main)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                container(ov)
                    .padding(16)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            ])
            .into()
        }
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    // ---- Sidebar ----

    fn sidebar(&self) -> Element<'_, Message> {
        let (ss, sc) = if self.connected {
            ("● Connected", theme::colors::SUCCESS)
        } else if self.connection_error.is_some() {
            ("✕ Error", theme::colors::DANGER)
        } else {
            ("○ Disconnected", theme::colors::TEXT_MUTED)
        };
        let col = column![
            text("Rover").size(20),
            Space::new(0, 16),
            self.nav_button("Connections", Screen::Connections),
            self.nav_button("Dashboard", Screen::Dashboard),
            self.nav_button("Deploy", Screen::Deploy),
            Space::new(0, Length::Fill),
            text(ss).size(11).color(sc),
        ]
        .spacing(4)
        .padding(12)
        .width(180)
        .height(Length::Fill);
        container(col)
            .style(|_: &iced::Theme| container::Style {
                background: Some(theme::colors::SURFACE.into()),
                ..container::Style::default()
            })
            .width(180)
            .height(Length::Fill)
            .into()
    }

    fn nav_button(&self, label: &'static str, screen: Screen) -> Element<'_, Message> {
        let mut b: iced::widget::Button<'_, Message> =
            button(text(label).size(14)).width(Length::Fill);
        if self.screen == screen {
            b = b.style(button::primary);
        } else {
            b = b.style(button::text).on_press(Message::Navigate(screen));
        }
        b.into()
    }

    // ---- Connections ----

    fn connections_screen(&self) -> Element<'_, Message> {
        let pr: Vec<Element<_>> = self
            .profiles
            .profiles
            .iter()
            .map(|p| {
                let cb: Element<_> = if let Some(ref k) = p.api_key {
                    button(text("Connect").size(12))
                        .on_press(Message::ConnectWithKey(p.address.clone(), k.clone()))
                        .into()
                } else {
                    Space::new(0, 0).into()
                };
                row![
                    text(format!("{} — {}", p.name, p.address)).size(14),
                    Space::new(8, 0),
                    cb,
                    Space::new(4, 0),
                    button(text("✕").size(12)).on_press(Message::DeleteProfile(p.id.clone()))
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8)
                .into()
            })
            .collect();
        let ps: Element<_> = if pr.is_empty() {
            text("No saved connections")
                .size(14)
                .color(theme::colors::TEXT_MUTED)
                .into()
        } else {
            column(pr).spacing(8).into()
        };
        let form = column![
            text("New Connection").size(18),
            text_input("Address (e.g. 192.168.1.42:9050)", &self.address_input)
                .on_input(Message::SetAddressInput)
                .padding(8),
            text_input("Pairing token", &self.token_input)
                .on_input(Message::SetTokenInput)
                .padding(8),
            text_input("Profile name", &self.profile_name)
                .on_input(Message::SetProfileName)
                .padding(8),
            button(text("Connect & Pair")).on_press(Message::Connect)
        ]
        .spacing(8);
        let err: Element<_> = match &self.connection_error {
            Some(e) => text(format!("Error: {e}"))
                .size(14)
                .color(theme::colors::DANGER)
                .into(),
            None => Space::new(0, 0).into(),
        };
        scrollable(
            column![
                text("Connections").size(24),
                Space::new(0, 16),
                ps,
                Space::new(0, 24),
                form,
                err
            ]
            .spacing(8)
            .padding(24)
            .width(Length::Fill),
        )
        .into()
    }

    // ---- Dashboard ----

    fn dashboard_screen(&self) -> Element<'_, Message> {
        let hdr = row![
            text("Dashboard").size(24),
            Space::new(Length::Fill, 0),
            button(text("↻ Refresh")).on_press(Message::Refresh)
        ];
        let info = if let Some(ref i) = self.server_info {
            text(format!(
                "{}  |  {}  |  Uptime: {}s",
                i.name, i.os, i.uptime_seconds
            ))
            .size(13)
        } else {
            text("Loading...").size(13).color(theme::colors::TEXT_MUTED)
        };
        let metrics: Element<_> = if let Some(ref m) = self.metrics {
            let cpu_label = format!("CPU: {:.0}%", m.cpu_percent);
            let ram_label = format!(
                "RAM: {:.0}MB / {:.0}MB",
                m.ram_used_bytes as f64 / 1_048_576.0,
                m.ram_total_bytes as f64 / 1_048_576.0
            );
            row![
                stat_badge(cpu_label, theme::colors::ACCENT),
                Space::new(8, 0),
                stat_badge(ram_label, theme::colors::SUCCESS),
            ]
            .into()
        } else {
            text("Loading metrics...").size(13).into()
        };
        let ar: Vec<Element<_>> = self
            .apps
            .iter()
            .map(|a| {
                let (s, c) = sd(a.status);
                button(
                    row![
                        text(&a.name).size(14),
                        Space::new(Length::Fill, 0),
                        text(s).size(11).color(c)
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .width(Length::Fill)
                .style(button::text)
                .on_press(Message::SelectApp(a.app_id.clone()))
                .into()
            })
            .collect();
        let al: Element<_> = if ar.is_empty() {
            text("No apps deployed.")
                .size(14)
                .color(theme::colors::TEXT_MUTED)
                .into()
        } else {
            column(ar).spacing(4).into()
        };
        scrollable(
            column![
                hdr,
                Space::new(0, 12),
                info,
                Space::new(0, 8),
                metrics,
                Space::new(0, 16),
                text("Apps").size(18),
                Space::new(0, 8),
                al
            ]
            .spacing(8)
            .padding(24)
            .width(Length::Fill),
        )
        .into()
    }

    // ---- App Detail ----

    fn app_detail_screen(&self) -> Element<'_, Message> {
        let Some(ref d) = self.app_detail else {
            return text("Loading...").size(16).into();
        };
        let (s, sc) = sd(d.status);
        let (r, _) = rd(d.runtime);
        let (t, _) = ad(d.app_type);
        let ev: Vec<Element<_>> = d
            .env_vars
            .iter()
            .map(|(k, v)| {
                text(format!(
                    "{k}={}",
                    if v.len() > 40 {
                        format!("{}...", &v[..37])
                    } else {
                        v.clone()
                    }
                ))
                .size(12)
                .into()
            })
            .collect();
        let log_lines: Vec<Element<_>> = self
            .log_entries
            .iter()
            .map(|l| text(l).size(11).into())
            .collect();
        scrollable(
            column![
                row![
                    button(text("← Back")).on_press(Message::Navigate(Screen::Dashboard)),
                    Space::new(16, 0),
                    text(&d.name).size(24),
                    Space::new(12, 0),
                    text(s).size(14).color(sc)
                ],
                Space::new(0, 8),
                text(format!(
                    "Runtime: {r}  |  Type: {t}  |  PID: {}  |  Restarts: {}",
                    d.pid.map_or("-".into(), |p| p.to_string()),
                    d.restart_count
                ))
                .size(13),
                text(format!("Build: {}", d.build_command))
                    .size(12)
                    .color(theme::colors::TEXT_MUTED),
                text(format!("Run: {}", d.run_command))
                    .size(12)
                    .color(theme::colors::TEXT_MUTED),
                Space::new(0, 8),
                row![
                    button(text("▶ Start")).on_press(Message::StartApp(d.app_id.clone())),
                    Space::new(8, 0),
                    button(text("■ Stop")).on_press(Message::StopApp(d.app_id.clone())),
                    Space::new(8, 0),
                    button(text("↻ Restart")).on_press(Message::RestartApp(d.app_id.clone())),
                    Space::new(8, 0),
                    button(text("✕ Delete"))
                        .style(button::danger)
                        .on_press(Message::DeleteApp(d.app_id.clone()))
                ],
                Space::new(0, 12),
                text("Environment Variables").size(16),
                column(ev).spacing(2),
                Space::new(0, 8),
                row![
                    text_input("KEY", &self.env_key_input)
                        .on_input(Message::SetEnvKey)
                        .width(150)
                        .padding(6),
                    Space::new(8, 0),
                    text_input("VALUE", &self.env_value_input)
                        .on_input(Message::SetEnvValue)
                        .width(250)
                        .padding(6),
                    Space::new(8, 0),
                    button(text("+ Add")).on_press(Message::AddEnvVar)
                ],
                Space::new(0, 16),
                text("Logs").size(16),
                container(scrollable(column(log_lines).spacing(1)))
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Color::from_rgb(0.05, 0.05, 0.08).into()),
                        ..container::Style::default()
                    })
                    .padding(8)
                    .height(200),
            ]
            .spacing(6)
            .padding(24)
            .width(Length::Fill),
        )
        .into()
    }

    // ---- Deploy ----

    fn deploy_screen(&self) -> Element<'_, Message> {
        let ls: Element<_> = if self.deploy_log.is_empty() {
            Space::new(0, 0).into()
        } else {
            container(scrollable(
                column(
                    self.deploy_log
                        .iter()
                        .map(|l| -> Element<'_, Message> { text(l).size(12).into() })
                        .collect::<Vec<_>>(),
                )
                .spacing(2),
            ))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Color::from_rgb(0.05, 0.05, 0.08).into()),
                ..container::Style::default()
            })
            .padding(12)
            .height(200)
            .into()
        };
        scrollable(
            column![
                text("Deploy an App").size(24),
                Space::new(0, 12),
                text_input("App name", &self.deploy_name)
                    .on_input(Message::SetDeployName)
                    .padding(8),
                text_input("Build command", &self.deploy_build_cmd)
                    .on_input(Message::SetDeployBuildCmd)
                    .padding(8),
                text_input("Run command", &self.deploy_run_cmd)
                    .on_input(Message::SetDeployRunCmd)
                    .padding(8),
                text_input("Runtime (python/node/go/rust)", &self.deploy_runtime)
                    .on_input(Message::SetDeployRuntime)
                    .padding(8),
                text_input("Type (service/job)", &self.deploy_app_type)
                    .on_input(Message::SetDeployAppType)
                    .padding(8),
                row![
                    text_input("Source directory", &self.deploy_source_path)
                        .on_input(Message::SetDeploySourcePath)
                        .padding(8)
                        .width(Length::Fill),
                    Space::new(8, 0),
                    button(text("Browse...")).on_press(Message::PickSourceDirectory)
                ],
                Space::new(0, 12),
                button(text(if self.deploying {
                    "Deploying..."
                } else {
                    "Deploy"
                }))
                .on_press_maybe(if self.deploying {
                    None
                } else {
                    Some(Message::SubmitDeploy)
                }),
                Space::new(0, 8),
                ls,
            ]
            .spacing(8)
            .padding(24)
            .width(Length::Fill),
        )
        .into()
    }

    fn terminal_screen(&self) -> Element<'_, Message> {
        column![
            text("Terminal").size(24),
            text("Available in a future update.").size(14)
        ]
        .padding(24)
        .spacing(12)
        .into()
    }
}

// ---- Helpers ----

fn sd(s: i32) -> (&'static str, iced::Color) {
    match s {
        1 => ("deploying", theme::colors::WARNING),
        2 => ("starting", theme::colors::WARNING),
        3 => ("running", theme::colors::SUCCESS),
        4 => ("stopped", theme::colors::TEXT_MUTED),
        5 => ("crashed", theme::colors::DANGER),
        6 => ("failed", theme::colors::DANGER),
        _ => ("?", theme::colors::TEXT_MUTED),
    }
}
fn rd(r: i32) -> (&'static str, iced::Color) {
    match r {
        1 => ("python", theme::colors::PYTHON),
        2 => ("node", theme::colors::NODE),
        3 => ("go", theme::colors::GO),
        4 => ("rust", theme::colors::RUST),
        _ => ("?", theme::colors::TEXT_MUTED),
    }
}
fn ad(a: i32) -> (&'static str, iced::Color) {
    match a {
        1 => ("service", theme::colors::TEXT),
        2 => ("job", theme::colors::TEXT_MUTED),
        _ => ("?", theme::colors::TEXT_MUTED),
    }
}

fn stat_badge(label: String, color: iced::Color) -> Element<'static, Message> {
    container(text(label).size(13).color(color))
        .padding(8)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(theme::colors::SURFACE.into()),
            border: iced::Border {
                color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

async fn connect_and_pair(
    addr: String,
    token: String,
    _name: String,
) -> Result<(Arc<Mutex<RoverClient>>, String), String> {
    let mut client = RoverClient::connect(&addr).await?;
    let resp = client.pair(&token).await?;
    Ok((Arc::new(Mutex::new(client)), resp.api_key))
}

async fn connect_with_key(addr: String, key: String) -> Result<Arc<Mutex<RoverClient>>, String> {
    let mut client = RoverClient::connect(&addr).await?;
    client.set_api_key(&key);
    client.get_info().await?;
    Ok(Arc::new(Mutex::new(client)))
}

fn package_source(dir: &str) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;
    let mut archive = tar::Builder::new(Vec::new());
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.')
            && name != "target"
            && name != "node_modules"
            && name != "__pycache__"
            && name != ".git"
    }) {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(dir)?;
        if relative.as_os_str().is_empty() || !path.is_file() {
            continue;
        }
        archive.append_path_with_name(path, relative)?;
    }
    let data = archive.into_inner()?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&data)?;
    Ok(encoder.finish()?)
}

fn runtime_to_proto(manifest: &str) -> i32 {
    if manifest.contains("python") {
        1
    } else if manifest.contains("node") {
        2
    } else if manifest.contains("go") {
        3
    } else if manifest.contains("rust") {
        4
    } else {
        1
    }
}

fn app_type_to_proto(manifest: &str) -> i32 {
    if manifest.contains("job") { 2 } else { 1 }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rover=info,info".into()),
        )
        .init();
    tracing::info!("Starting Rover client");
    iced::application(RoverApp::title, RoverApp::update, RoverApp::view)
        .theme(RoverApp::theme)
        .window_size(Size::new(1024.0, 768.0))
        .run_with(RoverApp::new)
}
