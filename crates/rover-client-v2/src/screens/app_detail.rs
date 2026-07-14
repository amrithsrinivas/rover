/// App detail screen — detailed view of a deployed application.
use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{
    icon_arrow_left, icon_copy, icon_play, icon_rotate_cw, icon_settings, icon_square, icon_trash_2,
};

use crate::app::RoverApp;
use crate::message::Message;
use crate::theme;

pub fn app_detail(app: &RoverApp) -> Element<'_, Message> {
    let detail = match &app.app_detail {
        Some(d) => d,
        None => {
            return container(
                text("Loading...")
                    .size(theme::TEXT_BASE)
                    .color(theme::INK_SECONDARY),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }
    };

    let content = column![
        detail_header(app, detail),
        Space::with_height(theme::SPACE_LG),
        info_grid(detail),
        Space::with_height(theme::SPACE_LG),
        commands_section(app, detail),
        Space::with_height(theme::SPACE_LG),
        actions_row(app, detail),
        Space::with_height(theme::SPACE_LG),
        logs_section(app),
    ]
    .spacing(0)
    .padding(theme::SPACE_XL)
    .width(Length::Fill);

    let base: Element<Message> = scrollable(content).height(Length::Fill).into();

    let with_update = if app.update_open {
        stack([base, modal_overlay(update_modal(app))]).into()
    } else {
        base
    };

    if let Some((app_id, name, si)) = &app.confirm_delete {
        let aid = app_id.clone();
        let nm = name.clone();
        let si = *si;
        stack([with_update, modal_overlay(delete_modal(&aid, &nm, si))]).into()
    } else {
        with_update
    }
}

fn detail_header<'a>(
    app: &'a RoverApp,
    detail: &'a rover_proto::v1::AppDetailResponse,
) -> Element<'a, Message> {
    let status_color = detail_status_color(detail.status);
    let status_label = detail_status_label(detail.status);
    let server_name = app.server_name_for(app.app_detail_server);

    row![
        button(
            row![
                icon_arrow_left().size(14),
                Space::with_width(4),
                text("Back").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::text)
        .on_press(Message::BackToDashboard),
        Space::with_width(theme::SPACE_MD),
        column![
            text(format!(
                "APP / {}  \u{b7}  {}",
                detail.app_id.chars().take(8).collect::<String>(),
                server_name
            ))
            .size(TEXT_MONO_LABEL)
            .color(theme::INK_SECONDARY),
            text(&detail.name)
                .size(theme::TEXT_3XL)
                .color(theme::INK_PRIMARY),
        ]
        .spacing(2)
        .width(Length::Fill),
        status_badge(status_label, status_color),
    ]
    .align_y(Alignment::Center)
    .spacing(0)
    .into()
}

fn status_badge<'a>(label: &'a str, color: iced::Color) -> Element<'a, Message> {
    container(text(label).size(TEXT_MICRO).color(color))
        .padding([2, 8])
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(theme::with_alpha(color, 0.08))),
            border: iced::Border {
                color,
                width: 1.0,
                radius: theme::RADIUS_SM.into(),
            },
            ..container::Style::default()
        })
        .into()
}

// ── Info grid ────────────────────────────────────────────────────────────────

fn info_grid(detail: &rover_proto::v1::AppDetailResponse) -> Element<'_, Message> {
    let runtime_name = detail_runtime_name(detail.runtime);
    let pid_str = detail.pid.map_or("-".to_string(), |p| p.to_string());
    let restarts_str = detail.restart_count.to_string();
    let created_str = theme::format_timestamp(detail.created_at.as_ref().map_or(0, |t| t.millis));

    let cells: Vec<Element<Message>> = vec![
        info_cell("Runtime", runtime_name.to_string()),
        info_cell("PID", pid_str),
        info_cell("Restarts", restarts_str),
        info_cell("Created", created_str),
    ];

    container(row(cells).spacing(0))
        .clip(true)
        .style(|_theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill)
        .into()
}

fn info_cell<'a>(label: &'static str, value: String) -> Element<'a, Message> {
    container(
        column![
            text(label).size(TEXT_LABEL).color(theme::INK_SECONDARY),
            Space::with_height(4),
            text(value).size(theme::TEXT_BASE).color(theme::INK_PRIMARY),
        ]
        .spacing(0)
        .padding(theme::SPACE_LG),
    )
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

// ── Commands section ─────────────────────────────────────────────────────────

fn commands_section<'a>(
    _app: &'a RoverApp,
    detail: &'a rover_proto::v1::AppDetailResponse,
) -> Element<'a, Message> {
    container(
        column![
            row![
                text("Commands")
                    .size(TEXT_SECTION)
                    .color(theme::INK_SECONDARY),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon_settings().size(12),
                        Space::with_width(4),
                        text("Update").size(theme::TEXT_SM),
                    ]
                    .align_y(Alignment::Center),
                )
                .style(button::text)
                .on_press(Message::OpenUpdate(detail.app_id.clone())),
            ]
            .align_y(Alignment::Center),
            Space::with_height(theme::SPACE_SM),
            command_row("Build", &detail.build_command),
            command_row("Run", &detail.run_command),
        ]
        .spacing(0)
        .padding(theme::SPACE_LG),
    )
    .style(|_theme| container::Style {
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: theme::RADIUS_MD.into(),
        },
        ..container::Style::default()
    })
    .width(Length::Fill)
    .into()
}

fn command_row<'a>(label: &'static str, command: &'a str) -> Element<'a, Message> {
    row![
        text(label)
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY)
            .width(Length::Fixed(60.0)),
        text(command)
            .size(theme::TEXT_SM)
            .font(iced::Font::MONOSPACE)
            .color(theme::INK_PRIMARY),
    ]
    .spacing(theme::SPACE_SM)
    .padding([4, 0])
    .align_y(Alignment::Center)
    .into()
}

// ── Action buttons ───────────────────────────────────────────────────────────

fn actions_row<'a>(
    app: &'a RoverApp,
    detail: &'a rover_proto::v1::AppDetailResponse,
) -> Element<'a, Message> {
    let app_id = detail.app_id.clone();
    let si = app.app_detail_server;

    row![
        action_button(
            "Start",
            || icon_play().size(13),
            Message::StartApp(app_id.clone(), si),
        ),
        Space::with_width(theme::SPACE_SM),
        action_button(
            "Stop",
            || icon_square().size(13),
            Message::StopApp(app_id.clone(), si),
        ),
        Space::with_width(theme::SPACE_SM),
        action_button(
            "Restart",
            || icon_rotate_cw().size(13),
            Message::RestartApp(app_id.clone(), si),
        ),
        Space::with_width(theme::SPACE_SM),
        action_button_danger("Delete", Message::DeleteApp(app_id.clone(), si)),
    ]
    .spacing(0)
    .into()
}

fn action_button<F>(label: &'static str, icon: F, on_press: Message) -> Element<'static, Message>
where
    F: Fn() -> iced::widget::Text<'static>,
{
    button(
        row![
            icon(),
            Space::with_width(6),
            text(label).size(theme::TEXT_SM),
        ]
        .align_y(Alignment::Center)
        .padding([6, 14]),
    )
    .style(button::secondary)
    .on_press(on_press)
    .into()
}

fn action_button_danger(label: &'static str, on_press: Message) -> Element<'static, Message> {
    button(
        row![
            icon_trash_2().size(13).color(theme::RED),
            Space::with_width(6),
            text(label).size(theme::TEXT_SM).color(theme::RED),
        ]
        .align_y(Alignment::Center)
        .padding([6, 14]),
    )
    .style(button::text)
    .on_press(on_press)
    .into()
}

// ── Logs section ─────────────────────────────────────────────────────────────

fn logs_section(app: &RoverApp) -> Element<'_, Message> {
    let log_lines: Vec<Element<Message>> = app
        .log_entries
        .iter()
        .map(|line| {
            text(line)
                .size(TEXT_MONO)
                .font(iced::Font::MONOSPACE)
                .color(theme::MACHINE_TEXT)
                .shaping(text::Shaping::Advanced)
                .into()
        })
        .collect();

    let log_body = if log_lines.is_empty() {
        container(
            text("No log output")
                .size(theme::TEXT_SM)
                .color(theme::MACHINE_MUTED)
                .font(iced::Font::MONOSPACE),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
    } else {
        container(scrollable(column(log_lines).spacing(2).width(Length::Fill)).height(Length::Fill))
            .width(Length::Fill)
    };

    let log_container = container(log_body.padding(theme::SPACE_MD))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::MACHINE)),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_SM.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill);

    let copy_btn = button(
        row![
            icon_copy().size(12),
            Space::with_width(4),
            text("Copy")
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
        ]
        .align_y(Alignment::Center),
    )
    .style(button::text)
    .on_press(Message::Copy(app.log_entries.join("\n")));

    column![
        row![
            text("Logs").size(TEXT_SECTION).color(theme::INK_SECONDARY),
            Space::with_width(Length::Fill),
            copy_btn,
        ]
        .align_y(Alignment::Center),
        Space::with_height(theme::SPACE_SM),
        log_container.height(240),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

// ── Modals ───────────────────────────────────────────────────────────────────

fn modal_overlay<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(
        container(content)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding([60, 40])
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.4,
        ))),
        ..container::Style::default()
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn update_modal<'a>(app: &RoverApp) -> Element<'a, Message> {
    let app_id = app
        .app_detail
        .as_ref()
        .map_or(String::new(), |d| d.app_id.clone());

    container(
        column![
            text("Update Commands")
                .size(theme::TEXT_XL)
                .color(theme::INK_PRIMARY),
            Space::with_height(theme::SPACE_LG),
            text("Build Command")
                .size(TEXT_LABEL)
                .color(theme::INK_SECONDARY),
            Space::with_height(4),
            text_input("build command", &app.update_build)
                .on_input(Message::SetUpdateBuild)
                .size(theme::TEXT_SM),
            Space::with_height(theme::SPACE_MD),
            text("Run Command")
                .size(TEXT_LABEL)
                .color(theme::INK_SECONDARY),
            Space::with_height(4),
            text_input("run command", &app.update_run)
                .on_input(Message::SetUpdateRun)
                .size(theme::TEXT_SM),
            Space::with_height(theme::SPACE_LG),
            row![
                button(text("Cancel").size(theme::TEXT_SM))
                    .style(button::text)
                    .on_press(Message::CloseUpdate),
                Space::with_width(theme::SPACE_SM),
                button(text("Update Commands").size(theme::TEXT_SM))
                    .style(button::primary)
                    .on_press(Message::ConfirmUpdate(app_id)),
            ]
            .spacing(0),
        ]
        .padding(theme::SPACE_XL)
        .width(Length::Fill),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(theme::PAPER)),
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: theme::RADIUS_LG.into(),
        },
        shadow: theme::shadow_overlay(),
        ..container::Style::default()
    })
    .into()
}

fn delete_modal<'a>(app_id: &str, name: &str, si: usize) -> Element<'a, Message> {
    let aid = app_id.to_string();
    let nm = name.to_string();
    container(
        column![
            row![
                icon_trash_2().size(20).color(theme::RED),
                Space::with_width(theme::SPACE_SM),
                text(format!("Delete {nm}?"))
                    .size(theme::TEXT_LG)
                    .color(theme::INK_PRIMARY),
            ]
            .align_y(Alignment::Center),
            Space::with_height(theme::SPACE_SM),
            text("This action cannot be undone. The application and all its data will be permanently removed.")
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
            Space::with_height(theme::SPACE_LG),
            row![
                button(text("Cancel").size(theme::TEXT_SM))
                    .style(button::text)
                    .on_press(Message::CancelDelete),
                Space::with_width(theme::SPACE_SM),
                button(
                    row![
                        icon_trash_2().size(13),
                        Space::with_width(4),
                        text("Delete").size(theme::TEXT_SM),
                    ]
                    .align_y(Alignment::Center),
                )
                .style(button::danger)
                .on_press(Message::ConfirmDelete(aid, nm, si)),
            ]
            .spacing(0),
        ]
        .padding(theme::SPACE_XL)
        .width(Length::Fill),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(theme::PAPER)),
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: theme::RADIUS_LG.into(),
        },
        shadow: theme::shadow_overlay(),
        ..container::Style::default()
    })
    .into()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn detail_status_color(status: i32) -> iced::Color {
    match status {
        1 | 2 => theme::WARNING,
        3 => theme::SUCCESS,
        4 => theme::INK_SECONDARY,
        5 | 6 => theme::DANGER,
        _ => theme::INK_SECONDARY,
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

const TEXT_MICRO: u16 = 10;
const TEXT_MONO_LABEL: u16 = 10;
const TEXT_MONO: u16 = 11;
const TEXT_LABEL: u16 = 10;
const TEXT_SECTION: u16 = 13;
