mod api;
mod message;
mod theme;

use api::client::RoverClient;
use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Color, Element, Length, Size, Task};
use message::Message;
use rover_proto::v1::{AppDetailResponse, AppSummary, ServerInfo, ServerMetrics};
use std::sync::Arc;
use tokio::sync::Mutex;

type ClientRef = Arc<Mutex<RoverClient>>;

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
    pub env_key: String,
    pub env_value: String,
    pub toasts: Vec<String>,
}

pub struct DeviceState {
    pub profile: rover_core::ConnectionProfile,
    pub client: Option<ClientRef>,
    pub connected: bool,
    pub info: Option<ServerInfo>,
    pub metrics: Option<ServerMetrics>,
    pub apps: Vec<AppSummary>,
    pub connecting: bool,
    pub err: Option<String>,
}

impl DeviceState {
    fn new(p: rover_core::ConnectionProfile) -> Self {
        Self {
            profile: p,
            client: None,
            connected: false,
            info: None,
            metrics: None,
            apps: vec![],
            connecting: false,
            err: None,
        }
    }
}

impl RoverApp {
    fn new() -> (Self, Task<Message>) {
        let store = rover_core::ConnectionProfileStore::load_from_disk().unwrap_or_default();
        let devices: Vec<_> = store.profiles.into_iter().map(DeviceState::new).collect();
        let show_add = devices.is_empty();
        (
            Self {
                devices,
                active: 0,
                show_add,
                addr: String::new(),
                token: String::new(),
                name: String::new(),
                error: None,
                selected_app: None,
                app_detail: None,
                log_entries: vec![],
                deploy_open: false,
                deploy_name: String::new(),
                deploy_runtime: "python".into(),
                deploy_build: String::new(),
                deploy_run: String::new(),
                deploy_path: String::new(),
                deploy_env_vars: vec![],
                deploy_env_key: String::new(),
                deploy_env_value: String::new(),
                deploying: false,
                deploy_log: vec![],
                confirm_delete: None,
                env_key: String::new(),
                env_value: String::new(),
                toasts: vec![],
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        match self.devices.get(self.active) {
            Some(d) if d.connected => format!("Rover — {}", d.profile.name),
            _ => "Rover".into(),
        }
    }

    fn dev(&self) -> Option<&DeviceState> {
        self.devices.get(self.active)
    }
    fn dev_mut(&mut self) -> Option<&mut DeviceState> {
        self.devices.get_mut(self.active)
    }
    fn client(&self) -> Option<ClientRef> {
        self.dev().and_then(|d| d.client.clone())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Select(i) => {
                if i < self.devices.len() {
                    self.active = i;
                    self.selected_app = None;
                    self.app_detail = None;
                    self.log_entries.clear();
                    self.show_add = false;
                    if let Some(key) = self.devices[i].profile.api_key.clone() {
                        let addr = self.devices[i].profile.address.clone();
                        self.devices[i].connecting = true;
                        return Task::perform(connect_with_key(addr, key), move |r| match r {
                            Ok(c) => Message::DevConnected(i as u32, Some(c)),
                            Err(e) => Message::DevError(i as u32, e),
                        });
                    }
                    return Task::done(Message::Refresh);
                }
            }
            Message::ShowAdd => self.show_add = true,
            Message::HideAdd => self.show_add = false,
            Message::SetAddr(v) => self.addr = v,
            Message::SetToken(v) => self.token = v,
            Message::SetName(v) => self.name = v,
            Message::Connect => {
                let (addr, token, name) =
                    (self.addr.clone(), self.token.clone(), self.name.clone());
                let n = name.clone();
                self.error = None;
                let n2 = n.clone();
                return Task::perform(connect_and_pair(addr, token, name), move |r| match r {
                    Ok((c, k)) => Message::DevAdded(n2.clone(), c, k),
                    Err(e) => Message::DevAddErr(e),
                });
            }
            Message::DevAdded(name, client, key) => {
                let p = rover_core::ConnectionProfile {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    address: self.addr.clone(),
                    api_key: Some(key),
                    last_used: Some(chrono::Utc::now()),
                };
                let mut store = rover_core::ConnectionProfileStore {
                    profiles: self.devices.iter().map(|d| d.profile.clone()).collect(),
                    active_profile_id: None,
                };
                store.upsert(p.clone());
                store.save_to_disk().ok();
                self.devices.push(DeviceState {
                    profile: p,
                    client: Some(client),
                    connected: true,
                    info: None,
                    metrics: None,
                    apps: vec![],
                    connecting: false,
                    err: None,
                });
                self.active = self.devices.len() - 1;
                self.addr.clear();
                self.token.clear();
                self.name.clear();
                self.show_add = false;
                return Task::batch(vec![
                    Task::done(Message::Refresh),
                    Task::done(Message::RefreshApps),
                ]);
            }
            Message::DevAddErr(e) => self.error = Some(e),
            Message::DevConnected(idx, c) => {
                if let Some(d) = self.devices.get_mut(idx as usize) {
                    d.connecting = false;
                    d.err = None;
                    if let Some(cl) = c {
                        d.client = Some(cl);
                    }
                    d.connected = true;
                }
                return Task::batch(vec![
                    Task::done(Message::Refresh),
                    Task::done(Message::RefreshApps),
                ]);
            }
            Message::DevError(idx, e) => {
                if let Some(d) = self.devices.get_mut(idx as usize) {
                    d.connecting = false;
                    d.err = Some(e);
                }
            }

            Message::Refresh => {
                if let Some(c) = self.client() {
                    return Task::perform(
                        async move {
                            let mut cl = c.lock().await;
                            Ok((
                                Box::new(cl.get_info().await?),
                                Box::new(cl.get_metrics().await?),
                            ))
                        },
                        |r| match r {
                            Ok((i, m)) => Message::Data(i, m),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }
            Message::RefreshApps => {
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    return Task::perform(
                        async move { c2.lock().await.list_apps().await.map(|r| r.apps) },
                        |r| match r {
                            Ok(a) => Message::Apps(a),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }
            Message::Data(i, m) => {
                if let Some(d) = self.dev_mut() {
                    d.info = Some(*i);
                    d.metrics = Some(*m);
                }
            }
            Message::Apps(apps) => {
                if let Some(d) = self.dev_mut() {
                    d.apps = apps;
                }
                if let Some(ref sid) = self.selected_app {
                    if let Some(d) = self.dev() {
                        if !d.apps.iter().any(|a| a.app_id == *sid) {
                            self.selected_app = None;
                            self.app_detail = None;
                        }
                    }
                }
            }

            Message::SelectApp(id) => {
                self.selected_app = Some(id.clone());
                self.log_entries.clear();
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    let c3 = c.clone();
                    let id2 = id.clone();
                    return Task::batch(vec![
                        Task::perform(
                            async move { c2.lock().await.get_app(&id2).await.map(Box::new) },
                            |r| match r {
                                Ok(d) => Message::Detail(d),
                                Err(e) => Message::Toast(e),
                            },
                        ),
                        Task::perform(
                            async move {
                                let mut rx = c3.lock().await.stream_logs(&id, 500).await?;
                                let mut lines: Vec<String> = vec![];
                                while let Some(Ok(e)) = rx.recv().await {
                                    lines.push(format!(
                                        "{} {}",
                                        if e.is_stderr { "[err]" } else { "" },
                                        e.line
                                    ));
                                }
                                Ok(lines)
                            },
                            |r: Result<Vec<String>, String>| match r {
                                Ok(l) => Message::Logs(l),
                                Err(e) => Message::Toast(e),
                            },
                        ),
                    ]);
                }
            }
            Message::Detail(d) => self.app_detail = Some(*d),
            Message::Logs(l) => self.log_entries = l,
            Message::Back => {
                self.selected_app = None;
                self.app_detail = None;
                self.log_entries.clear();
            }
            Message::Start(id) => {
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    let id2 = id;
                    return Task::perform(
                        async move { c2.lock().await.start_app(&id2).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::Detail(d),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }
            Message::Stop(id) => {
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    let id2 = id;
                    return Task::perform(
                        async move { c2.lock().await.stop_app(&id2).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::Detail(d),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }
            Message::Restart(id) => {
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    let id2 = id;
                    return Task::perform(
                        async move { c2.lock().await.restart_app(&id2).await.map(Box::new) },
                        |r| match r {
                            Ok(d) => Message::Detail(d),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }
            Message::Delete(id) => {
                let name = self
                    .app_detail
                    .as_ref()
                    .map_or_else(|| id.clone(), |d| d.name.clone());
                self.confirm_delete = Some((id, name));
            }
            Message::CancelDelete => self.confirm_delete = None,
            Message::ConfirmDelete((id, _)) => {
                self.confirm_delete = None;
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    let id2 = id;
                    return Task::perform(
                        async move {
                            let mut cl = c2.lock().await;
                            cl.delete_app(&id2).await?;
                            cl.list_apps().await.map(|r| r.apps)
                        },
                        |r| match r {
                            Ok(a) => Message::Apps(a),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }

            // Deploy
            Message::OpenDeploy => {
                self.deploy_open = true;
                self.deploy_name.clear();
                self.deploy_runtime = "python".into();
                self.deploy_build = "pip install -r requirements.txt".into();
                self.deploy_run = "python main.py".into();
                self.deploy_path.clear();
                self.deploy_env_vars.clear();
                self.deploy_env_key.clear();
                self.deploy_env_value.clear();
                self.deploying = false;
                self.deploy_log.clear();
            }
            Message::CloseDeploy => self.deploy_open = false,
            Message::SetDName(v) => self.deploy_name = v,
            Message::SetDRuntime(v) => {
                self.deploy_runtime = v;
                let defaults = [
                    (
                        "python",
                        "pip install -r requirements.txt",
                        "python main.py",
                    ),
                    ("node", "npm install", "node index.js"),
                    ("go", "go build -o app .", "./app"),
                    ("rust", "cargo build --release", "./target/release/app"),
                ];
                for (rt, b, r) in &defaults {
                    if self.deploy_runtime == *rt {
                        if self.deploy_build.is_empty()
                            || defaults.iter().any(|(_, db, _)| self.deploy_build == *db)
                        {
                            self.deploy_build = b.to_string();
                        }
                        if self.deploy_run.is_empty()
                            || defaults.iter().any(|(_, _, dr)| self.deploy_run == *dr)
                        {
                            self.deploy_run = r.to_string();
                        }
                    }
                }
            }
            Message::SetDBuild(v) => self.deploy_build = v,
            Message::SetDRun(v) => self.deploy_run = v,
            Message::SetDPath(v) => self.deploy_path = v,
            Message::PickPath => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(|h| h.path().display().to_string())
                    },
                    |r| match r {
                        Some(p) => Message::SetDPath(p),
                        None => Message::Noop,
                    },
                );
            }
            Message::SetDEKey(v) => self.deploy_env_key = v,
            Message::SetDEValue(v) => self.deploy_env_value = v,
            Message::AddDEVar => {
                let k = self.deploy_env_key.trim().to_string();
                let v = self.deploy_env_value.trim().to_string();
                if !k.is_empty() && !v.is_empty() {
                    self.deploy_env_vars.push((k, v));
                    self.deploy_env_key.clear();
                    self.deploy_env_value.clear();
                }
            }
            Message::RemoveDEVar(i) => {
                if i < self.deploy_env_vars.len() {
                    self.deploy_env_vars.remove(i);
                }
            }
            Message::SubmitDeploy => {
                if self.deploying || self.deploy_path.is_empty() {
                    return Task::none();
                }
                self.deploying = true;
                self.deploy_log.clear();
                self.deploy_log.push("Packaging...".into());
                let name = self.deploy_name.clone();
                let manifest = build_toml(
                    &self.deploy_name,
                    &self.deploy_runtime,
                    &self.deploy_build,
                    &self.deploy_run,
                    &self.deploy_env_vars,
                );
                let path = self.deploy_path.clone();
                if let Some(c) = self.client() {
                    let c2 = c.clone();
                    return Task::perform(
                        async move {
                            let archive =
                                package_src(&path).map_err(|e| format!("packaging: {e}"))?;
                            let rt = rt_proto(&manifest);
                            let mut rx = c2
                                .lock()
                                .await
                                .deploy_stream(name, rt, manifest, archive)
                                .await?;
                            let mut evs = vec![];
                            while let Some(e) = rx.recv().await {
                                match e {
                                    Ok(ev) => evs.push(ev),
                                    Err(e) => {
                                        evs.push(rover_proto::v1::DeployEvent {
                                            event: Some(
                                                rover_proto::v1::deploy_event::Event::Error(
                                                    rover_proto::v1::DeployError { message: e },
                                                ),
                                            ),
                                        });
                                        break;
                                    }
                                }
                            }
                            Ok(evs)
                        },
                        |r| match r {
                            Ok(evs) => Message::DeployDone(evs),
                            Err(e) => Message::DeployErr(e),
                        },
                    );
                }
            }
            Message::DeployDone(evs) => {
                for e in &evs {
                    if let Some(ref ev) = e.event {
                        match ev {
                            rover_proto::v1::deploy_event::Event::Log(l) => self.deploy_log.push(
                                format!("{}{}", if l.is_stderr { "[err] " } else { "" }, l.line),
                            ),
                            rover_proto::v1::deploy_event::Event::Complete(c) => {
                                self.deploy_log.push(format!("✅ Deployed — {}", c.app_id))
                            }
                            rover_proto::v1::deploy_event::Event::Error(e) => {
                                self.deploy_log.push(format!("❌ {}", e.message))
                            }
                            _ => {}
                        }
                    }
                }
                self.deploying = false;
                return Task::done(Message::RefreshApps);
            }
            Message::DeployErr(e) => {
                self.deploying = false;
                self.deploy_log.push(format!("❌ {e}"));
            }

            // Env vars (on detail)
            Message::SetEKey(v) => self.env_key = v,
            Message::SetEValue(v) => self.env_value = v,
            Message::AddEnv => {
                if let (Some(sid), Some(c)) = (&self.selected_app, self.client()) {
                    let c2 = c.clone();
                    let id = sid.clone();
                    let key = self.env_key.clone();
                    let val = self.env_value.clone();
                    self.env_key.clear();
                    self.env_value.clear();
                    return Task::perform(
                        async move {
                            let mut cl = c2.lock().await;
                            let mut vars = std::collections::HashMap::new();
                            vars.insert(key, val);
                            cl.set_env(&id, vars).await?;
                            cl.get_app(&id).await.map(Box::new)
                        },
                        |r| match r {
                            Ok(d) => Message::Detail(d),
                            Err(e) => Message::Toast(e),
                        },
                    );
                }
            }

            Message::Tick => {
                if let Some(d) = self.dev() {
                    if d.connected {
                        let mut tasks = vec![
                            Task::done(Message::Refresh),
                            Task::done(Message::RefreshApps),
                        ];
                        if self.selected_app.is_some() {
                            if let (Some(sid), Some(c)) = (&self.selected_app, self.client()) {
                                let c2 = c.clone();
                                let id = sid.clone();
                                tasks.push(Task::perform(
                                    async move {
                                        let mut rx = c2.lock().await.stream_logs(&id, 500).await?;
                                        let mut lines: Vec<String> = vec![];
                                        while let Some(Ok(e)) = rx.recv().await {
                                            lines.push(format!(
                                                "{} {}",
                                                if e.is_stderr { "[err]" } else { "" },
                                                e.line
                                            ));
                                        }
                                        Ok(lines)
                                    },
                                    |r: Result<Vec<String>, String>| match r {
                                        Ok(l) => Message::Logs(l),
                                        Err(e) => Message::Toast(e),
                                    },
                                ));
                            }
                        }
                        return Task::batch(tasks);
                    }
                }
            }
            Message::Toast(m) => self.toasts.push(m),
            Message::Dismiss(i) => {
                if i < self.toasts.len() {
                    self.toasts.remove(i);
                }
            }
            Message::Noop => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar();
        let content = self.content();
        let status = self.status();

        let body = column![row![sidebar, content].height(Length::Fill), status];

        // Deploy modal
        let body: Element<_> = if self.deploy_open {
            let modal = self.deploy_modal();
            let overlay = container(modal)
                .center(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_: &iced::Theme| container::Style {
                    background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.6).into()),
                    ..container::Style::default()
                });
            iced::widget::stack([
                container(body)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                overlay.into(),
            ])
            .into()
        } else {
            body.into()
        };

        // Toasts
        let body: Element<_> = if self.toasts.is_empty() {
            body
        } else {
            let tc: Vec<Element<_>> = self
                .toasts
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    button(text(m).size(11).color(C::TEXT))
                        .style(button::text)
                        .on_press(Message::Dismiss(i))
                        .into()
                })
                .collect();
            iced::widget::stack([
                container(body)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                container(column(tc).spacing(4).padding(8))
                    .padding(12)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            ])
            .into()
        };

        // Delete confirm
        if let Some((ref id, ref name)) = self.confirm_delete {
            let modal = container(
                column![
                    text(format!("Delete {name}?")).size(16),
                    text("This cannot be undone.").size(12).color(C::TEXT_MUTED),
                    Space::new(0, 12),
                    row![
                        button(text("Cancel"))
                            .style(button::text)
                            .on_press(Message::CancelDelete),
                        Space::new(8, 0),
                        button(text("Delete"))
                            .style(button::danger)
                            .on_press(Message::ConfirmDelete((id.clone(), name.clone())))
                    ]
                    .spacing(8)
                ]
                .spacing(8)
                .padding(20),
            )
            .style(|_: &iced::Theme| bg_c(C::ELEVATED, C::DANGER));
            let overlay = container(modal)
                .center(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill);
            iced::widget::stack([body, overlay.into()]).into()
        } else {
            body
        }
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::Tick)
    }

    // ── Sidebar ────────────────────────────────────────────────

    fn sidebar(&self) -> Element<'_, Message> {
        let header = text("Devices").size(13).color(C::TEXT_MUTED);
        let mut items: Vec<Element<'_, Message>> = vec![container(header).padding(12).into()];

        for (i, dev) in self.devices.iter().enumerate() {
            let active = i == self.active;
            let (dot, c) = if dev.connecting {
                ("◌", C::WARNING)
            } else if dev.connected {
                ("●", C::SUCCESS)
            } else {
                ("○", C::TEXT_MUTED)
            };
            let name_color = if active { C::TEXT } else { C::TEXT_MUTED };
            let row = column![
                text(format!("{dot}  {}", dev.profile.name))
                    .size(13)
                    .color(name_color),
                text(&dev.profile.address).size(10).color(C::TEXT_MUTED)
            ]
            .spacing(2)
            .padding(10)
            .width(Length::Fill);
            if active {
                items.push(
                    container(row)
                        .style(|_: &iced::Theme| bg_c(C::SURFACE, C::ACCENT))
                        .width(Length::Fill)
                        .into(),
                );
            } else {
                items.push(
                    button(row)
                        .style(button::text)
                        .on_press(Message::Select(i))
                        .width(Length::Fill)
                        .into(),
                );
            }
        }

        items.push(
            button(row![text("+  Connect a device").size(13).color(C::TEXT_MUTED)].padding(10))
                .style(button::text)
                .width(Length::Fill)
                .on_press(Message::ShowAdd)
                .into(),
        );

        if self.show_add {
            let form = column![
                text_input("Address", &self.addr)
                    .on_input(Message::SetAddr)
                    .size(12)
                    .padding(6),
                Space::new(0, 4),
                text_input("Pairing token", &self.token)
                    .on_input(Message::SetToken)
                    .size(12)
                    .padding(6),
                Space::new(0, 4),
                text_input("Name", &self.name)
                    .on_input(Message::SetName)
                    .size(12)
                    .padding(6),
                Space::new(0, 6),
                row![
                    button(text("Connect").size(12))
                        .style(button::primary)
                        .on_press(Message::Connect),
                    Space::new(4, 0),
                    button(text("Cancel").size(12))
                        .style(button::text)
                        .on_press(Message::HideAdd)
                ]
                .spacing(4)
            ]
            .spacing(2)
            .padding(12);
            items.push(
                container(form)
                    .style(|_: &iced::Theme| bg_c(C::SURFACE, C::BORDER))
                    .into(),
            );
        }
        if let Some(ref e) = self.error {
            items.push(text(e).size(11).color(C::DANGER).into());
        }

        container(scrollable(column(items).spacing(0)))
            .style(|_: &iced::Theme| bg_c(C::SURFACE, C::BORDER))
            .width(220)
            .height(Length::Fill)
            .into()
    }

    // ── Content ────────────────────────────────────────────────

    fn content(&self) -> Element<'_, Message> {
        match self.dev() {
            Some(d) if d.connected => {
                if self.selected_app.is_some() {
                    self.detail_view(d)
                } else {
                    self.dash_view(d)
                }
            }
            Some(d) if d.connecting => {
                container(text("Connecting...").size(16).color(C::TEXT_MUTED))
                    .center(Length::Fill)
                    .into()
            }
            Some(d) if d.err.is_some() => container(
                column![
                    text("Connection failed").size(16).color(C::DANGER),
                    text(d.err.as_ref().unwrap()).size(13).color(C::TEXT_MUTED),
                    Space::new(0, 12),
                    button(text("Retry")).on_press(Message::Select(self.active))
                ]
                .spacing(8)
                .width(300),
            )
            .center(Length::Fill)
            .into(),
            Some(_) => container(
                text("Select a device and connect")
                    .size(16)
                    .color(C::TEXT_MUTED),
            )
            .center(Length::Fill)
            .into(),
            None => container(
                text("Add a device to get started")
                    .size(16)
                    .color(C::TEXT_MUTED),
            )
            .center(Length::Fill)
            .into(),
        }
    }

    fn dash_view<'a>(&'a self, dev: &'a DeviceState) -> Element<'a, Message> {
        let header = row![
            text(&dev.profile.name).size(22),
            Space::new(Length::Fill, 0),
            button(text("Deploy").size(13))
                .style(button::primary)
                .on_press(Message::OpenDeploy)
        ];
        let info = if let Some(ref i) = dev.info {
            text(format!(
                "{} | {} | uptime {}",
                i.name,
                i.os,
                fmt_uptime(i.uptime_seconds)
            ))
            .size(12)
            .color(C::TEXT_MUTED)
        } else {
            text("").size(12)
        };
        let metrics: Element<_> = if let Some(ref m) = dev.metrics {
            row![
                metric("CPU", format!("{:.0}%", m.cpu_percent), C::ACCENT),
                Space::new(8, 0),
                metric(
                    "RAM",
                    format!("{:.0} MB", m.ram_used_bytes as f64 / 1_048_576.0),
                    C::SUCCESS
                ),
            ]
            .into()
        } else {
            Space::new(0, 0).into()
        };

        let cards: Vec<Element<_>> = dev
            .apps
            .iter()
            .map(|a| {
                let (s, sc) = status_info(a.status);
                let (r, _) = runtime_info(a.runtime);
                button(container(
                    column![
                        row![
                            text(&a.name).size(14),
                            Space::new(Length::Fill, 0),
                            container(text(s).size(10))
                                .style(move |_: &iced::Theme| container::Style {
                                    background: Some(Color { a: 0.15, ..sc }.into()),
                                    border: iced::Border {
                                        color: sc,
                                        width: 1.0,
                                        radius: 4.0.into()
                                    },
                                    ..container::Style::default()
                                })
                                .padding(4)
                        ],
                        Space::new(0, 4),
                        text(format!("{r} | {}", &a.app_id[..a.app_id.len().min(8)]))
                            .size(11)
                            .color(C::TEXT_MUTED)
                    ]
                    .padding(12),
                ))
                .style(button::text)
                .width(Length::Fill)
                .on_press(Message::SelectApp(a.app_id.clone()))
                .into()
            })
            .collect();

        let apps: Element<_> = if cards.is_empty() {
            container(text("No apps deployed").size(14).color(C::TEXT_MUTED))
                .style(|_: &iced::Theme| dashed())
                .padding(24)
                .width(Length::Fill)
                .center(Length::Fill)
                .into()
        } else {
            column(cards).spacing(6).into()
        };

        scrollable(
            column![
                header,
                Space::new(0, 4),
                info,
                Space::new(0, 16),
                metrics,
                Space::new(0, 24),
                text("Applications").size(16),
                Space::new(0, 8),
                apps
            ]
            .spacing(0)
            .padding(32)
            .width(Length::Fill),
        )
        .into()
    }

    fn detail_view(&self, _dev: &DeviceState) -> Element<'_, Message> {
        let Some(ref d) = self.app_detail else {
            return container(text("Loading...").size(16).color(C::TEXT_MUTED))
                .center(Length::Fill)
                .into();
        };
        let (s, sc) = status_info(d.status);
        let (r, _) = runtime_info(d.runtime);
        let log_lines: Vec<Element<_>> = self
            .log_entries
            .iter()
            .map(|l| {
                text(l)
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .color(C::TEXT_MUTED)
                    .into()
            })
            .collect();
        let env_vars: Vec<Element<_>> = d
            .env_vars
            .iter()
            .map(|(k, v)| {
                text(format!("{k}={v}"))
                    .size(12)
                    .color(C::TEXT_MUTED)
                    .into()
            })
            .collect();

        scrollable(
            column![
                row![
                    button(text("← Back"))
                        .style(button::text)
                        .on_press(Message::Back),
                    Space::new(12, 0),
                    text(&d.name).size(22),
                    Space::new(12, 0),
                    container(text(s).size(12).color(sc))
                        .style(move |_: &iced::Theme| container::Style {
                            background: Some(Color { a: 0.12, ..sc }.into()),
                            border: iced::Border {
                                color: sc,
                                width: 1.0,
                                radius: 4.0.into()
                            },
                            ..container::Style::default()
                        })
                        .padding(4)
                ],
                Space::new(0, 8),
                text(format!(
                    "{r} | PID {} | Restarts {}",
                    d.pid.map_or("-".into(), |p| p.to_string()),
                    d.restart_count
                ))
                .size(12)
                .color(C::TEXT_MUTED),
                Space::new(0, 4),
                text(format!("build: {}", d.build_command))
                    .size(11)
                    .color(C::TEXT_MUTED),
                text(format!("run: {}", d.run_command))
                    .size(11)
                    .color(C::TEXT_MUTED),
                Space::new(0, 16),
                row![
                    button(text("Start"))
                        .style(button::primary)
                        .on_press(Message::Start(d.app_id.clone())),
                    Space::new(6, 0),
                    button(text("Stop")).on_press(Message::Stop(d.app_id.clone())),
                    Space::new(6, 0),
                    button(text("Restart")).on_press(Message::Restart(d.app_id.clone())),
                    Space::new(6, 0),
                    button(text("Delete"))
                        .style(button::danger)
                        .on_press(Message::Delete(d.app_id.clone()))
                ],
                Space::new(0, 20),
                text("Environment").size(14).color(C::TEXT_MUTED),
                Space::new(0, 6),
                row![
                    text_input("KEY", &self.env_key)
                        .on_input(Message::SetEKey)
                        .size(12)
                        .padding(6)
                        .width(140),
                    Space::new(6, 0),
                    text_input("VALUE", &self.env_value)
                        .on_input(Message::SetEValue)
                        .size(12)
                        .padding(6)
                        .width(220),
                    Space::new(6, 0),
                    button(text("Add").size(12)).on_press(Message::AddEnv)
                ],
                Space::new(0, 8),
                if env_vars.is_empty() {
                    let empty: Element<'_, Message> = text("No environment variables set.")
                        .size(12)
                        .color(C::TEXT_MUTED)
                        .into();
                    empty
                } else {
                    column(env_vars).spacing(2).into()
                },
                Space::new(0, 20),
                text("Logs").size(14).color(C::TEXT_MUTED),
                Space::new(0, 4),
                container(scrollable(column(log_lines).spacing(1)))
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(Color::from_rgb(0.04, 0.04, 0.06).into()),
                        border: iced::Border {
                            color: C::BORDER,
                            width: 1.0,
                            radius: 6.0.into()
                        },
                        ..container::Style::default()
                    })
                    .padding(10)
                    .height(200),
            ]
            .spacing(0)
            .padding(32)
            .width(Length::Fill),
        )
        .into()
    }

    fn deploy_modal(&self) -> Element<'_, Message> {
        let env_rows: Vec<Element<Message>> = self
            .deploy_env_vars
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                row![
                    text(format!("{k}={v}"))
                        .size(12)
                        .color(C::TEXT_MUTED)
                        .width(Length::Fill),
                    button(text("✕").size(12))
                        .style(button::text)
                        .on_press(Message::RemoveDEVar(i))
                ]
                .align_y(iced::Alignment::Center)
                .into()
            })
            .collect();
        let log_section: Element<_> = if self.deploy_log.is_empty() {
            Space::new(0, 0).into()
        } else {
            container(scrollable(
                column(
                    self.deploy_log
                        .iter()
                        .map(|l| {
                            text(l)
                                .size(11)
                                .font(iced::Font::MONOSPACE)
                                .color(C::TEXT_MUTED)
                                .into()
                        })
                        .collect::<Vec<Element<Message>>>(),
                )
                .spacing(1),
            ))
            .style(|_: &iced::Theme| container::Style {
                background: Some(Color::from_rgb(0.04, 0.04, 0.06).into()),
                border: iced::Border {
                    color: C::BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            })
            .padding(10)
            .height(150)
            .into()
        };
        container(
            column![
                row![
                    text("Deploy Application").size(18),
                    Space::new(Length::Fill, 0),
                    button(text("✕").size(14))
                        .style(button::text)
                        .on_press(Message::CloseDeploy)
                ],
                Space::new(0, 16),
                text_input("App name", &self.deploy_name)
                    .on_input(Message::SetDName)
                    .size(13)
                    .padding(8),
                Space::new(0, 8),
                pick_list(
                    &["python", "node", "go", "rust"][..],
                    Some(&self.deploy_runtime[..]),
                    |s| Message::SetDRuntime(s.to_string())
                )
                .placeholder("Runtime"),
                Space::new(0, 8),
                text_input("Build command", &self.deploy_build)
                    .on_input(Message::SetDBuild)
                    .size(13)
                    .padding(8),
                text_input("Run command", &self.deploy_run)
                    .on_input(Message::SetDRun)
                    .size(13)
                    .padding(8),
                Space::new(0, 4),
                row![
                    text_input("Source directory", &self.deploy_path)
                        .on_input(Message::SetDPath)
                        .size(13)
                        .padding(8)
                        .width(Length::Fill),
                    Space::new(8, 0),
                    button(text("Browse").size(13)).on_press(Message::PickPath)
                ],
                Space::new(0, 12),
                text("Environment Variables").size(12).color(C::TEXT_MUTED),
                Space::new(0, 4),
                if !env_rows.is_empty() {
                    let c: iced::widget::Column<'_, Message> = column(env_rows);
                    c.spacing(2).into()
                } else {
                    let empty: Element<'_, Message> =
                        text("None").size(11).color(C::TEXT_MUTED).into();
                    empty
                },
                row![
                    text_input("KEY", &self.deploy_env_key)
                        .on_input(Message::SetDEKey)
                        .size(12)
                        .padding(6)
                        .width(140),
                    Space::new(6, 0),
                    text_input("VALUE", &self.deploy_env_value)
                        .on_input(Message::SetDEValue)
                        .size(12)
                        .padding(6)
                        .width(200),
                    Space::new(6, 0),
                    button(text("+ Add").size(12)).on_press(Message::AddDEVar)
                ],
                Space::new(0, 12),
                button(text(if self.deploying {
                    "Deploying..."
                } else {
                    "Deploy"
                }))
                .style(button::primary)
                .width(Length::Fill)
                .on_press_maybe(
                    if self.deploying || self.deploy_path.is_empty() {
                        None
                    } else {
                        Some(Message::SubmitDeploy)
                    }
                ),
                Space::new(0, 8),
                log_section,
            ]
            .spacing(0)
            .padding(28),
        )
        .style(|_: &iced::Theme| box_style(C::ELEVATED, C::BORDER, 12.0))
        .width(580)
        .into()
    }

    fn status(&self) -> Element<'_, Message> {
        let txt = match self.dev() {
            Some(d) if d.connecting => format!("◌ Connecting to {}...", d.profile.name),
            Some(d) if d.connected => format!("● {} — {}", d.profile.name, d.profile.address),
            Some(d) if d.err.is_some() => format!("✕ {}", d.profile.name),
            Some(d) => format!("○ {} — disconnected", d.profile.name),
            None => "No devices".into(),
        };
        container(text(txt).size(11).color(C::TEXT_MUTED))
            .style(|_: &iced::Theme| bg_c(C::SURFACE, C::BORDER))
            .padding(8)
            .width(Length::Fill)
            .into()
    }
}

// ── Colors ────────────────────────────────────────────────────

mod C {
    use iced::Color;
    pub const BG: Color = Color::from_rgb(0.039, 0.039, 0.059);
    pub const SURFACE: Color = Color::from_rgb(0.067, 0.067, 0.094);
    pub const ELEVATED: Color = Color::from_rgb(0.094, 0.094, 0.125);
    pub const BORDER: Color = Color::from_rgb(0.118, 0.118, 0.165);
    pub const TEXT: Color = Color::from_rgb(0.894, 0.894, 0.925);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.545, 0.545, 0.620);
    pub const ACCENT: Color = Color::from_rgb(0.388, 0.400, 0.945);
    pub const SUCCESS: Color = Color::from_rgb(0.133, 0.773, 0.369);
    pub const WARNING: Color = Color::from_rgb(0.961, 0.620, 0.043);
    pub const DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267);
}

fn bg_c(bg: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(bg.into()),
        border: iced::Border {
            color: border,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}
fn box_style(bg: Color, border: Color, r: f32) -> container::Style {
    container::Style {
        background: Some(bg.into()),
        border: iced::Border {
            color: border,
            width: 1.0,
            radius: r.into(),
        },
        ..container::Style::default()
    }
}
fn dashed() -> container::Style {
    container::Style {
        border: iced::Border {
            color: C::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}
fn status_info(s: i32) -> (&'static str, Color) {
    match s {
        1 => ("deploying", C::WARNING),
        2 => ("starting", C::WARNING),
        3 => ("running", C::SUCCESS),
        4 => ("stopped", C::TEXT_MUTED),
        5 => ("crashed", C::DANGER),
        6 => ("failed", C::DANGER),
        _ => ("?", C::TEXT_MUTED),
    }
}
fn runtime_info(r: i32) -> (&'static str, Color) {
    match r {
        1 => ("python", C::ACCENT),
        2 => ("node", C::SUCCESS),
        3 => ("go", Color::from_rgb(0.024, 0.714, 0.831)),
        4 => ("rust", C::WARNING),
        _ => ("?", C::TEXT_MUTED),
    }
}
fn metric(label: &'static str, value: String, color: Color) -> Element<'static, Message> {
    container(
        column![
            text(label).size(10).color(C::TEXT_MUTED),
            text(value).size(16).color(color)
        ]
        .spacing(2)
        .padding(12),
    )
    .style(|_: &iced::Theme| bg_c(C::SURFACE, C::BORDER))
    .into()
}
fn fmt_uptime(s: u32) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

fn build_toml(name: &str, rt: &str, build: &str, run: &str, env: &[(String, String)]) -> String {
    let mut app = toml::map::Map::new();
    app.insert("name".into(), toml::Value::String(name.into()));
    app.insert("runtime".into(), toml::Value::String(rt.into()));
    let mut b = toml::map::Map::new();
    b.insert("command".into(), toml::Value::String(build.into()));
    let mut r = toml::map::Map::new();
    r.insert("command".into(), toml::Value::String(run.into()));
    let mut root = toml::map::Map::new();
    root.insert("app".into(), toml::Value::Table(app));
    root.insert("build".into(), toml::Value::Table(b));
    root.insert("run".into(), toml::Value::Table(r));
    if !env.is_empty() {
        let mut e = toml::map::Map::new();
        for (k, v) in env {
            e.insert(k.clone(), toml::Value::String(v.clone()));
        }
        root.insert("env".into(), toml::Value::Table(e));
    }
    toml::to_string_pretty(&root).unwrap_or_default()
}
fn rt_proto(m: &str) -> i32 {
    if m.contains("python") {
        1
    } else if m.contains("node") {
        2
    } else if m.contains("go") {
        3
    } else if m.contains("rust") {
        4
    } else {
        1
    }
}

fn package_src(dir: &str) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;
    let mut archive = tar::Builder::new(Vec::new());
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !n.starts_with('.')
            && n != "target"
            && n != "node_modules"
            && n != "__pycache__"
            && n != ".git"
    }) {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(dir)?;
        if rel.as_os_str().is_empty() || !path.is_file() {
            continue;
        }
        archive.append_path_with_name(path, rel)?;
    }
    let data = archive.into_inner()?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&data)?;
    Ok(encoder.finish()?)
}

async fn connect_and_pair(
    addr: String,
    token: String,
    _name: String,
) -> Result<(ClientRef, String), String> {
    let mut c = RoverClient::connect(&addr).await?;
    let resp = c.pair(&token).await?;
    Ok((Arc::new(Mutex::new(c)), resp.api_key))
}
async fn connect_with_key(addr: String, key: String) -> Result<ClientRef, String> {
    let mut c = RoverClient::connect(&addr).await?;
    c.set_api_key(&key);
    c.get_info().await?;
    Ok(Arc::new(Mutex::new(c)))
}

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rover=info,info".into()),
        )
        .init();
    iced::application(RoverApp::title, RoverApp::update, RoverApp::view)
        .theme(RoverApp::theme)
        .subscription(RoverApp::subscription)
        .window_size(Size::new(1100.0, 750.0))
        .run_with(RoverApp::new)
}
