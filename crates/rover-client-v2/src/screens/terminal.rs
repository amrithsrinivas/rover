/// System shell terminal — provides a live shell session to the connected server.
///
/// Output is rendered as clickable monospace lines — click any line to copy it.
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{icon_copy, icon_loader, icon_terminal, icon_x};

use crate::app::RoverApp;
use crate::message::Message;
use crate::theme;

/// Render the terminal view.
pub fn terminal(app: &RoverApp) -> Element<'_, Message> {
    let server_name = app.server_name_for(app.terminal_server);

    let header = row![
        icon_terminal().size(16).color(theme::BLUE),
        Space::with_width(8),
        text(format!("Shell — {server_name}"))
            .size(theme::TEXT_LG)
            .color(theme::INK_PRIMARY),
        Space::with_width(Length::Fill),
        button(
            row![
                icon_copy().size(12),
                Space::with_width(4),
                text("Copy All")
                    .size(theme::TEXT_SM)
                    .color(theme::INK_SECONDARY),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::text)
        .on_press(Message::Copy(app.terminal_output.join("\n"))),
        Space::with_width(8),
        button(
            row![
                icon_x().size(14),
                Space::with_width(4),
                text("Close").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::text)
        .on_press(Message::CloseTerminal),
    ]
    .align_y(Alignment::Center);

    // Terminal output area — each line clickable to copy
    let output_lines: Vec<Element<Message>> = app
        .terminal_output
        .iter()
        .map(|line| {
            let line_owned = line.clone();
            button(
                text(line)
                    .size(TEXT_MONO)
                    .font(iced::Font::MONOSPACE)
                    .color(theme::MACHINE_TEXT)
                    .shaping(text::Shaping::Advanced),
            )
            .style(button::text)
            .on_press(Message::Copy(line_owned))
            .into()
        })
        .collect();

    let terminal_body = if output_lines.is_empty() {
        container(
            text("Shell connected — type a command and press Enter")
                .size(theme::TEXT_SM)
                .color(theme::MACHINE_MUTED)
                .font(iced::Font::MONOSPACE),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
    } else {
        container(
            scrollable(column(output_lines).spacing(1).width(Length::Fill)).height(Length::Fill),
        )
        .width(Length::Fill)
    };

    let output_container = container(terminal_body.padding(theme::SPACE_MD))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(theme::MACHINE)),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: theme::RADIUS_SM.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill)
        .height(Length::Fill);

    // Command input
    let input_row = text_input("Type a command and press Enter...", &app.terminal_input)
        .on_input(Message::SetTerminalInput)
        .on_submit(Message::SubmitShellCommand)
        .size(theme::TEXT_SM)
        .font(iced::Font::MONOSPACE)
        .width(Length::Fill);

    // Pending status
    let status_line: Element<Message> = if app.terminal_pending {
        row![
            icon_loader().size(12).color(theme::WARNING),
            Space::with_width(6),
            text(format!("Running: {}", app.terminal_last_cmd))
                .size(TEXT_STATUS)
                .font(iced::Font::MONOSPACE)
                .color(theme::WARNING),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        container(
            text("Ready")
                .size(TEXT_STATUS)
                .color(theme::with_alpha(theme::INK_SECONDARY, 0.4)),
        )
        .height(Length::Fixed(18.0))
        .into()
    };

    let body = column![
        header,
        Space::with_height(theme::SPACE_MD),
        output_container,
        Space::with_height(theme::SPACE_SM),
        input_row,
        Space::with_height(4),
        status_line,
    ]
    .spacing(0)
    .padding(theme::SPACE_XL)
    .width(Length::Fill)
    .height(Length::Fill);

    body.into()
}

const TEXT_MONO: u16 = 12;
const TEXT_STATUS: u16 = 10;
