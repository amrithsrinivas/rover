use iced::widget::{
    Space, button, column, container, horizontal_rule, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{icon_pencil, icon_plug, icon_plus, icon_trash_2};

use crate::app::{RoverApp, ServerState};
use crate::message::Message;
use crate::theme;

/// Render the sidebar — server list + connection form.
pub fn sidebar(app: &RoverApp) -> Element<'_, Message> {
    let header = row![
        icon_plug().size(14).color(theme::INK_SECONDARY),
        Space::with_width(6),
        text("Servers")
            .size(theme::TEXT_SM)
            .color(theme::INK_SECONDARY),
    ]
    .align_y(Alignment::Center)
    .padding(12);

    let server_list = scrollable(
        column(
            app.servers
                .iter()
                .enumerate()
                .map(|(i, s)| server_row(app, i, s))
                .collect::<Vec<_>>(),
        )
        .spacing(2),
    )
    .height(Length::Fill);

    let bottom_section: Element<'_, Message> = if app.show_add_form {
        column![
            horizontal_rule(1).style(separator_style),
            connection_form(app),
        ]
        .spacing(theme::SPACE_SM)
        .padding(theme::SPACE_MD)
        .into()
    } else {
        let add_btn = button(
            row![
                icon_plus().size(14),
                Space::with_width(6),
                text("Connect a server").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::ShowAddForm);
        container(add_btn).padding(theme::SPACE_MD).into()
    };

    column![header, server_list, bottom_section]
        .width(Length::Fixed(theme::SIDEBAR_WIDTH))
        .height(Length::Fill)
        .into()
}

fn server_row<'a>(app: &'a RoverApp, idx: usize, server: &'a ServerState) -> Element<'a, Message> {
    let dot_color = if server.connected {
        theme::SUCCESS
    } else if server.connecting {
        theme::WARNING
    } else {
        theme::INK_SECONDARY
    };

    let dot = icon_plug().size(12).color(dot_color);

    let edit_btn = button(icon_pencil().size(12))
        .style(button::text)
        .on_press(Message::StartRename(idx));

    let label: Element<Message> = if app.editing_server == Some(idx) {
        text_input("name", &app.rename_value)
            .on_input(Message::SetRenameValue)
            .on_submit(Message::ConfirmRename(idx))
            .size(theme::TEXT_SM)
            .padding(4)
            .into()
    } else {
        column![
            text(&server.profile.name)
                .size(theme::TEXT_SM)
                .color(theme::INK_PRIMARY),
            text(&server.profile.address)
                .size(TEXT_MICRO)
                .color(theme::INK_SECONDARY),
        ]
        .spacing(2)
        .into()
    };

    let delete_btn = button(icon_trash_2().size(12).color(theme::RED))
        .style(button::text)
        .on_press(Message::ConfirmServerDelete(idx));

    let is_active = idx == app.active_server;

    let row_content = row![
        dot,
        label,
        Space::with_width(Length::Fill),
        edit_btn,
        delete_btn
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding(10);

    let styled = container(row_content).style(move |_theme| container::Style {
        background: if is_active {
            Some(iced::Background::Color(theme::with_alpha(
                theme::BLUE,
                0.06,
            )))
        } else {
            None
        },
        border: if is_active {
            iced::Border {
                color: theme::BLUE,
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            }
        } else {
            iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: theme::RADIUS_MD.into(),
            }
        },
        ..container::Style::default()
    });

    button(styled)
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::SelectServer(idx))
        .into()
}

fn connection_form(app: &RoverApp) -> Element<'_, Message> {
    let addr_input = text_input("192.168.1.42:9050", &app.addr_input)
        .on_input(Message::SetAddr)
        .size(TEXT_FIELD);

    let token_input = text_input("Pairing token", &app.token_input)
        .on_input(Message::SetToken)
        .size(TEXT_FIELD);

    let name_input = text_input("Server name", &app.name_input)
        .on_input(Message::SetServerName)
        .size(TEXT_FIELD);

    let mut cols = column![
        text("Address").size(TEXT_LABEL).color(theme::INK_SECONDARY),
        addr_input,
        Space::with_height(6),
        text("Pairing Token")
            .size(TEXT_LABEL)
            .color(theme::INK_SECONDARY),
        token_input,
        Space::with_height(6),
        text("Name").size(TEXT_LABEL).color(theme::INK_SECONDARY),
        name_input,
    ]
    .spacing(2);

    if let Some(err) = &app.form_error {
        cols = cols.push(Space::with_height(4));
        cols = cols.push(text(err).size(TEXT_MICRO).color(theme::RED));
    }

    let connect_btn = button(
        row![
            icon_plug().size(13),
            Space::with_width(6),
            text("Connect").size(theme::TEXT_SM),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(button::primary)
    .on_press(Message::Connect);

    let cancel_btn = button(text("Cancel").size(theme::TEXT_SM))
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::HideAddForm);

    cols = cols.push(Space::with_height(6));
    cols = cols.push(connect_btn);
    cols = cols.push(Space::with_height(2));
    cols = cols.push(cancel_btn);

    cols.into()
}

fn separator_style(_theme: &iced::Theme) -> iced::widget::rule::Style {
    iced::widget::rule::Style {
        color: theme::BORDER,
        width: 1,
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
    }
}

const TEXT_MICRO: u16 = 10;
const TEXT_LABEL: u16 = 10;
const TEXT_FIELD: u16 = 13;
