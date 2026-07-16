mod api;
mod app;
mod message;
mod screens;
mod theme;
mod update;
mod view;
mod widgets;

use std::sync::Arc;
use std::time::Duration;

use iced::{Settings, Size, Subscription, Task};
use tokio::sync::Mutex;

use api::client::RoverClient;
use app::{RoverApp, Screen, ServerState};
use message::Message;
use rover_core::ConnectionProfileStore;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    iced::application(title, update::update, view::view)
        .window(iced::window::Settings {
            icon: iced::window::icon::from_file_data(include_bytes!("../icon/icon.png"), None).ok(),
            ..iced::window::Settings::default()
        })
        .settings(Settings {
            fonts: vec![lucide_icons::LUCIDE_FONT_BYTES.into()],
            ..Settings::default()
        })
        .theme(|_| theme::rover_theme())
        .subscription(subscription)
        .window_size(Size::new(1100.0, 750.0))
        .run_with(init)
}

fn title(app: &RoverApp) -> String {
    let connected = app.connected_count();
    if connected > 0 {
        format!(
            "Rover — {connected} server{}",
            if connected > 1 { "s" } else { "" }
        )
    } else {
        "Rover".into()
    }
}

fn init() -> (RoverApp, Task<Message>) {
    let store = ConnectionProfileStore::load_from_disk().unwrap_or_default();
    let servers: Vec<ServerState> = store
        .profiles
        .into_iter()
        .map(ServerState::from_profile)
        .collect();
    let show_add = servers.is_empty();

    let connect_tasks: Vec<Task<Message>> = servers
        .iter()
        .enumerate()
        .filter_map(|(idx, server)| {
            server.profile.api_key.as_ref().map(|key| {
                let addr = server.profile.address.clone();
                let key = key.clone();
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
            })
        })
        .collect();

    let app = RoverApp {
        servers,
        all_apps: Vec::new(),
        show_add_form: show_add,
        show_manage_servers: false,
        addr_input: String::new(),
        token_input: String::new(),
        name_input: String::new(),
        form_error: None,
        screen: Screen::Connect,
        app_detail: None,
        app_detail_server: 0,
        log_entries: Vec::new(),
        deploy_open: false,
        deploy_target: None,
        deploy_name: String::new(),
        deploy_runtime: String::new(),
        deploy_build: String::new(),
        deploy_run: String::new(),
        deploy_path: String::new(),
        deploy_use_github: false,
        deploy_github_url: String::new(),
        github_tokens: update::load_github_tokens(),
        selected_github_token: None,
        new_token_label: String::new(),
        new_token_value: String::new(),
        deploy_env_vars: Vec::new(),
        deploy_env_key: String::new(),
        deploy_env_value: String::new(),
        next_deploy_id: 1,
        deploy_jobs: Vec::new(),
        expanded_deploy: None,
        confirm_delete: None,
        confirm_server_delete: None,
        editing_server: None,
        rename_value: String::new(),
        edit_address: String::new(),
        update_open: false,
        update_build: String::new(),
        update_run: String::new(),
        toasts: Vec::new(),
        terminal_open: false,
        terminal_server: 0,
        terminal_output: Vec::new(),
        terminal_input: String::new(),
        terminal_sender: None,
        terminal_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
        terminal_pending: false,
        terminal_last_cmd: String::new(),
    };

    (app, Task::batch(connect_tasks))
}

fn subscription(_app: &RoverApp) -> Subscription<Message> {
    iced::time::every(Duration::from_millis(500)).map(|_| Message::Tick)
}
