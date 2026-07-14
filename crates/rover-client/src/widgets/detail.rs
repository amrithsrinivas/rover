use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::RoverApp;
use crate::message::Message;
use crate::theme::{colors, with_alpha};

/// Render the app detail view for the selected app.
pub fn app_detail(app: &RoverApp) -> Element<'_, Message> {
    let detail = match &app.app_detail {
        Some(d) => d,
        None => {
            return container(text("Loading...").color(colors::TEXT_MUTED))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }
    };

    let back_btn = button(text("\u{2190} Back").size(13))
        .style(button::text)
        .on_press(Message::Back);

    let status_color = detail_status_color(detail.status);
    let status_label = detail_status_label(detail.status);

    let header = row![
        back_btn,
        text(&detail.name).size(22).color(colors::TEXT),
        Space::with_width(8),
        status_badge(status_label, status_color),
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    let runtime_name = detail_runtime_name(detail.runtime);
    let pid_str = detail.pid.map_or("-".to_string(), |p| p.to_string());
    let info_section = row![
        column![
            text("Runtime").size(10).color(colors::TEXT_MUTED),
            text(runtime_name).size(13).color(colors::TEXT),
        ],
        Space::with_width(24),
        column![
            text("PID").size(10).color(colors::TEXT_MUTED),
            text(pid_str).size(13).color(colors::TEXT),
        ],
        Space::with_width(24),
        column![
            text("Restarts").size(10).color(colors::TEXT_MUTED),
            text(format!("{}", detail.restart_count))
                .size(13)
                .color(colors::TEXT),
        ],
    ]
    .spacing(0);

    let commands_section = column![
        text("Build Command").size(10).color(colors::TEXT_MUTED),
        text(&detail.build_command)
            .size(11)
            .color(colors::TEXT_MUTED),
        Space::with_height(6),
        text("Run Command").size(10).color(colors::TEXT_MUTED),
        text(&detail.run_command).size(11).color(colors::TEXT_MUTED),
        Space::with_height(8),
        button(text("Update Commands").size(13))
            .style(button::primary)
            .on_press(Message::OpenUpdate(detail.app_id.clone())),
    ]
    .spacing(2);

    let actions = action_buttons(&detail.app_id);

    // --- Update commands modal ---
    let update_modal = if app.update_open {
        Some(update_modal_content(app))
    } else {
        None
    };

    let delete_modal = app.confirm_delete.as_ref().map(|(app_id, name)| {
        container(
            column![
                text(format!("Delete {name}?")).size(16).color(colors::TEXT),
                Space::with_height(8),
                text("This cannot be undone.")
                    .size(13)
                    .color(colors::TEXT_MUTED),
                Space::with_height(16),
                row![
                    button(text("Cancel").size(13))
                        .style(button::secondary)
                        .on_press(Message::CancelDelete),
                    Space::with_width(8),
                    button(text("Delete").size(13))
                        .style(button::danger)
                        .on_press(Message::ConfirmDelete(app_id.clone(), name.clone(),)),
                ]
                .spacing(0),
            ]
            .padding(24),
        )
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(colors::ELEVATED)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..container::Style::default()
        })
    });

    // Build the base content
    let base = column![
        header,
        Space::with_height(12),
        info_section,
        Space::with_height(12),
        commands_section,
        Space::with_height(16),
        actions,
        Space::with_height(20),
        logs_section(app),
    ]
    .spacing(0)
    .padding(24);

    let content: Element<Message> = if let Some(modal) = delete_modal {
        iced::widget::stack([
            base.into(),
            container(
                container(modal)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
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
    } else if let Some(modal) = update_modal {
        iced::widget::stack([
            base.into(),
            container(
                container(modal)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
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
        base.into()
    };

    scrollable(content).height(Length::Fill).into()
}

/// Render the update commands modal.
fn update_modal_content<'a>(app: &RoverApp) -> Element<'a, Message> {
    let app_id = match &app.app_detail {
        Some(d) => d.app_id.clone(),
        None => String::new(),
    };

    container(
        column![
            text("Update Commands").size(18).color(colors::TEXT),
            Space::with_height(16),
            text("Build Command").size(10).color(colors::TEXT_MUTED),
            Space::with_height(4),
            text_input("build command", &app.update_build)
                .on_input(Message::SetUpdateBuild)
                .size(13),
            Space::with_height(12),
            text("Run Command").size(10).color(colors::TEXT_MUTED),
            Space::with_height(4),
            text_input("run command", &app.update_run)
                .on_input(Message::SetUpdateRun)
                .size(13),
            Space::with_height(20),
            row![
                button(text("Cancel").size(13))
                    .style(button::secondary)
                    .on_press(Message::CloseUpdate),
                Space::with_width(8),
                button(text("Update Commands").size(13))
                    .style(button::primary)
                    .on_press(Message::ConfirmUpdate(app_id)),
            ]
            .spacing(0),
        ]
        .padding(24)
        .width(Length::Fixed(480.0)),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(colors::ELEVATED)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn status_badge<'a>(label: &'a str, color: iced::Color) -> Element<'a, Message> {
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

fn action_buttons<'a>(app_id: &str) -> Element<'a, Message> {
    let app_id = app_id.to_string();

    row![
        button(text("Start").size(13))
            .style(button::primary)
            .on_press(Message::Start(app_id.clone())),
        Space::with_width(8),
        button(text("Stop").size(13))
            .style(button::secondary)
            .on_press(Message::Stop(app_id.clone())),
        Space::with_width(8),
        button(text("Restart").size(13))
            .style(button::secondary)
            .on_press(Message::Restart(app_id.clone())),
        Space::with_width(8),
        button(text("Delete").size(13))
            .style(button::danger)
            .on_press(Message::Delete(app_id.clone())),
    ]
    .spacing(0)
    .into()
}

fn logs_section(app: &RoverApp) -> Element<'_, Message> {
    let log_lines: Vec<Element<Message>> = app
        .log_entries
        .iter()
        .map(|line| {
            text(line)
                .size(11)
                .font(iced::Font::MONOSPACE)
                .color(colors::TEXT)
                .into()
        })
        .collect();

    let log_content = if log_lines.is_empty() {
        container(
            text("No log output yet")
                .size(11)
                .color(colors::TEXT_MUTED)
                .font(iced::Font::MONOSPACE),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
    } else {
        container(scrollable(column(log_lines).spacing(2)).height(Length::Fill))
    };

    let log_container = container(log_content.padding(12)).style(|_theme| container::Style {
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

    let copy_btn = button(text("Copy").size(11).color(colors::TEXT_MUTED))
        .style(button::text)
        .on_press(Message::Copy(app.log_entries.join("\n")));

    column![
        row![
            text("Logs").size(14).color(colors::TEXT_MUTED),
            Space::with_width(Length::Fill),
            copy_btn,
        ]
        .align_y(Alignment::Center),
        Space::with_height(4),
        log_container.height(200),
    ]
    .spacing(0)
    .into()
}

fn detail_status_color(status: i32) -> iced::Color {
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

fn detail_status_label(status: i32) -> &'static str {
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

fn detail_runtime_name(runtime: i32) -> &'static str {
    match runtime {
        1 => "Python",
        2 => "Node.js",
        3 => "Go",
        4 => "Rust",
        _ => "Unknown",
    }
}
