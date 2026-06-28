use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{Alignment, Element, Length};

use crate::app::{RoverApp, ToastKind};
use crate::message::Message;
use crate::{theme, widgets};

/// Render the root client UI.
pub fn view(app: &RoverApp) -> Element<'_, Message> {
    let sidebar = widgets::sidebar::sidebar(app);

    let content = if app.devices.is_empty() && !app.show_add {
        container(
            text("Add a device to get started")
                .size(16)
                .color(theme::colors::TEXT_MUTED),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(40)
        .into()
    } else if let Some(device) = app.devices.get(app.active) {
        if device.connecting {
            centered_message("Connecting...")
        } else if let Some(err) = &device.err {
            connection_error(app, err)
        } else if !device.connected {
            centered_message("Select a device and connect")
        } else if app.selected_app.is_some() {
            widgets::detail::app_detail(app)
        } else {
            widgets::dashboard::dashboard(app)
        }
    } else {
        centered_message("Select a device and connect")
    };

    let main_area = column![status_bar(app), content].spacing(0);
    let body = row![sidebar, main_area].spacing(0);

    let body_with_modal = if app.deploy_open {
        stack([body.into(), widgets::deploy::deploy_modal(app)]).into()
    } else {
        body.into()
    };

    let body_with_delete = device_delete_overlay(app, body_with_modal);
    toast_overlay(app, body_with_delete)
}

fn centered_message(message: &str) -> Element<'_, Message> {
    container(text(message).size(16).color(theme::colors::TEXT_MUTED))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn connection_error<'a>(app: &'a RoverApp, err: &'a str) -> Element<'a, Message> {
    let retry_btn = button(text("Retry").size(14))
        .style(button::primary)
        .on_press(Message::Select(app.active));

    container(
        column![
            text("Connection failed")
                .size(16)
                .color(theme::colors::DANGER),
            Space::with_height(8),
            text(err).size(13).color(theme::colors::TEXT_MUTED),
            Space::with_height(12),
            retry_btn,
        ]
        .align_x(Alignment::Center)
        .spacing(0),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn device_delete_overlay<'a>(
    app: &'a RoverApp,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let Some(idx) = app.confirm_device_delete else {
        return body;
    };
    let Some(device) = app.devices.get(idx) else {
        return body;
    };

    let name = device.profile.name.clone();
    let modal = container(
        column![
            text(format!("Remove {name}?"))
                .size(16)
                .color(theme::colors::TEXT),
            Space::with_height(8),
            text("Saved API key will be deleted.")
                .size(13)
                .color(theme::colors::TEXT_MUTED),
            Space::with_height(16),
            row![
                button(text("Cancel").size(13))
                    .style(button::secondary)
                    .on_press(Message::CancelDeleteDevice),
                Space::with_width(8),
                button(text("Remove").size(13))
                    .style(button::danger)
                    .on_press(Message::ConfirmDeleteDevice(idx)),
            ]
            .spacing(0),
        ]
        .padding(24),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(theme::colors::ELEVATED)),
        border: iced::Border {
            color: theme::colors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    });

    stack([
        body,
        container(modal.center_x(Length::Fill).center_y(Length::Fill))
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
}

fn toast_overlay<'a>(app: &'a RoverApp, body: Element<'a, Message>) -> Element<'a, Message> {
    if app.toasts.is_empty() {
        return body;
    }

    let toasts: Vec<Element<Message>> = app
        .toasts
        .iter()
        .enumerate()
        .map(|(i, toast)| {
            let color = match toast.kind {
                ToastKind::Info => theme::colors::ACCENT,
                ToastKind::Error => theme::colors::DANGER,
            };
            button(
                container(
                    row![
                        text(&toast.message).size(12).color(theme::colors::TEXT),
                        Space::with_width(8),
                        text("✕").size(11).color(theme::colors::TEXT_MUTED),
                    ]
                    .align_y(Alignment::Center)
                    .padding(10),
                )
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: iced::Border {
                        color,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..container::Style::default()
                }),
            )
            .style(button::text)
            .on_press(Message::Dismiss(i))
            .into()
        })
        .collect();

    stack([
        body,
        container(column(toasts).spacing(6).padding(12))
            .width(Length::Fill)
            .into(),
    ])
    .into()
}

fn status_bar(app: &RoverApp) -> Element<'_, Message> {
    let mut status_text = if let Some(device) = app.devices.get(app.active) {
        if device.connected {
            format!("● {} — {}", device.profile.name, device.profile.address)
        } else if device.connecting {
            format!("◌ Connecting to {}...", device.profile.name)
        } else if device.err.is_some() {
            format!("✕ {}", device.profile.name)
        } else {
            format!("○ {} — disconnected", device.profile.name)
        }
    } else if app.devices.is_empty() {
        "No devices".into()
    } else {
        "Select a device".into()
    };

    let active_deploy_count = app.active_deploy_count();
    if active_deploy_count > 0 {
        status_text.push_str(&format!(" — {active_deploy_count} deploy running"));
    }

    container(text(status_text).size(11).color(theme::colors::TEXT_MUTED))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::colors::SURFACE)),
            border: iced::Border {
                color: theme::colors::BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .padding(8)
        .width(Length::Fill)
        .into()
}
