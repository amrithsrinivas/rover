use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length};

use crate::RoverApp;
use crate::message::Message;
use crate::theme::{colors, format_bytes, format_uptime, with_alpha};

/// Render the dashboard for a connected device.
pub fn dashboard(app: &RoverApp) -> Element<'_, Message> {
    let d = match app.devices.get(app.active) {
        Some(d) => d,
        None => {
            return container(text("No device selected").color(colors::TEXT_MUTED))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }
    };

    let info = match &d.info {
        Some(info) => info,
        None => {
            return container(text("Connecting...").color(colors::TEXT_MUTED))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }
    };

    let header = row![
        text(&d.profile.name)
            .size(22)
            .color(colors::TEXT)
            .width(Length::Fill),
        deploy_button(),
    ]
    .align_y(Alignment::Center)
    .spacing(12);

    let info_bar = row![
        text(&info.hostname).size(12).color(colors::TEXT_MUTED),
        text(" | ").size(12).color(colors::BORDER),
        text(&info.os).size(12).color(colors::TEXT_MUTED),
        text(" | ").size(12).color(colors::BORDER),
        text(format!("uptime {}", format_uptime(info.uptime_seconds)))
            .size(12)
            .color(colors::TEXT_MUTED),
    ]
    .spacing(6);

    let metrics_row = metrics_cards(d);

    let apps_section = apps_list(app, d);

    let content = column![
        header,
        Space::with_height(4),
        info_bar,
        Space::with_height(16),
        metrics_row,
        Space::with_height(20),
        text("Applications").size(14).color(colors::TEXT_MUTED),
        Space::with_height(8),
        apps_section,
    ]
    .spacing(0)
    .padding(24);

    scrollable(content).height(Length::Fill).into()
}

fn deploy_button() -> Element<'static, Message> {
    button(text("+ Deploy").size(14))
        .style(button::primary)
        .on_press(Message::OpenDeploy)
        .into()
}

fn metrics_cards(d: &crate::state::DeviceState) -> Element<'_, Message> {
    let (cpu_text, ram_text) = match &d.metrics {
        Some(m) => {
            let cpu = format!("{:.1}%", m.cpu_percent);
            let ram = format!(
                "{} / {}",
                format_bytes(m.ram_used_bytes),
                format_bytes(m.ram_total_bytes)
            );
            (cpu, ram)
        }
        None => ("--".to_string(), "--".to_string()),
    };

    let cpu_card = metric_card("CPU", cpu_text, colors::ACCENT);
    let ram_card = metric_card("RAM", ram_text, colors::SUCCESS);

    row![cpu_card, Space::with_width(12), ram_card]
        .width(Length::Fill)
        .into()
}

fn metric_card<'a>(label: &'static str, value: String, color: Color) -> Element<'a, Message> {
    container(
        column![
            text(label).size(10).color(colors::TEXT_MUTED),
            Space::with_height(4),
            text(value).size(22).color(color),
        ]
        .spacing(0)
        .padding(16),
    )
    .width(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(colors::ELEVATED)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn apps_list<'a>(_app: &'a RoverApp, d: &'a crate::state::DeviceState) -> Element<'a, Message> {
    let apps = &d.apps;

    if apps.is_empty() {
        return container(
            container(text("No apps deployed").size(13).color(colors::TEXT_MUTED))
                .center_x(Length::Fill)
                .padding(24),
        )
        .style(|_theme| container::Style {
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill)
        .into();
    }

    let cards: Vec<Element<Message>> = apps.iter().map(|a| app_card(a)).collect();

    column(cards).spacing(8).into()
}

fn app_card(app_summary: &rover_proto::v1::AppSummary) -> Element<'_, Message> {
    let status_color = status_color(app_summary.status);
    let status_label = status_label(app_summary.status);
    let runtime_name = runtime_name(app_summary.runtime);
    let runtime_color = runtime_color(app_summary.runtime);

    let truncated_id: String = app_summary.app_id.chars().take(8).collect();

    let row_content = container(
        row![
            column![
                text(&app_summary.name).size(14).color(colors::TEXT),
                Space::with_height(2),
                row![
                    text(runtime_name).size(11).color(runtime_color),
                    text(" · ").size(11).color(colors::TEXT_MUTED),
                    text(truncated_id).size(11).color(colors::TEXT_MUTED),
                ]
                .spacing(4),
            ]
            .spacing(2)
            .width(Length::Fill),
            status_badge(status_label, status_color),
        ]
        .align_y(Alignment::Center)
        .padding(12),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(colors::ELEVATED)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    });

    let app_id = app_summary.app_id.clone();
    button(row_content)
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::SelectApp(app_id))
        .into()
}

fn status_badge<'a>(label: &'a str, color: Color) -> Element<'a, Message> {
    container(text(label).size(11).color(color))
        .padding([2, 8])
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(with_alpha(color, 0.15))),
            border: iced::Border {
                color,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn status_color(status: i32) -> Color {
    match status {
        1 => colors::WARNING,
        2 => colors::WARNING,
        3 => colors::SUCCESS,
        4 => colors::TEXT_MUTED,
        5 => colors::DANGER,
        6 => colors::DANGER,
        _ => colors::TEXT_MUTED,
    }
}

fn status_label(status: i32) -> &'static str {
    match status {
        1 => "deploying",
        2 => "starting",
        3 => "running",
        4 => "stopped",
        5 => "crashed",
        6 => "failed",
        _ => "unknown",
    }
}

fn runtime_name(runtime: i32) -> &'static str {
    match runtime {
        1 => "python",
        2 => "node",
        3 => "go",
        4 => "rust",
        _ => "unknown",
    }
}

fn runtime_color(runtime: i32) -> Color {
    match runtime {
        1 => colors::ACCENT,
        2 => colors::SUCCESS,
        3 => colors::WARNING,
        4 => colors::WARNING,
        _ => colors::TEXT_MUTED,
    }
}
