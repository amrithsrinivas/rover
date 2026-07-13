mod api;
mod app;
mod deploy_job;
mod deploy_update;
mod message;
mod state;
mod theme;
mod update;
mod view;
mod widgets;

use std::sync::Arc;
use std::time::Duration;

use iced::{Size, Subscription, Task};
use tokio::sync::Mutex;

use api::client::RoverClient;
use message::Message;
use rover_core::ConnectionProfileStore;
use state::DeviceState;

pub use app::{DeployState, RoverApp, ToastKind, ToastState};

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    iced::application(title, update::update, view::view)
        .window(iced::window::Settings {
            icon: iced::window::icon::from_file_data(
                include_bytes!("../icon/icon.png"),
                None,
            ).ok(),
            ..iced::window::Settings::default()
        })
        .theme(|_| theme::rover_theme())
        .subscription(subscription)
        .window_size(Size::new(1100.0, 750.0))
        .run_with(init)
}

fn title(app: &RoverApp) -> String {
    if let Some(device) = app.devices.get(app.active) {
        if device.connected {
            return format!("Rover — {}", device.profile.name);
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

    let tasks: Vec<Task<Message>> = devices
        .iter()
        .enumerate()
        .filter_map(|(idx, device)| {
            device.profile.api_key.as_ref().map(|key| {
                let addr = device.profile.address.clone();
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
        deploy_env_file: String::new(),
        deploy_env_vars: Vec::new(),
        deploy_env_key: String::new(),
        deploy_env_value: String::new(),
        next_deploy_id: 1,
        active_deploys: Vec::new(),
        expanded_deploy: None,
        confirm_delete: None,
        confirm_device_delete: None,
        update_open: false,
        update_build: String::new(),
        update_run: String::new(),
        toasts: Vec::new(),
    };

    (app, Task::batch(tasks))
}

fn subscription(_app: &RoverApp) -> Subscription<Message> {
    iced::time::every(Duration::from_secs(2)).map(|_| Message::Tick)
}
