use iced::Task;
use rover_proto::v1::DeployEvent;

use crate::app::{DeployState, RoverApp};
use crate::deploy_job::start_deploy_task;
use crate::message::Message;
use crate::update::{get_client, refresh_apps};

/// Start a background deploy from the current deploy form values.
pub fn submit_deploy(app: &mut RoverApp) -> Task<Message> {
    let use_github = app.deploy_use_github && !app.deploy_github_url.trim().is_empty();
    if (!use_github && app.deploy_path.is_empty())
        || app.deploy_name.trim().is_empty()
        || app.deploy_runtime.is_empty()
    {
        return Task::none();
    }

    let deploy_id = app.next_deploy_id;
    app.next_deploy_id += 1;

    let name = app.deploy_name.trim().to_string();
    let runtime = app.deploy_runtime.clone();
    let build_cmd = app.deploy_build.trim().to_string();
    let run_cmd = app.deploy_run.trim().to_string();
    let source_path = if app.deploy_use_github {
        String::new()
    } else {
        app.deploy_path.clone()
    };
    let github_url = if app.deploy_use_github && !app.deploy_github_url.trim().is_empty() {
        Some(app.deploy_github_url.trim().to_string())
    } else {
        None
    };
    let github_token = if app.deploy_use_github {
        app.selected_github_token.as_ref().and_then(|label| {
            app.github_tokens
                .iter()
                .find(|t| &t.label == label)
                .map(|t| t.token.clone())
        })
    } else {
        None
    };
    let env_vars = app.deploy_env_vars.clone();
    let client = get_client(app);

    app.active_deploys.push(DeployState {
        id: deploy_id,
        name: name.clone(),
        runtime: runtime.clone(),
        source_path: source_path.clone(),
        status: String::from("packaging"),
        logs: vec![String::from("Packaging source...")],
        app_id: None,
        error: None,
    });
    app.expanded_deploy = Some(deploy_id);
    app.deploy_open = false;

    Task::batch([
        Task::done(Message::Info(format!("Deploying {name} in the background"))),
        start_deploy_task(
            deploy_id,
            client,
            name,
            runtime,
            build_cmd,
            run_cmd,
            source_path,
            env_vars,
            github_url,
            github_token,
        ),
    ])
}

/// Apply one event from the server-side deploy stream.
pub fn deploy_event(app: &mut RoverApp, deploy_id: usize, event: DeployEvent) -> Task<Message> {
    let mut tasks = Vec::new();
    let mut refresh = false;
    if let Some(deploy) = app.find_deploy_mut(deploy_id) {
        if let Some(line) = format_deploy_event(&event) {
            deploy.logs.push(line);
        }

        match &event.event {
            Some(rover_proto::v1::deploy_event::Event::Complete(complete)) => {
                deploy.status = String::from("complete");
                deploy.app_id = Some(complete.app_id.clone());
                tasks.push(Task::done(Message::Info(format!(
                    "Deployed {}",
                    deploy.name
                ))));
                refresh = true;
            }
            Some(rover_proto::v1::deploy_event::Event::Error(err)) => {
                deploy.status = String::from("failed");
                deploy.error = Some(err.message.clone());
                tasks.push(Task::done(Message::Toast(format!(
                    "Deploy {} failed: {}",
                    deploy.name, err.message
                ))));
                refresh = true;
            }
            Some(rover_proto::v1::deploy_event::Event::Progress(progress)) => {
                deploy.status = progress.stage.clone();
            }
            Some(rover_proto::v1::deploy_event::Event::Log(_)) | None => {}
        }
    }
    if refresh {
        tasks.push(refresh_apps(app));
    }
    Task::batch(tasks)
}

/// Handle a deploy stream that ended without an explicit terminal event.
pub fn deploy_stream_ended(app: &mut RoverApp, deploy_id: usize) -> Task<Message> {
    let mut tasks = vec![refresh_apps(app)];
    if let Some(deploy) = app.find_deploy_mut(deploy_id) {
        if deploy.is_active() {
            let message = String::from("deploy stream ended before completion");
            deploy.status = String::from("failed");
            deploy.error = Some(message.clone());
            deploy.logs.push(format!("❌ {message}"));
            tasks.push(Task::done(Message::Toast(format!(
                "Deploy {} failed: {message}",
                deploy.name
            ))));
        }
    }
    Task::batch(tasks)
}

/// Mark a background deploy as failed before or while opening the stream.
pub fn deploy_failed(app: &mut RoverApp, deploy_id: usize, error: String) -> Task<Message> {
    if let Some(deploy) = app.find_deploy_mut(deploy_id) {
        deploy.status = String::from("failed");
        deploy.error = Some(error.clone());
        deploy.logs.push(format!("❌ {error}"));
    }
    Task::batch([
        Task::done(Message::Toast(format!("Deploy failed: {error}"))),
        refresh_apps(app),
    ])
}

fn format_deploy_event(event: &DeployEvent) -> Option<String> {
    match &event.event {
        Some(rover_proto::v1::deploy_event::Event::Log(log)) => {
            if log.is_stderr {
                Some(format!("[err] {}", log.line))
            } else {
                Some(log.line.clone())
            }
        }
        Some(rover_proto::v1::deploy_event::Event::Complete(complete)) => {
            Some(format!("✅ Deployed — {}", complete.app_id))
        }
        Some(rover_proto::v1::deploy_event::Event::Error(err)) => {
            Some(format!("❌ {}", err.message))
        }
        Some(rover_proto::v1::deploy_event::Event::Progress(progress)) => {
            Some(format!("[{:.0}%] {}", progress.percent, progress.stage))
        }
        None => None,
    }
}
