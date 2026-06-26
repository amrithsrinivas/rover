use iced::widget::{
    Space, button, column, container, horizontal_rule, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::RoverApp;
use crate::message::Message;
use crate::state::DeviceState;
use crate::theme::colors;

/// Render the sidebar — device list + connection form.
pub fn sidebar(app: &RoverApp) -> Element<'_, Message> {
    let header = container(
        text("Devices")
            .size(14)
            .color(colors::TEXT_MUTED)
            .font(iced::Font::MONOSPACE),
    )
    .padding(12);

    let device_list = scrollable(
        column(
            app.devices
                .iter()
                .enumerate()
                .map(|(i, d)| device_row(app, i, d))
                .collect::<Vec<_>>(),
        )
        .spacing(4),
    )
    .height(Length::Fill);

    let add_button: Element<'_, Message> = if app.show_add {
        column![
            horizontal_rule(1).style(separator_style),
            connection_form(app),
        ]
        .spacing(8)
        .padding(12)
        .into()
    } else {
        button(text("+ Connect a device").size(13))
            .width(Length::Fill)
            .style(button::secondary)
            .on_press(Message::ShowAdd)
            .into()
    };

    column![header, device_list, add_button]
        .width(Length::Fixed(220.0))
        .height(Length::Fill)
        .into()
}

fn device_row<'a>(app: &'a RoverApp, i: usize, d: &'a DeviceState) -> Element<'a, Message> {
    let dot = if d.connected {
        text("●").color(colors::SUCCESS).size(11)
    } else if d.connecting {
        text("◌").color(colors::WARNING).size(11)
    } else {
        text("○").color(colors::TEXT_MUTED).size(11)
    };

    let label = column![
        text(&d.profile.name).size(13).color(colors::TEXT),
        text(&d.profile.address).size(10).color(colors::TEXT_MUTED),
    ]
    .spacing(2);

    let row_content = row![dot, label]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding(10);

    let is_active = i == app.active;

    let styled = container(row_content).style(move |_theme| container::Style {
        border: if is_active {
            iced::Border {
                color: colors::ACCENT,
                width: 1.0,
                radius: 6.0.into(),
            }
        } else {
            iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            }
        },
        ..container::Style::default()
    });

    button(styled)
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::Select(i))
        .into()
}

fn connection_form(app: &RoverApp) -> Element<'_, Message> {
    let addr_input = text_input("192.168.1.42:9050", &app.addr)
        .on_input(Message::SetAddr)
        .size(13);

    let token_input = text_input("Pairing token", &app.token)
        .on_input(Message::SetToken)
        .size(13);

    let name_input = text_input("Device name", &app.name)
        .on_input(Message::SetName)
        .size(13);

    let mut cols = column![
        text("Address").size(10).color(colors::TEXT_MUTED),
        addr_input,
        Space::with_height(8),
        text("Pairing Token").size(10).color(colors::TEXT_MUTED),
        token_input,
        Space::with_height(8),
        text("Name").size(10).color(colors::TEXT_MUTED),
        name_input,
    ]
    .spacing(4)
    .padding(0);

    if let Some(err) = &app.error {
        cols = cols.push(Space::with_height(4));
        cols = cols.push(text(err).size(11).color(colors::DANGER));
    }

    let connect_btn = button(text("Connect").size(13))
        .width(Length::Fill)
        .style(button::primary)
        .on_press(Message::Connect);

    let cancel_btn = button(text("Cancel").size(13))
        .width(Length::Fill)
        .style(button::secondary)
        .on_press(Message::HideAdd);

    cols = cols.push(Space::with_height(8));
    cols = cols.push(connect_btn);
    cols = cols.push(Space::with_height(4));
    cols = cols.push(cancel_btn);

    cols.into()
}

fn separator_style(_theme: &iced::Theme) -> iced::widget::rule::Style {
    iced::widget::rule::Style {
        color: colors::BORDER,
        width: 1,
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
    }
}
