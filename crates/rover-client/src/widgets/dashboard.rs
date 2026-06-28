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
        Space::with_width(8),
        button(text("Disconnect").size(13))
            .style(button::secondary)
            .on_press(Message::Disconnect),
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

    let deploys_section = deploy_activity(app);
    let apps_section = apps_list(app, d);

    let content = column![
        header,
        Space::with_height(4),
        info_bar,
        Space::with_height(16),
        metrics_row,
        Space::with_height(20),
        deploys_section,
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
    let (cpu_text, ram_text, disk_text) = match &d.metrics {
        Some(m) => {
            let cpu = format!("{:.1}%", m.cpu_percent);
            let ram = format!(
                "{} / {}",
                format_bytes(m.ram_used_bytes),
                format_bytes(m.ram_total_bytes)
            );
            let disk = format!(
                "{} / {}",
                format_bytes(m.disk_used_bytes),
                format_bytes(m.disk_total_bytes)
            );
            (cpu, ram, disk)
        }
        None => ("--".to_string(), "--".to_string(), "--".to_string()),
    };

    let cpu_card = metric_card("CPU", cpu_text, colors::ACCENT);
    let ram_card = metric_card("RAM", ram_text, colors::SUCCESS);
    let disk_card = metric_card("Disk", disk_text, colors::WARNING);

    row![
        cpu_card,
        Space::with_width(12),
        ram_card,
        Space::with_width(12),
        disk_card
    ]
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

fn deploy_activity(app: &RoverApp) -> Element<'_, Message> {
    if app.active_deploys.is_empty() {
        return Space::with_height(0).into();
    }

    let active_count = app.active_deploy_count();
    let mut header = row![
        text("Deployments").size(14).color(colors::TEXT_MUTED),
        Space::with_width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    if active_count == 0 {
        header = header.push(
            button(text("Clear finished").size(11))
                .style(button::text)
                .on_press(Message::ClearFinishedDeploys),
        );
    } else {
        header = header.push(
            text(format!("{active_count} running"))
                .size(11)
                .color(colors::ACCENT),
        );
    }

    let cards: Vec<Element<Message>> = app
        .active_deploys
        .iter()
        .rev()
        .map(|deploy| deploy_card(app, deploy))
        .collect();

    column![header, Space::with_height(8), column(cards).spacing(8)]
        .spacing(0)
        .into()
}

fn deploy_card<'a>(app: &'a RoverApp, deploy: &'a crate::DeployState) -> Element<'a, Message> {
    let status_color = deploy_status_color(&deploy.status);
    let latest = deploy.latest_log().unwrap_or("No build output yet");
    let expanded = app.expanded_deploy == Some(deploy.id);
    let toggle_label = if expanded { "Hide log" } else { "View log" };

    let mut actions = row![
        button(text(toggle_label).size(11))
            .style(button::text)
            .on_press(Message::ToggleDeployLog(deploy.id)),
        Space::with_width(8),
        button(text("Copy log").size(11))
            .style(button::text)
            .on_press(Message::Copy(deploy.logs.join("\n"))),
    ]
    .align_y(Alignment::Center);

    if let Some(app_id) = &deploy.app_id {
        actions = actions.push(Space::with_width(8)).push(
            button(text("View app").size(11))
                .style(button::text)
                .on_press(Message::SelectApp(app_id.clone())),
        );
    }

    let mut body = column![
        row![
            column![
                text(&deploy.name).size(13).color(colors::TEXT),
                Space::with_height(2),
                text(format!(
                    "{} · {}",
                    deploy.runtime,
                    truncate_middle(&deploy.source_path, 56)
                ))
                .size(11)
                .color(colors::TEXT_MUTED),
            ]
            .spacing(0)
            .width(Length::Fill),
            status_badge(&deploy.status, status_color),
        ]
        .align_y(Alignment::Center),
        Space::with_height(8),
        text(latest).size(11).color(colors::TEXT_MUTED),
        Space::with_height(8),
        actions,
    ]
    .spacing(0)
    .padding(12);

    if expanded {
        let log_lines: Vec<Element<Message>> = deploy
            .logs
            .iter()
            .map(|line| {
                text(line)
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .color(colors::TEXT)
                    .into()
            })
            .collect();
        let log = container(scrollable(column(log_lines).spacing(1)).height(Length::Fixed(140.0)))
            .padding(10)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.02, 0.02, 0.04,
                ))),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            });
        body = body.push(Space::with_height(8)).push(log);
    }

    container(body)
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

fn deploy_status_color(status: &str) -> Color {
    match status {
        "complete" => colors::SUCCESS,
        "failed" => colors::DANGER,
        "packaging" | "sending" | "building" | "starting" => colors::ACCENT,
        _ => colors::WARNING,
    }
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let keep = max_chars.saturating_sub(3) / 2;
    let start: String = value.chars().take(keep).collect();
    let end: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}...{end}")
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
