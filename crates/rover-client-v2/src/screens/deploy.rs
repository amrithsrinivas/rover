/// Deploy screen — modal form for deploying a new application.
use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use lucide_icons::iced::{icon_folder_open, icon_plus, icon_rocket, icon_trash_2, icon_x};

use crate::app::RoverApp;
use crate::message::Message;
use crate::theme;

const RUNTIMES: &[&str] = &["python", "node", "go", "rust"];

/// Render the deploy modal overlay.
pub fn deploy_modal(app: &RoverApp) -> Element<'_, Message> {
    let modal = container(deploy_form(app)).style(|_theme| container::Style {
        background: Some(iced::Background::Color(theme::PAPER)),
        border: iced::Border {
            color: theme::BORDER,
            width: 1.0,
            radius: theme::RADIUS_LG.into(),
        },
        shadow: theme::shadow_overlay(),
        ..container::Style::default()
    });

    container(
        container(modal.width(Length::Fill))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding([40, 60])
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

fn deploy_form(app: &RoverApp) -> Element<'_, Message> {
    let title_row = row![
        row![
            icon_rocket().size(18).color(theme::BLUE),
            Space::with_width(8),
            text("Deploy Application")
                .size(theme::TEXT_XL)
                .color(theme::INK_PRIMARY),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
        button(icon_x().size(16).color(theme::INK_SECONDARY))
            .style(button::text)
            .on_press(Message::CloseDeploy),
    ]
    .align_y(Alignment::Center);

    let name_input = text_input("my-app", &app.deploy_name)
        .on_input(Message::SetDeployName)
        .size(theme::TEXT_SM)
        .width(Length::Fill);

    let runtime_picker = pick_list(
        RUNTIMES,
        if app.deploy_runtime.is_empty() {
            None
        } else {
            Some(app.deploy_runtime.as_str())
        },
        |s| Message::SetDeployRuntime(s.to_string()),
    )
    .placeholder("Select runtime...");

    let build_input = text_input("Build command", &app.deploy_build)
        .on_input(Message::SetDeployBuild)
        .size(theme::TEXT_SM)
        .width(Length::Fill);

    let run_input = text_input("Run command", &app.deploy_run)
        .on_input(Message::SetDeployRun)
        .size(theme::TEXT_SM)
        .width(Length::Fill);

    let source_toggle = row![
        text(if app.deploy_use_github {
            "GitHub Source"
        } else {
            "Local Directory"
        })
        .size(theme::TEXT_SM)
        .color(theme::INK_SECONDARY),
        Space::with_width(Length::Fill),
        button(
            text(if app.deploy_use_github {
                "Use Local Dir"
            } else {
                "Use GitHub"
            })
            .size(theme::TEXT_SM),
        )
        .style(button::text)
        .on_press(Message::ToggleGithub),
    ]
    .align_y(Alignment::Center);

    let source_row: Element<Message> = if app.deploy_use_github {
        github_source(app)
    } else {
        local_source(app)
    };

    let env_section = env_vars_section(app);
    let deploy_btn = deploy_button(app);

    let form = column![
        title_row,
        Space::with_height(theme::SPACE_LG),
        // Target server picker
        field_label("Deploy to"),
        Space::with_height(4),
        target_picker(app),
        Space::with_height(theme::SPACE_MD),
        field_label("App Name"),
        Space::with_height(4),
        name_input,
        Space::with_height(theme::SPACE_MD),
        field_label("Runtime"),
        Space::with_height(4),
        runtime_picker,
        Space::with_height(theme::SPACE_MD),
        field_label("Build Command"),
        Space::with_height(4),
        build_input,
        Space::with_height(theme::SPACE_MD),
        field_label("Run Command"),
        Space::with_height(4),
        run_input,
        Space::with_height(theme::SPACE_MD),
        field_label("Source"),
        Space::with_height(4),
        source_toggle,
        Space::with_height(theme::SPACE_SM),
        source_row,
        Space::with_height(theme::SPACE_MD),
        env_section,
        Space::with_height(theme::SPACE_LG),
        deploy_btn,
    ]
    .spacing(0)
    .padding(theme::SPACE_XL);

    scrollable(form).height(Length::Fill).into()
}

fn field_label(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(TEXT_LABEL)
        .color(theme::INK_SECONDARY)
        .into()
}

fn local_source(app: &RoverApp) -> Element<'_, Message> {
    row![
        text_input("Source directory path", &app.deploy_path)
            .on_input(Message::SetDeployPath)
            .size(theme::TEXT_SM)
            .width(Length::Fill),
        Space::with_width(theme::SPACE_SM),
        button(
            row![
                icon_folder_open().size(13),
                Space::with_width(4),
                text("Browse").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::secondary)
        .on_press(Message::PickPath),
    ]
    .spacing(0)
    .into()
}

fn github_source(app: &RoverApp) -> Element<'_, Message> {
    let token_labels: Vec<String> = app.github_tokens.iter().map(|t| t.label.clone()).collect();

    let token_row = row![
        pick_list(token_labels, app.selected_github_token.clone(), |s| {
            Message::SelectGithubToken(Some(s))
        },)
        .placeholder("No token (public repo)")
        .width(Length::Fill),
        button(icon_x().size(12).color(theme::INK_SECONDARY))
            .style(button::text)
            .on_press(Message::SelectGithubToken(None)),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let add_token_row = row![
        text_input("Label (e.g. Personal)", &app.new_token_label)
            .on_input(Message::SetNewTokenLabel)
            .size(theme::TEXT_SM)
            .width(Length::Fixed(150.0)),
        text_input("ghp_...", &app.new_token_value)
            .on_input(Message::SetNewTokenValue)
            .size(theme::TEXT_SM)
            .width(Length::Fill),
        button(
            row![
                icon_plus().size(12),
                Space::with_width(4),
                text("Save").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::primary)
        .on_press(Message::SaveGithubToken),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    column![
        text_input("https://github.com/user/repo", &app.deploy_github_url)
            .on_input(Message::SetDeployGithubUrl)
            .size(theme::TEXT_SM)
            .width(Length::Fill),
        Space::with_height(6),
        text("Access Token")
            .size(TEXT_LABEL)
            .color(theme::INK_SECONDARY),
        Space::with_height(2),
        token_row,
        Space::with_height(4),
        add_token_row,
    ]
    .spacing(0)
    .into()
}

fn env_vars_section(app: &RoverApp) -> Element<'_, Message> {
    let env_rows: Vec<Element<Message>> = app
        .deploy_env_vars
        .iter()
        .enumerate()
        .map(|(i, (k, v))| {
            container(
                row![
                    text(k).size(theme::TEXT_SM).color(theme::BLUE),
                    text("=").size(theme::TEXT_SM).color(theme::INK_SECONDARY),
                    container(text(v).size(theme::TEXT_SM).color(theme::INK_PRIMARY))
                        .width(Length::Fill)
                        .clip(true),
                    button(icon_trash_2().size(11).color(theme::RED))
                        .style(button::text)
                        .on_press(Message::RemoveEnvVar(i)),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([4, 10])
            .style(move |_theme| container::Style {
                border: iced::Border {
                    color: theme::BORDER,
                    width: 1.0,
                    radius: theme::RADIUS_SM.into(),
                },
                ..container::Style::default()
            })
            .into()
        })
        .collect();

    let import_row = row![
        button(
            row![
                icon_folder_open().size(13),
                Space::with_width(4),
                text("Import .env file").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::text)
        .on_press(Message::PickEnvFile),
    ]
    .align_y(Alignment::Center);

    let add_row = row![
        text_input("KEY", &app.deploy_env_key)
            .on_input(Message::SetEnvKey)
            .size(theme::TEXT_SM)
            .width(Length::Fixed(140.0)),
        text_input("VALUE", &app.deploy_env_value)
            .on_input(Message::SetEnvValue)
            .size(theme::TEXT_SM)
            .width(Length::Fixed(240.0)),
        button(
            row![
                icon_plus().size(12),
                Space::with_width(4),
                text("Add").size(theme::TEXT_SM),
            ]
            .align_y(Alignment::Center),
        )
        .style(button::secondary)
        .on_press(Message::AddEnvVar),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let mut col = column![
        text("Environment Variables")
            .size(TEXT_LABEL)
            .color(theme::INK_SECONDARY),
        Space::with_height(6),
        import_row,
        Space::with_height(6),
        add_row,
    ]
    .spacing(0);

    if env_rows.is_empty() {
        col = col.push(Space::with_height(6));
        col = col.push(
            text("No variables set")
                .size(theme::TEXT_SM)
                .color(theme::INK_SECONDARY),
        );
    } else {
        for row_elem in env_rows {
            col = col.push(Space::with_height(4));
            col = col.push(row_elem);
        }
    }

    col.into()
}

fn deploy_button(app: &RoverApp) -> Element<'static, Message> {
    let use_github = app.deploy_use_github && !app.deploy_github_url.trim().is_empty();

    let (label, can_submit) = if app.deploy_target.is_none() {
        ("Select a target server", false)
    } else if app.deploy_name.trim().is_empty() {
        ("Enter an app name", false)
    } else if app.deploy_runtime.is_empty() {
        ("Select a runtime", false)
    } else if !use_github && app.deploy_path.is_empty() {
        ("Select a source directory", false)
    } else {
        ("Deploy", true)
    };

    let btn = button(
        row![
            icon_rocket().size(14),
            Space::with_width(6),
            text(label).size(theme::TEXT_BASE),
        ]
        .align_y(Alignment::Center)
        .padding([10, 20]),
    )
    .width(Length::Fill)
    .style(button::primary);

    if can_submit {
        btn.on_press(Message::SubmitDeploy).into()
    } else {
        btn.into()
    }
}

// ── Target server picker ──────────────────────────────────────────────────────

fn target_picker(app: &RoverApp) -> Element<'_, Message> {
    let connected: Vec<String> = app
        .servers
        .iter()
        .enumerate()
        .filter(|(_, s)| s.connected)
        .map(|(_, s)| {
            format!(
                "{} — {} ({} apps)",
                s.profile.name,
                s.profile.address,
                s.app_count()
            )
        })
        .collect();

    if connected.is_empty() {
        return text("No connected servers available")
            .size(theme::TEXT_SM)
            .color(theme::RED)
            .into();
    }

    let connected_indices: Vec<usize> = app
        .servers
        .iter()
        .enumerate()
        .filter(|(_, s)| s.connected)
        .map(|(i, _)| i)
        .collect();

    let selected_label = app.deploy_target.and_then(|idx| {
        app.servers.get(idx).map(|s| {
            format!(
                "{} — {} ({} apps)",
                s.profile.name,
                s.profile.address,
                s.app_count()
            )
        })
    });

    pick_list(connected.clone(), selected_label, move |chosen| {
        let idx = connected
            .iter()
            .position(|s| s == &chosen)
            .and_then(|pos| connected_indices.get(pos).copied());
        Message::SetDeployTarget(idx)
    })
    .placeholder("Select a server...")
    .into()
}

const TEXT_LABEL: u16 = 10;
