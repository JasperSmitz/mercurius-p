use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, timeout};

use crate::model::ExecutionResult;

pub struct ProcessExecutor;

impl ProcessExecutor {
    pub async fn execute(
        command: &str,
        arguments: &[String],
        timeout_ms: u64,
    ) -> Result<ExecutionResult, String> {
        let start = Instant::now();

        let mut child = match Command::new(command)
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return Err(format!("Failed to start process '{command}': {error}"));
            }
        };

        let stdout_task = match child.stdout.take() {
            Some(stdout) => read_pipe_to_string(stdout),
            None => {
                return Err(format!("Failed to capture stdout for process '{command}'"));
            }
        };

        let stderr_task = match child.stderr.take() {
            Some(stderr) => read_pipe_to_string(stderr),
            None => {
                return Err(format!("Failed to capture stderr for process '{command}'"));
            }
        };

        let timeout_duration = Duration::from_millis(timeout_ms);

        let mut timed_out = false;

        let exit_code = match timeout(timeout_duration, child.wait()).await {
            Ok(wait_result) => match wait_result {
                Ok(status) => status.code(),
                Err(error) => {
                    return Err(format!(
                        "Failed while waiting for process '{command}': {error}"
                    ));
                }
            },
            Err(_) => {
                timed_out = true;

                match child.kill().await {
                    Ok(()) => {}
                    Err(error) => {
                        return Err(format!(
                            "Failed to kill timed out process '{command}': {error}"
                        ));
                    }
                }

                match child.wait().await {
                    Ok(_) => {}
                    Err(error) => {
                        return Err(format!(
                            "Failed to wait for killed process '{command}': {error}"
                        ));
                    }
                }

                None
            }
        };

        let stdout = read_task_result(stdout_task, "stdout").await?;
        let stderr = read_task_result(stderr_task, "stderr").await?;

        Ok(ExecutionResult {
            stdout,
            stderr,
            exit_code,
            duration_ms: start.elapsed().as_millis(),
            timed_out,
        })
    }
}

fn read_pipe_to_string<R>(mut pipe: R) -> JoinHandle<Result<String, std::io::Error>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut output = String::new();

        match pipe.read_to_string(&mut output).await {
            Ok(_) => Ok(output),
            Err(error) => Err(error),
        }
    })
}

async fn read_task_result(
    task: JoinHandle<Result<String, std::io::Error>>,
    stream_name: &str,
) -> Result<String, String> {
    match task.await {
        Ok(read_result) => match read_result {
            Ok(output) => Ok(output),
            Err(error) => Err(format!("Failed to read process {stream_name}: {error}")),
        },
        Err(error) => Err(format!("Failed to join {stream_name} reader task: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_command_successfully() {
        let arguments = vec!["--version".to_string()];

        match ProcessExecutor::execute("rustc", &arguments, 5000).await {
            Ok(result) => {
                assert!(!result.timed_out);
                assert_eq!(result.exit_code, Some(0));
                assert!(result.stdout.contains("rustc"));
            }
            Err(error) => {
                panic!("Expected process to execute successfully, but got: {error}");
            }
        }
    }

    #[tokio::test]
    async fn captures_stderr_for_failed_command() {
        let arguments = vec!["--definitely-not-a-real-rustc-flag".to_string()];

        match ProcessExecutor::execute("rustc", &arguments, 5000).await {
            Ok(result) => {
                assert!(!result.timed_out);
                assert_ne!(result.exit_code, Some(0));
                assert!(!result.stderr.trim().is_empty());
            }
            Err(error) => {
                panic!("Expected process execution result, but got startup error: {error}");
            }
        }
    }

    #[tokio::test]
    async fn returns_error_for_missing_command() {
        let arguments = Vec::new();

        match ProcessExecutor::execute("definitely-not-a-real-command", &arguments, 5000).await {
            Ok(result) => {
                panic!("Expected missing command to fail, but got: {result:?}");
            }
            Err(error) => {
                assert!(error.contains("Failed to start process"));
            }
        }
    }

    #[tokio::test]
    async fn times_out_long_running_process() {
        let (command, arguments) = long_running_command();

        match ProcessExecutor::execute(command, &arguments, 100).await {
            Ok(result) => {
                assert!(result.timed_out);
                assert_eq!(result.exit_code, None);
            }
            Err(error) => {
                panic!("Expected timeout result, but got error: {error}");
            }
        }
    }

    #[cfg(unix)]
    fn long_running_command() -> (&'static str, Vec<String>) {
        ("sh", vec!["-c".to_string(), "sleep 5".to_string()])
    }

    #[cfg(windows)]
    fn long_running_command() -> (&'static str, Vec<String>) {
        (
            "cmd",
            vec!["/C".to_string(), "ping 127.0.0.1 -n 6 > nul".to_string()],
        )
    }
}
