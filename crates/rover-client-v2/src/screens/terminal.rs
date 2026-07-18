/// System shell terminal with ANSI color support and interactive prompt.
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{icon_copy, icon_loader, icon_terminal, icon_x};

use crate::app::RoverApp;
use crate::message::Message;
use crate::screens::ansi;
use crate::theme;

pub fn terminal(app: &RoverApp) -> Element<'_, Message> {
    let server_name = app.server_name_for(app.terminal_server);

    let header = row![
        icon_terminal().size(16).color(theme::BLUE),
        Space::with_width(8),
        column![
            text(format!("Shell — {server_name}"))
                .size(theme::TEXT_LG)
                .color(theme::INK_PRIMARY),
            text("bash · ANSI colors enabled")
                .size(TEXT_MINI)
                .color(theme::INK_SECONDARY),
        ]
        .spacing(2)
        .width(Length::Fill),
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

    // Terminal output with ANSI color parsing
    let output_lines: Vec<Element<Message>> = app
        .terminal_output
        .iter()
        .flat_map(|line| render_ansi_line(line))
        .collect();

    let terminal_body: Element<Message> = if output_lines.is_empty() {
        container(
            text("Shell connected — type a command and press Enter")
                .size(theme::TEXT_SM)
                .color(theme::MACHINE_MUTED)
                .font(iced::Font::MONOSPACE),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .into()
    } else {
        scrollable(column(output_lines).spacing(1).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    let output_container = container(terminal_body)
        .padding(theme::SPACE_MD)
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

    let input_row = text_input("Type a command and press Enter...", &app.terminal_input)
        .on_input(Message::SetTerminalInput)
        .on_submit(Message::SubmitShellCommand)
        .size(theme::TEXT_SM)
        .font(iced::Font::MONOSPACE)
        .width(Length::Fill);

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
        container(text("").size(TEXT_STATUS))
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

/// Render a single line of output, parsing ANSI codes into styled text.
fn render_ansi_line(line: &str) -> Vec<Element<Message>> {
    let spans = ansi::parse_ansi_line(line);

    if spans.len() == 1 {
        let span = spans.into_iter().next().unwrap();
        let fg = ansi::span_fg_color(span.fg, span.bold);
        let bg = ansi::span_bg_color(span.bg);
        let line_owned = span.text.clone();

        let content = text(span.text)
            .size(TEXT_MONO)
            .font(iced::Font::MONOSPACE)
            .color(fg)
            .shaping(text::Shaping::Advanced);

        let styled: Element<Message> = if bg != iced::Color::TRANSPARENT {
            container(content)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..container::Style::default()
                })
                .into()
        } else {
            content.into()
        };

        vec![
            button(styled)
                .style(button::text)
                .on_press(Message::Copy(line_owned))
                .into(),
        ]
    } else {
        // Multiple spans — render as a row of colored text segments
        let line_owned = spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        let segments: Vec<Element<Message>> = spans
            .into_iter()
            .map(|span| {
                let fg = ansi::span_fg_color(span.fg, span.bold);
                let bg = ansi::span_bg_color(span.bg);
                let content = text(span.text)
                    .size(TEXT_MONO)
                    .font(iced::Font::MONOSPACE)
                    .color(fg)
                    .shaping(text::Shaping::Advanced);
                if bg != iced::Color::TRANSPARENT {
                    container(content)
                        .style(move |_theme| container::Style {
                            background: Some(iced::Background::Color(bg)),
                            ..container::Style::default()
                        })
                        .into()
                } else {
                    content.into()
                }
            })
            .collect();

        vec![
            button(row(segments).spacing(0))
                .style(button::text)
                .on_press(Message::Copy(line_owned))
                .into(),
        ]
    }
}

const TEXT_MONO: u16 = 12;
const TEXT_MINI: u16 = 10;
const TEXT_STATUS: u16 = 10;
