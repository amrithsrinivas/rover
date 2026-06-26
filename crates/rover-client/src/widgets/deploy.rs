use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::RoverApp;
use crate::message::Message;
use crate::theme::colors;

const RUNTIMES: &[&str] = &["python", "node", "go", "rust"];

/// Render the deploy modal overlay.
pub fn deploy_modal(app: &RoverApp) -> Element<'_, Message> {
    let modal_content = container(deploy_form(app)).style(|_theme| container::Style {
        background: Some(iced::Background::Color(colors::ELEVATED)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    });

    container(
        container(modal_content.width(Length::Fixed(580.0)))
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
    .into()
}

fn deploy_form(app: &RoverApp) -> Element<'_, Message> {
    let title_row = row![
        text("Deploy Application")
            .size(18)
            .color(colors::TEXT)
            .width(Length::Fill),
        button(text("✕").size(16).color(colors::TEXT_MUTED))
            .style(button::text)
            .on_press(Message::CloseDeploy),
    ]
    .align_y(Alignment::Center);

    let name_input = text_input("my-app", &app.deploy_name)
        .on_input(Message::SetDName)
        .size(13)
        .width(Length::Fill);

    let runtime_picker = pick_list(
        RUNTIMES,
        if app.deploy_runtime.is_empty() {
            None
        } else {
            Some(app.deploy_runtime.as_str())
        },
        |s| Message::SetDRuntime(s.to_string()),
    )
    .placeholder("Select runtime...");

    let build_input = text_input("Build command", &app.deploy_build)
        .on_input(Message::SetDBuild)
        .size(13)
        .width(Length::Fill);

    let run_input = text_input("Run command", &app.deploy_run)
        .on_input(Message::SetDRun)
        .size(13)
        .width(Length::Fill);

    let path_row = row![
        text_input("Source directory path", &app.deploy_path)
            .on_input(Message::SetDPath)
            .size(13)
            .width(Length::Fill),
        button(text("Browse").size(12))
            .style(button::secondary)
            .on_press(Message::PickPath),
    ]
    .spacing(8);

    let env_section = deploy_env_section(app);

    let deploy_btn = if app.deploying {
        button(text("Deploying...").size(14))
            .width(Length::Fill)
            .style(button::primary)
    } else if app.deploy_path.is_empty() {
        button(text("Select a source directory").size(14))
            .width(Length::Fill)
            .style(button::primary)
    } else {
        button(text("Deploy").size(14))
            .width(Length::Fill)
            .style(button::primary)
            .on_press(Message::SubmitDeploy)
    };

    let log_section = build_log_section(app);

    let mut form = column![
        title_row,
        Space::with_height(16),
        text("App Name").size(10).color(colors::TEXT_MUTED),
        name_input,
        Space::with_height(10),
        text("Runtime").size(10).color(colors::TEXT_MUTED),
        runtime_picker,
        Space::with_height(10),
        text("Build Command").size(10).color(colors::TEXT_MUTED),
        build_input,
        Space::with_height(10),
        text("Run Command").size(10).color(colors::TEXT_MUTED),
        run_input,
        Space::with_height(10),
        text("Source Directory").size(10).color(colors::TEXT_MUTED),
        path_row,
        Space::with_height(16),
        env_section,
        Space::with_height(16),
        deploy_btn,
    ]
    .spacing(0)
    .padding(24);

    if !app.deploy_log.is_empty() {
        form = form.push(Space::with_height(12));
        form = form.push(log_section);
    }

    scrollable(form).height(Length::Fill).into()
}

fn deploy_env_section(app: &RoverApp) -> Element<'_, Message> {
    let env_rows: Vec<Element<Message>> = app
        .deploy_env_vars
        .iter()
        .enumerate()
        .map(|(i, (k, v))| {
            container(
                row![
                    text(format!("{k}={v}")).size(12).color(colors::TEXT),
                    Space::with_width(8),
                    button(text("✕").size(11).color(colors::DANGER))
                        .style(button::text)
                        .on_press(Message::RemoveDEVar(i)),
                ]
                .align_y(Alignment::Center),
            )
            .padding([2, 0])
            .into()
        })
        .collect();

    let add_row = row![
        text_input("KEY", &app.deploy_env_key)
            .on_input(Message::SetDEKey)
            .size(12)
            .width(Length::Fixed(140.0)),
        text_input("VALUE", &app.deploy_env_value)
            .on_input(Message::SetDEValue)
            .size(12)
            .width(Length::Fixed(200.0)),
        button(text("+ Add").size(12))
            .style(button::primary)
            .on_press(Message::AddDEVar),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut col = column![
        text("Environment Variables")
            .size(10)
            .color(colors::TEXT_MUTED),
        Space::with_height(4),
        add_row,
    ]
    .spacing(0);

    if env_rows.is_empty() {
        col = col.push(Space::with_height(4));
        col = col.push(text("None").size(11).color(colors::TEXT_MUTED));
    } else {
        for row in env_rows {
            col = col.push(Space::with_height(2));
            col = col.push(row);
        }
    }

    col.into()
}

fn build_log_section(app: &RoverApp) -> Element<'_, Message> {
    let log_lines: Vec<Element<Message>> = app
        .deploy_log
        .iter()
        .map(|line| {
            text(line)
                .size(11)
                .font(iced::Font::MONOSPACE)
                .color(colors::TEXT)
                .into()
        })
        .collect();

    let log_content = container(scrollable(column(log_lines).spacing(1)).height(Length::Fill));

    container(log_content.padding(10))
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
        })
        .height(150)
        .into()
}
