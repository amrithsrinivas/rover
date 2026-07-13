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
        button(text("\u{2715}").size(16).color(colors::TEXT_MUTED))
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

    let source_toggle = row![
        text(if app.deploy_use_github { "GitHub URL" } else { "Local Directory" })
            .size(11)
            .color(colors::TEXT_MUTED),
        Space::with_width(Length::Fill),
        button(text(if app.deploy_use_github { "Use Local Dir" } else { "Use GitHub" }).size(11))
            .style(button::secondary)
            .on_press(Message::ToggleGithub),
    ]
    .align_y(Alignment::Center);

    let source_row: Element<Message> = if app.deploy_use_github {
        let token_labels: Vec<String> = app
            .github_tokens
            .iter()
            .map(|t| t.label.clone())
            .collect();

        let token_row = row![
            pick_list(
                token_labels,
                app.selected_github_token.clone(),
                |s| Message::SelectGithubToken(Some(s)),
            )
            .placeholder("No token (public repo)")
            .width(Length::Fill),
            button(text("\u{2715}").size(11).color(colors::TEXT_MUTED))
                .style(button::text)
                .on_press(Message::SelectGithubToken(None)),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        let add_token_row = row![
            text_input("Label (e.g. Personal)", &app.new_token_label)
                .on_input(Message::SetNewTokenLabel)
                .size(12)
                .width(Length::Fixed(150.0)),
            text_input("ghp_...", &app.new_token_value)
                .on_input(Message::SetNewTokenValue)
                .size(12)
                .width(Length::Fill),
            button(text("Save").size(12))
                .style(button::primary)
                .on_press(Message::SaveGithubToken),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        column![
            text_input("https://github.com/user/repo", &app.deploy_github_url)
                .on_input(Message::SetDGithubUrl)
                .size(13)
                .width(Length::Fill),
            Space::with_height(6),
            text("Access Token").size(10).color(colors::TEXT_MUTED),
            Space::with_height(2),
            token_row,
            Space::with_height(4),
            add_token_row,
        ]
        .spacing(0)
        .into()
    } else {
        row![
            text_input("Source directory path", &app.deploy_path)
                .on_input(Message::SetDPath)
                .size(13)
                .width(Length::Fill),
            button(text("Browse").size(12))
                .style(button::secondary)
                .on_press(Message::PickPath),
        ]
        .spacing(8)
        .into()
    };

    let env_section = deploy_env_section(app);

    let use_github = app.deploy_use_github && !app.deploy_github_url.trim().is_empty();
    let deploy_btn = if app.deploy_name.trim().is_empty() {
        button(text("Enter an app name").size(14))
            .width(Length::Fill)
            .style(button::primary)
    } else if app.deploy_runtime.is_empty() {
        button(text("Select a runtime").size(14))
            .width(Length::Fill)
            .style(button::primary)
    } else if !use_github && app.deploy_path.is_empty() {
        button(text("Select a source directory").size(14))
            .width(Length::Fill)
            .style(button::primary)
    } else {
        button(text("Deploy").size(14))
            .width(Length::Fill)
            .style(button::primary)
            .on_press(Message::SubmitDeploy)
    };

    let form = column![
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
        text("Source").size(10).color(colors::TEXT_MUTED),
        source_toggle,
        Space::with_height(4),
        source_row,
        Space::with_height(16),
        env_section,
        Space::with_height(16),
        deploy_btn,
    ]
    .spacing(0)
    .padding(24);

    scrollable(form).height(Length::Fill).into()
}

fn deploy_env_section(app: &RoverApp) -> Element<'_, Message> {
    let pill_bg = iced::Color::from_rgba(0.06, 0.06, 0.12, 0.8);

    let env_rows: Vec<Element<Message>> = app
        .deploy_env_vars
        .iter()
        .enumerate()
        .map(|(i, (k, v))| {
            container(
                row![
                    text(k).size(12).color(colors::ACCENT),
                    text("=").size(12).color(colors::TEXT_MUTED),
                    container(text(v).size(12).color(colors::TEXT))
                        .width(Length::Fill)
                        .clip(true),
                    button(text("\u{2715}").size(10).color(colors::TEXT_MUTED))
                        .style(button::text)
                        .on_press(Message::RemoveDEVar(i)),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([4, 10])
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(pill_bg)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            })
            .into()
        })
        .collect();

    let import_row = row![
        text_input("Path to .env file", &app.deploy_env_file)
            .on_input(Message::SetDEnvFile)
            .size(12)
            .width(Length::Fixed(348.0)),
        button(text("Browse").size(12))
            .style(button::secondary)
            .on_press(Message::PickEnvFile),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

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
        import_row,
        Space::with_height(4),
        add_row,
    ]
    .spacing(0);

    if env_rows.is_empty() {
        col = col.push(Space::with_height(4));
        col = col.push(text("None").size(11).color(colors::TEXT_MUTED));
    } else {
        for row in env_rows {
            col = col.push(Space::with_height(4));
            col = col.push(row);
        }
    }

    col.into()
}
