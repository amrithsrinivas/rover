mod api;
mod message;
mod screens;
mod state;
mod theme;
mod widgets;

use crate::message::Message;
use iced::{Element, Size, Task};

/// Navigation screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connections,
    Dashboard,
    AppDetail,
    Deploy,
    Terminal,
}

/// State of the connection to the Rover server.
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error { message: String },
}

/// Top-level application state.
pub struct RoverApp {
    pub screen: Screen,
    pub connection_state: ConnectionState,
    pub profiles: rover_core::ConnectionProfileStore,
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
                connection_state: ConnectionState::Disconnected,
                profiles,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        match &self.connection_state {
            ConnectionState::Connected => "Rover — Connected".to_string(),
            _ => "Rover".to_string(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(screen) => {
                self.screen = screen;
            }
            Message::Tick => {
                // Periodic refresh — to be implemented
            }
            _ => {
                // Other messages handled in later phases
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        // Placeholder — full UI in Phase 1B
        iced::widget::text("Rover").size(32).into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

// Entry point
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
