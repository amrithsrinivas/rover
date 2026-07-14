use iced::widget::{container, text};
use iced::{Alignment, Element, Length};

use crate::app::RoverApp;
use crate::message::Message;
use crate::theme;

/// Render the status bar at the top of the main content area.
pub fn status_bar(app: &RoverApp) -> Element<'_, Message> {
    let status_text = build_status_text(app);

    container(
        text(status_text)
            .size(TEXT_MICRO)
            .color(theme::INK_SECONDARY),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(theme::PANEL)),
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .padding([4, 12])
    .width(Length::Fill)
    .align_x(Alignment::Start)
    .align_y(Alignment::Center)
    .height(Length::Fixed(theme::STATUS_BAR_HEIGHT))
    .into()
}

fn build_status_text(app: &RoverApp) -> String {
    if let Some(server) = app.active_server() {
        if server.connected {
            let mut text = format!(
                "● {}    {}    v{}    {}",
                server.profile.name,
                server.profile.address,
                server.info.as_ref().map_or("?", |i| i.version.as_str()),
                server.info.as_ref().map_or(String::from(""), |i| format!(
                    "uptime {}",
                    theme::format_uptime(i.uptime_seconds)
                ))
            );

            let active = app.active_deploy_count();
            if active > 0 {
                text.push_str(&format!("    {active} deploy running"));
            }

            text
        } else if server.connecting {
            format!("◌ Connecting to {}...", server.profile.name)
        } else if server.error.is_some() {
            format!("✕ {} — connection failed", server.profile.name)
        } else {
            format!("○ {} — disconnected", server.profile.name)
        }
    } else if app.servers.is_empty() {
        String::from("No servers configured")
    } else {
        String::from("Select a server")
    }
}

const TEXT_MICRO: u16 = 11;
