use std::path::Path;
use std::process::Command;

use crate::error::Result;
use crate::logger::Logger;

pub(crate) fn git_stdout(repo: &Path, args: &[&str], logger: Logger) -> Result<Option<String>> {
    logger.log(format!("running {}", format_git_command(repo, args)));

    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git in {}: {err}", repo.display()))?;

    if !output.status.success() {
        logger.log(format!(
            "git command failed with status {}",
            output
                .status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        ));
        return Ok(None);
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

pub(crate) fn git_success(repo: &Path, args: &[&str], logger: Logger) -> Result<bool> {
    logger.log(format!("running {}", format_git_command(repo, args)));

    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git in {}: {err}", repo.display()))?;

    logger.log(format!(
        "git command exited with status {}",
        output
            .status
            .code()
            .map_or_else(|| "signal".to_string(), |code| code.to_string())
    ));

    Ok(output.status.success())
}

fn format_git_command(repo: &Path, args: &[&str]) -> String {
    let mut command = format!("git -C {}", repo.display());

    for arg in args {
        command.push(' ');
        command.push_str(arg);
    }

    command
}
