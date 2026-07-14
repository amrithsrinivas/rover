use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{Alignment, Element, Length};

use crate::app::{RoverApp, ToastKind};
use crate::message::Message;
use crate::theme;

/// Overlay toast notifications at the top-right of the screen.
pub fn toast_overlay<'a>(app: &'a RoverApp, body: Element<'a, Message>) -> Element<'a, Message> {
    if app.toasts.is_empty() {
        return body;
    }

    let toasts: Vec<Element<Message>> = app
        .toasts
        .iter()
        .enumerate()
        .map(|(i, toast)| {
            let (bg, border_color) = match toast.kind {
                ToastKind::Info => (theme::with_alpha(theme::BLUE, 0.08), theme::BLUE),
                ToastKind::Error => (theme::with_alpha(theme::RED, 0.08), theme::RED),
            };

            button(
                container(
                    row![
                        text(&toast.message)
                            .size(theme::TEXT_SM)
                            .color(theme::INK_PRIMARY),
                        Space::with_width(12),
                        text("✕").size(TEXT_MICRO).color(theme::INK_SECONDARY),
                    ]
                    .align_y(Alignment::Center)
                    .padding([8, 12]),
                )
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: theme::RADIUS_SM.into(),
                    },
                    ..container::Style::default()
                }),
            )
            .style(button::text)
            .on_press(Message::DismissToast(i))
            .into()
        })
        .collect();

    stack([
        body,
        container(
            column(toasts)
                .spacing(6)
                .padding(12)
                .width(Length::Fill)
                .align_x(Alignment::End),
        )
        .width(Length::Fill)
        .into(),
    ])
    .into()
}

const TEXT_MICRO: u16 = 10;
