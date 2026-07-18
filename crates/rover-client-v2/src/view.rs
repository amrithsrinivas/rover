use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{
    icon_pencil, icon_plug, icon_plus, icon_rocket, icon_terminal, icon_trash_2,
};

use crate::app::{RoverApp, Screen, ServerState};
use crate::message::Message;
use crate::theme;

pub fn view(app: &RoverApp) -> Element<'_, Message> {
    let top_bar = top_bar(app);

    let main_content = if app.servers.is_empty() && !app.show_add_form {
        welcome_screen(app)
    } else if app.servers.is_empty() && app.show_add_form {
        connection_form_centered(app)
    } else {
        match &app.screen {
            Screen::AppDetail(_, _) => app_detail_layout(app),
            Screen::Terminal(_) => crate::screens::terminal::terminal(app),
            Screen::Dashboard | Screen::Connect => dashboard_layout(app),
        }
    };

    let body = column![top_bar, main_content].spacing(0);

    let with_deploy = if app.deploy_open {
        stack([body.into(), crate::screens::deploy::deploy_modal(app)]).into()
    } else {
        body.into()
    };

    let with_manage = if app.show_manage_servers {
        stack([with_deploy, manage_servers_modal(app)]).into()
    } else if app.show_add_form {
        stack([with_deploy, add_server_modal(app)]).into()
    } else {
        with_deploy
    };

    let with_delete = server_delete_overlay(app, with_manage);
    crate::widgets::toast_overlay(app, with_delete)
}

// ── Top bar ──────────────────────────────────────────────────────────────────

fn top_bar(app: &RoverApp) -> Element<'_, Message> {
    let left = row![text("Rover").size(theme::TEXT_LG).color(theme::INK_PRIMARY),]
        .align_y(Alignment::Center);

    let connected = app.connected_count();
    let total = app.server_count();

    let status_text = if total == 0 {
        "No servers".to_string()
    } else {
        format!("{connected}/{total} servers connected")
    };

    let manage_btn: Element<Message> = if total > 0 {
        button(text("Manage Servers").size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::ManageServers)
            .into()
    } else {
        Space::with_width(0).into()
    };

    let right = row![
        text(status_text)
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY),
        Space::with_width(12),
        manage_btn,
        Space::with_width(8),
        button(
            row![
                icon_plus().size(13),
                Space::with_width(4),
                text("Add Server").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::text)
        .on_press(Message::ShowAddForm),
    ]
    .align_y(Alignment::Center)
    .spacing(0);

    container(
        row![left, Space::with_width(Length::Fill), right]
            .align_y(Alignment::Center)
            .padding([8, 20]),
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
    .width(Length::Fill)
    .into()
}

// ── Dashboard layout ─────────────────────────────────────────────────────────

fn dashboard_layout(app: &RoverApp) -> Element<'_, Message> {
    let content = column![
        server_cards(app),
        Space::with_height(theme::SPACE_LG),
        deploy_activity(app),
        Space::with_height(theme::SPACE_LG),
        unified_apps_table(app),
    ]
    .spacing(0)
    .padding(theme::SPACE_XL);

    scrollable(content).height(Length::Fill).into()
}

// ── Server health cards ──────────────────────────────────────────────────────

fn server_cards(app: &RoverApp) -> Element<'_, Message> {
    if app.servers.is_empty() {
        return Space::with_height(0).into();
    }

    // Build cards in rows of 3 (Element doesn't impl Clone, so we build rows directly)
    let card_rows: Vec<Element<Message>> = app
        .servers
        .chunks(3)
        .enumerate()
        .map(|(_row_idx, chunk)| {
            row(chunk
                .iter()
                .enumerate()
                .map(|(i, server)| {
                    let idx = _row_idx * 3 + i;
                    server_card(app, idx, server)
                })
                .collect::<Vec<_>>())
            .spacing(theme::SPACE_MD)
            .into()
        })
        .collect();

    let header = row![
        text("Servers")
            .size(TEXT_SECTION)
            .color(theme::INK_SECONDARY),
        Space::with_width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    column![
        header,
        Space::with_height(theme::SPACE_SM),
        column(card_rows).spacing(theme::SPACE_MD)
    ]
    .spacing(0)
    .into()
}

fn server_card<'a>(
    _app: &'a RoverApp,
    idx: usize,
    server: &'a ServerState,
) -> Element<'a, Message> {
    let status_color = if server.connected {
        theme::SUCCESS
    } else if server.connecting {
        theme::WARNING
    } else if server.error.is_some() {
        theme::DANGER
    } else {
        theme::INK_SECONDARY
    };

    let cpu_str = server
        .metrics
        .as_ref()
        .map_or("--".into(), |m| format!("{:.1}%", m.cpu_percent));
    let ram_str = server
        .metrics
        .as_ref()
        .map_or("--".into(), |m| theme::format_bytes(m.ram_used_bytes));
    let disk_str = server.metrics.as_ref().map_or("--".into(), |m| {
        format!(
            "{} / {}",
            theme::format_bytes(m.disk_used_bytes),
            theme::format_bytes(m.disk_total_bytes)
        )
    });

    let info = server.info.as_ref();

    // Action buttons based on connection state
    let actions: Element<Message> = if server.connected {
        let shell_btn: Element<Message> = button(
            row![
                icon_terminal().size(13),
                Space::with_width(4),
                text("Shell").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::secondary)
        .on_press(Message::OpenTerminal(idx))
        .into();

        let disconnect_btn: Element<Message> = button(text("Disconnect").size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::Disconnect(idx))
            .into();

        row![
            shell_btn,
            Space::with_width(theme::SPACE_SM),
            disconnect_btn,
        ]
        .align_y(Alignment::Center)
        .into()
    } else if server.profile.api_key.is_some() && !server.connecting {
        let connect_btn: Element<Message> = button(text("Connect").size(theme::TEXT_SM))
            .style(button::secondary)
            .on_press(Message::Reconnect(idx))
            .into();
        connect_btn
    } else {
        Space::with_height(0).into()
    };

    container(
        column![
            row![
                text(&server.profile.name)
                    .size(theme::TEXT_BASE)
                    .color(theme::INK_PRIMARY)
                    .width(Length::Fill),
                status_chip(server.status_label(), status_color),
            ]
            .align_y(Alignment::Center),
            container(
                text(if let Some(info) = info {
                    format!("{}  v{}", info.hostname, info.version)
                } else if server.connecting {
                    String::from("Connecting...")
                } else {
                    server.profile.address.clone()
                })
                .size(TEXT_MINI)
                .color(theme::INK_SECONDARY),
            )
            .height(Length::Fixed(18.0)),
            row![
                metric_line("CPU", cpu_str),
                Space::with_width(12),
                metric_line("RAM", ram_str),
                Space::with_width(12),
                metric_line("Disk", disk_str),
                Space::with_width(12),
                metric_line("Apps", server.app_count().to_string()),
            ]
            .align_y(Alignment::Center),
            actions,
        ]
        .spacing(4)
        .padding(theme::SPACE_MD),
    )
    .width(Length::Fill)
    .height(Length::Fixed(138.0))
    .style(|_theme| container::Style {
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: theme::RADIUS_MD.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn status_chip<'a>(label: &'a str, color: iced::Color) -> Element<'a, Message> {
    container(text(label).size(9).color(color))
        .padding([3, 8])
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

fn metric_line<'a>(label: &'static str, value: String) -> Element<'a, Message> {
    row![
        text(label).size(TEXT_MINI).color(theme::INK_SECONDARY),
        Space::with_width(4),
        text(value).size(TEXT_MINI).color(theme::INK_PRIMARY),
    ]
    .align_y(Alignment::Center)
    .into()
}

// ── Deploy activity ──────────────────────────────────────────────────────────

fn deploy_activity(app: &RoverApp) -> Element<'_, Message> {
    if app.deploy_jobs.is_empty() {
        return Space::with_height(0).into();
    }

    let active_count = app.active_deploy_count();
    let header_text = if active_count == 0 {
        "Deployment Activity".to_string()
    } else {
        format!("Deployment Activity — {active_count} active")
    };

    let clear_btn: Element<Message> = if active_count == 0 {
        button(text("Clear finished").size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::ClearFinishedDeploys)
            .into()
    } else {
        Space::with_width(0).into()
    };

    let cards: Vec<Element<Message>> = app
        .deploy_jobs
        .iter()
        .rev()
        .map(|job| deploy_card(app, job))
        .collect();

    container(
        column![
            row![
                text(header_text)
                    .size(TEXT_SECTION)
                    .color(theme::INK_SECONDARY),
                Space::with_width(Length::Fill),
                clear_btn,
            ]
            .align_y(Alignment::Center),
            Space::with_height(theme::SPACE_SM),
            column(cards).spacing(theme::SPACE_SM),
        ]
        .spacing(0),
    )
    .into()
}

fn deploy_card<'a>(app: &'a RoverApp, job: &'a crate::app::DeployJob) -> Element<'a, Message> {
    let status_color = deploy_status_color(&job.status);
    let latest = job.latest_log().unwrap_or("Waiting for output...");
    let expanded = app.expanded_deploy == Some(job.id);
    let toggle_label = if expanded { "Hide log" } else { "View log" };

    let mut actions = row![
        button(text(toggle_label).size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::ToggleDeployLog(job.id)),
        Space::with_width(theme::SPACE_SM),
        button(text("Copy log").size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::Copy(job.logs.join("\n"))),
    ]
    .align_y(Alignment::Center);

    if let Some(app_id) = &job.app_id {
        let aid = app_id.clone();
        let si = job.server_index;
        actions = actions.push(Space::with_width(theme::SPACE_SM)).push(
            button(text("View app").size(theme::TEXT_SM))
                .style(button::text)
                .on_press(Message::SelectApp(aid, si)),
        );
    }

    let mut body = column![
        row![
            column![
                text(&job.name)
                    .size(theme::TEXT_BASE)
                    .color(theme::INK_PRIMARY),
                Space::with_height(2),
                text(format!("{} · {}", job.runtime, job.server_name))
                    .size(theme::TEXT_SM)
                    .color(theme::INK_SECONDARY),
            ]
            .spacing(0)
            .width(Length::Fill),
            status_pill(&job.status, status_color),
        ]
        .align_y(Alignment::Center),
        Space::with_height(theme::SPACE_SM),
        text(latest)
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY),
        Space::with_height(theme::SPACE_SM),
        actions,
    ]
    .spacing(0)
    .padding(theme::SPACE_MD);

    if expanded {
        let log_lines: Vec<Element<Message>> = job
            .logs
            .iter()
            .map(|line| {
                text(line)
                    .size(TEXT_MONO_SM)
                    .font(iced::Font::MONOSPACE)
                    .color(theme::MACHINE_TEXT)
                    .into()
            })
            .collect();
        let log = container(
            scrollable(column(log_lines).spacing(1).width(Length::Fill))
                .height(Length::Fixed(140.0)),
        )
        .padding(10)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::MACHINE)),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_SM.into(),
            },
            ..container::Style::default()
        });
        body = body.push(Space::with_height(theme::SPACE_SM)).push(log);
    }

    container(body)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn status_pill<'a>(label: &'a str, color: iced::Color) -> Element<'a, Message> {
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

// ── Unified apps table ───────────────────────────────────────────────────────

fn unified_apps_table(app: &RoverApp) -> Element<'_, Message> {
    let deploy_btn: Element<Message> = if app.any_connected() {
        button(
            row![
                icon_rocket().size(14),
                Space::with_width(6),
                text("Deploy").size(theme::TEXT_BASE),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::primary)
        .on_press(Message::OpenDeploy)
        .into()
    } else {
        Space::with_width(0).into()
    };

    let header = row![
        text("Applications")
            .size(TEXT_SECTION)
            .color(theme::INK_SECONDARY),
        Space::with_width(Length::Fill),
        deploy_btn,
    ]
    .align_y(Alignment::Center);

    let all_apps = &app.all_apps;

    if all_apps.is_empty() {
        let empty = container(
            column![
                icon_rocket().size(24).color(theme::INK_SECONDARY),
                Space::with_height(theme::SPACE_SM),
                text("No applications deployed yet")
                    .size(theme::TEXT_BASE)
                    .color(theme::INK_SECONDARY),
                Space::with_height(4),
                text("Deploy to a connected server to get started")
                    .size(theme::TEXT_SM)
                    .color(theme::INK_SECONDARY),
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill),
        )
        .center_x(Length::Fill)
        .padding(theme::SPACE_2XL)
        .style(|_theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill);

        return column![header, Space::with_height(theme::SPACE_SM), empty]
            .spacing(0)
            .into();
    }

    let rows: Vec<Element<Message>> = all_apps.iter().map(|a| app_row(a)).collect();

    let table = container(column(rows).spacing(0))
        .clip(true)
        .style(|_theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill);

    column![header, Space::with_height(theme::SPACE_SM), table]
        .spacing(0)
        .into()
}

fn app_row(app: &crate::app::AnnotatedApp) -> Element<'_, Message> {
    let status_color = app_status_color(app.summary.status);
    let status_label = app_status_label(app.summary.status);
    let runtime_name = runtime_name(app.summary.runtime);
    let short_id: String = app.summary.app_id.chars().take(8).collect();

    let row_content = row![
        column![
            text(&app.summary.name)
                .size(theme::TEXT_BASE)
                .color(theme::INK_PRIMARY),
            Space::with_height(2),
            row![
                text(runtime_name)
                    .size(theme::TEXT_SM)
                    .color(theme::INK_SECONDARY),
                text(" · ").size(theme::TEXT_SM).color(theme::INK_SECONDARY),
                text(short_id)
                    .size(theme::TEXT_SM)
                    .font(iced::Font::MONOSPACE)
                    .color(theme::INK_SECONDARY),
                text(" · ").size(theme::TEXT_SM).color(theme::INK_SECONDARY),
                text(&app.server_name)
                    .size(theme::TEXT_SM)
                    .color(theme::INK_SECONDARY),
            ]
            .spacing(2),
        ]
        .spacing(2)
        .width(Length::Fill),
        status_pill(status_label, status_color),
    ]
    .align_y(Alignment::Center)
    .padding(theme::SPACE_MD);

    let app_id = app.summary.app_id.clone();
    let si = app.server_index;
    button(row_content)
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::SelectApp(app_id, si))
        .into()
}

// ── App detail layout ────────────────────────────────────────────────────────

fn app_detail_layout(app: &RoverApp) -> Element<'_, Message> {
    let sidebar = app_list_sidebar(app);
    let detail = crate::screens::app_detail::app_detail(app);
    row![sidebar, detail].spacing(0).into()
}

fn app_list_sidebar(app: &RoverApp) -> Element<'_, Message> {
    let header = row![
        icon_rocket().size(13).color(theme::INK_SECONDARY),
        Space::with_width(6),
        text("Apps")
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY),
        Space::with_width(Length::Fill),
        button(text("← Back").size(theme::TEXT_SM))
            .style(button::text)
            .on_press(Message::BackToDashboard),
    ]
    .align_y(Alignment::Center)
    .padding(12);

    let apps: Vec<Element<Message>> = app
        .all_apps
        .iter()
        .map(|a| {
            let short_id: String = a.summary.app_id.chars().take(8).collect();
            let is_active = match &app.screen {
                Screen::AppDetail(id, _) => id == &a.summary.app_id,
                _ => false,
            };
            let app_id = a.summary.app_id.clone();
            let si = a.server_index;
            let row_content = row![
                text(&a.summary.name)
                    .size(theme::TEXT_SM)
                    .color(theme::INK_PRIMARY)
                    .width(Length::Fill),
                text(short_id)
                    .size(TEXT_MICRO)
                    .font(iced::Font::MONOSPACE)
                    .color(theme::INK_SECONDARY),
            ]
            .align_y(Alignment::Center)
            .padding(8);

            let styled = container(row_content).style(move |_theme| container::Style {
                background: if is_active {
                    Some(iced::Background::Color(theme::with_alpha(
                        theme::BLUE,
                        0.06,
                    )))
                } else {
                    None
                },
                border: iced::Border {
                    color: if is_active {
                        theme::BLUE
                    } else {
                        iced::Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: theme::RADIUS_SM.into(),
                },
                ..container::Style::default()
            });

            button(styled)
                .width(Length::Fill)
                .style(button::text)
                .on_press(Message::SelectApp(app_id, si))
                .into()
        })
        .collect();

    column![
        header,
        scrollable(column(apps).spacing(2)).height(Length::Fill)
    ]
    .width(Length::Fixed(180.0))
    .height(Length::Fill)
    .into()
}

// ── Manage servers modal ─────────────────────────────────────────────────────

fn manage_servers_modal(app: &RoverApp) -> Element<'_, Message> {
    let connected_text = format!(
        "{} of {} server{} connected",
        app.connected_count(),
        app.server_count(),
        if app.server_count() == 1 { "" } else { "s" }
    );

    let server_rows: Vec<Element<Message>> = app
        .servers
        .iter()
        .enumerate()
        .map(|(idx, server)| {
            let status_color = if server.connected {
                theme::SUCCESS
            } else if server.connecting {
                theme::WARNING
            } else {
                theme::INK_SECONDARY
            };

            let name_cell: Element<Message> = if app.editing_server == Some(idx) {
                text_input("name", &app.rename_value)
                    .on_input(Message::SetRenameValue)
                    .on_submit(Message::ConfirmRename(idx))
                    .size(theme::TEXT_SM)
                    .padding(4)
                    .width(Length::Fixed(130.0))
                    .into()
            } else {
                text(&server.profile.name)
                    .size(theme::TEXT_BASE)
                    .color(theme::INK_PRIMARY)
                    .width(Length::Fixed(130.0))
                    .into()
            };

            // Address cell — editable when in edit mode
            let addr_cell: Element<Message> = if app.editing_server == Some(idx) {
                text_input("address", &app.edit_address)
                    .on_input(Message::SetEditAddress)
                    .on_submit(Message::ConfirmRename(idx))
                    .size(theme::TEXT_SM)
                    .padding(4)
                    .font(iced::Font::MONOSPACE)
                    .width(Length::Fixed(200.0))
                    .into()
            } else {
                text(&server.profile.address)
                    .size(theme::TEXT_SM)
                    .font(iced::Font::MONOSPACE)
                    .color(theme::INK_SECONDARY)
                    .into()
            };

            let info_text: Element<Message> = if let Some(info) = &server.info {
                text(format!(
                    "v{}  —  {}  —  uptime {}",
                    info.version,
                    info.os,
                    theme::format_uptime(info.uptime_seconds)
                ))
                .size(TEXT_MINI)
                .color(theme::INK_SECONDARY)
                .into()
            } else {
                Space::with_height(0).into()
            };

            let disconnect_btn: Element<Message> = if server.connected {
                button(text("Disconnect").size(theme::TEXT_SM))
                    .style(button::text)
                    .on_press(Message::Disconnect(idx))
                    .into()
            } else {
                Space::with_width(0).into()
            };

            // Each server as a contained row card
            container(
                column![
                    // Top row: status, name, actions
                    row![
                        status_chip(server.status_label(), status_color),
                        Space::with_width(12),
                        name_cell,
                        Space::with_width(16),
                        column![addr_cell, info_text,].spacing(2),
                        Space::with_width(Length::Fill),
                        if app.editing_server == Some(idx) {
                            let cancel_btn: Element<Message> =
                                button(text("Cancel").size(theme::TEXT_SM))
                                    .style(button::text)
                                    .on_press(Message::CancelRename)
                                    .into();
                            let save_btn: Element<Message> =
                                button(text("Save").size(theme::TEXT_SM))
                                    .style(button::primary)
                                    .on_press(Message::ConfirmRename(idx))
                                    .into();
                            let edit_row: Element<Message> =
                                row![cancel_btn, Space::with_width(4), save_btn,]
                                    .align_y(Alignment::Center)
                                    .into();
                            edit_row
                        } else {
                            row![
                                button(icon_pencil().size(14).color(theme::INK_SECONDARY))
                                    .style(button::text)
                                    .on_press(Message::StartRename(idx)),
                                Space::with_width(4),
                                disconnect_btn,
                                Space::with_width(4),
                                button(icon_trash_2().size(14).color(theme::RED))
                                    .style(button::text)
                                    .on_press(Message::ConfirmServerDelete(idx)),
                            ]
                            .align_y(Alignment::Center)
                            .into()
                        },
                    ]
                    .align_y(Alignment::Center),
                ]
                .padding(theme::SPACE_MD),
            )
            .style(|_theme| container::Style {
                border: iced::Border {
                    color: theme::BORDER,
                    width: 1.0,
                    radius: theme::RADIUS_MD.into(),
                },
                ..container::Style::default()
            })
            .into()
        })
        .collect();

    let body: Element<Message> = if app.servers.is_empty() {
        container(
            text("No servers configured. Add one to get started.")
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
        )
        .center_x(Length::Fill)
        .padding(theme::SPACE_XL)
        .into()
    } else {
        scrollable(column(server_rows).spacing(theme::SPACE_SM))
            .height(Length::Fixed(340.0))
            .into()
    };

    let modal = container(
        column![
            // Header
            text("Servers")
                .size(theme::TEXT_XL)
                .color(theme::INK_PRIMARY),
            Space::with_height(4),
            text(connected_text)
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
            Space::with_height(theme::SPACE_LG),
            // Server list
            body,
            Space::with_height(theme::SPACE_LG),
            // Footer
            row![
                button(
                    row![
                        icon_plus().size(14),
                        Space::with_width(6),
                        text("Add Server").size(theme::TEXT_SM),
                    ]
                    .align_y(Alignment::Center),
                )
                .style(button::secondary)
                .on_press(Message::ShowAddForm),
                Space::with_width(Length::Fill),
                button(text("Done").size(theme::TEXT_SM))
                    .style(button::primary)
                    .on_press(Message::CloseManageServers),
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
    });

    modal_overlay(modal.into())
}

fn add_server_modal(app: &RoverApp) -> Element<'_, Message> {
    let addr_input = text_input("192.168.1.42:9050", &app.addr_input)
        .on_input(Message::SetAddr)
        .size(theme::TEXT_SM)
        .width(Length::Fixed(320.0));

    let token_input = text_input("Pairing token", &app.token_input)
        .on_input(Message::SetToken)
        .size(theme::TEXT_SM)
        .width(Length::Fixed(320.0));

    let name_input = text_input("Server name", &app.name_input)
        .on_input(Message::SetServerName)
        .size(theme::TEXT_SM)
        .width(Length::Fixed(320.0));

    let mut form = column![
        text("Add a Server")
            .size(theme::TEXT_XL)
            .color(theme::INK_PRIMARY),
        Space::with_height(theme::SPACE_SM),
        text("Enter the server address and the pairing token shown in the server's terminal.")
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY),
        Space::with_height(theme::SPACE_LG),
        text("Address").size(TEXT_LABEL).color(theme::INK_SECONDARY),
        Space::with_height(4),
        addr_input,
        Space::with_height(theme::SPACE_MD),
        text("Pairing Token")
            .size(TEXT_LABEL)
            .color(theme::INK_SECONDARY),
        Space::with_height(4),
        token_input,
        Space::with_height(theme::SPACE_MD),
        text("Name (optional)")
            .size(TEXT_LABEL)
            .color(theme::INK_SECONDARY),
        Space::with_height(4),
        name_input,
    ]
    .spacing(0);

    if let Some(err) = &app.form_error {
        form = form.push(Space::with_height(theme::SPACE_SM));
        form = form.push(text(err).size(theme::TEXT_SM).color(theme::RED));
    }

    form = form.push(Space::with_height(theme::SPACE_LG));
    form = form.push(
        row![
            button(text("Cancel").size(theme::TEXT_SM))
                .style(button::text)
                .on_press(Message::HideAddForm),
            Space::with_width(theme::SPACE_SM),
            button(text("Connect").size(theme::TEXT_SM))
                .style(button::primary)
                .on_press(Message::Connect),
        ]
        .spacing(0),
    );

    let modal = container(form.padding(theme::SPACE_XL).width(Length::Fill)).style(|_theme| {
        container::Style {
            background: Some(iced::Background::Color(theme::PAPER)),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_LG.into(),
            },
            shadow: theme::shadow_overlay(),
            ..container::Style::default()
        }
    });

    modal_overlay(modal.into())
}

fn modal_overlay<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(
        container(content)
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

// ── Welcome screen ───────────────────────────────────────────────────────────

fn welcome_screen(_app: &RoverApp) -> Element<'_, Message> {
    container(
        column![
            text("Rover")
                .size(theme::TEXT_3XL)
                .color(theme::INK_PRIMARY),
            Space::with_height(theme::SPACE_SM),
            text("Deploy and manage applications on your own infrastructure.")
                .size(theme::TEXT_BASE)
                .color(theme::INK_SECONDARY),
            Space::with_height(theme::SPACE_LG),
            button(
                row![
                    icon_plug().size(14),
                    Space::with_width(6),
                    text("Connect to a Server").size(theme::TEXT_BASE),
                ]
                .align_y(Alignment::Center),
            )
            .style(button::primary)
            .on_press(Message::ShowAddForm),
        ]
        .align_x(Alignment::Center)
        .padding(theme::SPACE_2XL),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn connection_form_centered(app: &RoverApp) -> Element<'_, Message> {
    add_server_modal(app)
}

// ── Server delete overlay ────────────────────────────────────────────────────

fn server_delete_overlay<'a>(
    app: &'a RoverApp,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let Some(idx) = app.confirm_server_delete else {
        return body;
    };
    let Some(server) = app.servers.get(idx) else {
        return body;
    };

    let name = server.profile.name.clone();
    let modal = container(
        column![
            text(format!("Remove {name}?"))
                .size(theme::TEXT_LG)
                .color(theme::INK_PRIMARY),
            Space::with_height(theme::SPACE_SM),
            text("The saved API key will be deleted. You can re-pair later.")
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
            Space::with_height(theme::SPACE_LG),
            row![
                button(text("Cancel").size(theme::TEXT_SM))
                    .style(button::text)
                    .on_press(Message::CancelServerDelete),
                Space::with_width(theme::SPACE_SM),
                button(text("Remove").size(theme::TEXT_SM))
                    .style(button::danger)
                    .on_press(Message::DeleteServer(idx)),
            ]
            .spacing(0),
        ]
        .padding(theme::SPACE_XL),
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
    });

    stack([
        body,
        container(
            container(modal)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.4,
            ))),
            ..container::Style::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    ])
    .into()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn deploy_status_color(status: &str) -> iced::Color {
    match status {
        "complete" => theme::SUCCESS,
        "failed" => theme::DANGER,
        "packaging" | "sending" | "building" | "starting" => theme::BLUE,
        _ => theme::WARNING,
    }
}

fn app_status_color(status: i32) -> iced::Color {
    match status {
        1 | 2 => theme::WARNING,
        3 => theme::SUCCESS,
        4 => theme::INK_SECONDARY,
        5 | 6 => theme::DANGER,
        _ => theme::INK_SECONDARY,
    }
}

fn app_status_label(status: i32) -> &'static str {
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
        1 => "Python",
        2 => "Node.js",
        3 => "Go",
        4 => "Rust",
        _ => "Unknown",
    }
}

const TEXT_MICRO: u16 = 10;
const TEXT_MONO_SM: u16 = 11;
const TEXT_MINI: u16 = 10;
const TEXT_LABEL: u16 = 10;
const TEXT_SECTION: u16 = 13;
