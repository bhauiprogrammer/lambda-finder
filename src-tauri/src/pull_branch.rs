use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const BRANCHES: &[&str] = &[
    "release/develop",
    "release/uat",
    "release/pre-prod",
    "production-release",
];

// Same set as makepull.sh (utec-microservices intentionally excluded).
const REPOS_TO_PULL: &[&str] = &[
    "service-requests-backend",
    "polaris-backend",
    "service-sr-tracker",
    "bpd-qrc-backend",
    "service-user-onboarding",
    "backend-lambdas",
];

#[derive(Debug, Serialize, Clone)]
pub struct PullDone {
    pub ok: bool,
    pub error: Option<String>,
}

fn emit_log(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("pull-log", line.into());
}

fn emit_done(app: &AppHandle, ok: bool, error: Option<String>) {
    let _ = app.emit("pull-done", PullDone { ok, error });
}

async fn run_streaming(
    app: &AppHandle,
    program: &str,
    args: &[&str],
    cwd: &PathBuf,
    log_output: bool,
) -> Result<i32, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn error: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_app = app.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if log_output {
                    emit_log(&stdout_app, line);
                }
            }
        }
    });

    let stderr_app = app.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if log_output {
                    emit_log(&stderr_app, line);
                }
            }
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("wait error: {}", e))?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    Ok(status.code().unwrap_or(-1))
}

pub async fn pull_branch(app: AppHandle, repo_root: String, branch: String) {
    if !BRANCHES.contains(&branch.as_str()) {
        emit_log(
            &app,
            format!(
                "!! ERROR: Invalid branch '{}'. Allowed: {}",
                branch,
                BRANCHES.join(", ")
            ),
        );
        emit_done(&app, false, Some(format!("Invalid branch: {}", branch)));
        return;
    }

    let root = PathBuf::from(&repo_root);
    if !root.exists() {
        let msg = format!("Repo root does not exist: {}", repo_root);
        emit_log(&app, format!("!! ERROR: {}", msg));
        emit_done(&app, false, Some(msg));
        return;
    }

    emit_log(&app, format!("==> Using branch: {}", branch));
    emit_log(&app, format!("==> Working in: {}", repo_root));
    emit_log(&app, String::new());

    for repo in REPOS_TO_PULL {
        let repo_path = root.join(repo);
        emit_log(&app, format!("********* Pulling {} ({}) ******", repo, branch));

        if !repo_path.exists() {
            emit_log(
                &app,
                format!(
                    "!! Skipping {} (directory not found at {})",
                    repo,
                    repo_path.display()
                ),
            );
            emit_log(&app, String::new());
            continue;
        }

        let fetch_code = run_streaming(&app, "git", &["fetch", "--all"], &repo_path, true).await;
        if !matches!(fetch_code, Ok(0)) {
            emit_log(
                &app,
                format!("!! git fetch failed for {} ({:?})", repo, fetch_code),
            );
            emit_log(&app, String::new());
            continue;
        }

        let verify_ref = format!("refs/remotes/origin/{}", branch);
        let verify_code = run_streaming(
            &app,
            "git",
            &["show-ref", "--verify", "--quiet", verify_ref.as_str()],
            &repo_path,
            false,
        )
        .await;
        if !matches!(verify_code, Ok(0)) {
            emit_log(
                &app,
                format!(
                    "!! Branch {} not found on origin for {}, skipping.",
                    branch, repo
                ),
            );
            emit_log(&app, String::new());
            continue;
        }

        let checkout_code =
            run_streaming(&app, "git", &["checkout", branch.as_str()], &repo_path, true).await;
        if !matches!(checkout_code, Ok(0)) {
            emit_log(
                &app,
                format!("!! git checkout failed for {} ({:?})", repo, checkout_code),
            );
            emit_log(&app, String::new());
            continue;
        }

        let pull_code = run_streaming(
            &app,
            "git",
            &["pull", "origin", branch.as_str()],
            &repo_path,
            true,
        )
        .await;
        if !matches!(pull_code, Ok(0)) {
            emit_log(
                &app,
                format!("!! git pull failed for {} ({:?})", repo, pull_code),
            );
        }

        emit_log(&app, format!("*********** {} pull is done **************", repo));
        emit_log(&app, String::new());
    }

    emit_log(&app, "==> All done.".to_string());
    emit_done(&app, true, None);
}
