mod common;

use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixListener,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use common::test_socket_path;
use serde_json::json;

fn aerospace_success_response(stdout: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string(&json!({
        "serverVersionAndHash": "0.20.0-Beta abc",
        "stdout": stdout,
        "stderr": "",
        "exitCode": 0
    }))?)
}

fn aerospace_error_response() -> anyhow::Result<String> {
    Ok(serde_json::to_string(&json!({
        "serverVersionAndHash": "0.20.0-Beta abc",
        "stdout": "",
        "stderr": "failed",
        "exitCode": 1
    }))?)
}

fn handle_ipc_connection(listener: &UnixListener, response: &str) -> anyhow::Result<String> {
    let (mut stream, _) = listener.accept()?;
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer)?;
    let request = String::from_utf8(buffer[..read].to_vec())?;
    stream.write_all(response.as_bytes())?;
    Ok(request)
}

const NO_PENDING_CONNECTION_DEADLINE: Duration = Duration::from_millis(50);
const NO_PENDING_CONNECTION_POLL: Duration = Duration::from_millis(5);

fn ensure_no_pending_connection(listener: &UnixListener) -> anyhow::Result<()> {
    listener.set_nonblocking(true)?;
    let start = Instant::now();
    let result = loop {
        match listener.accept() {
            Ok(_) => break Err(anyhow::anyhow!("unexpected extra IPC connection")),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= NO_PENDING_CONNECTION_DEADLINE {
                    break Ok(());
                }
                thread::sleep(NO_PENDING_CONNECTION_POLL);
            }
            Err(error) => break Err(error.into()),
        }
    };
    listener.set_nonblocking(false)?;
    result
}

fn read_host_response_line(reader: &mut impl BufRead) -> anyhow::Result<serde_json::Value> {
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;
    serde_json::from_str(&response_line).map_err(Into::into)
}

fn run_host_focus(
    socket_path: &Path,
    server: thread::JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<(serde_json::Value, String)> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_spindle-aerospace"))
        .env("AEROSPACESOCK", socket_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("child stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("child stdout unavailable"))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("child stderr unavailable"))?;

    writeln!(
        stdin,
        r#"{{"type":"invoke","invocation":{{"action":"aerospace.workspace.focus","args":{{"workspace":"2"}},"event":null,"extension":null}}}}"#
    )?;
    stdin.flush()?;

    let mut stdout = BufReader::new(stdout);
    let response = read_host_response_line(&mut stdout)?;

    writeln!(stdin, r#"{{"type":"shutdown"}}"#)?;
    stdin.flush()?;
    drop(stdin);

    // Consume the shutdown acknowledgement before closing stdout.
    let _shutdown_ack = read_host_response_line(&mut stdout);
    let stderr = BufReader::new(stderr_pipe)
        .lines()
        .map_while(Result::ok)
        .collect::<Vec<_>>()
        .join("");
    let status = child.wait()?;
    server
        .join()
        .map_err(|_payload| anyhow::anyhow!("server thread panicked"))??;

    assert!(
        status.success(),
        "host exited with failure status: {stderr}"
    );
    Ok((response, stderr))
}

#[test]
fn cli_focus_workspace_uses_aerospace_socket() -> anyhow::Result<()> {
    let (_dir, socket_path) = test_socket_path()?;
    let listener = UnixListener::bind(&socket_path)?;
    let handle = thread::spawn(move || -> anyhow::Result<String> {
        let request = handle_ipc_connection(&listener, &aerospace_success_response("")?)?;
        handle_ipc_connection(&listener, &aerospace_success_response("2\n3\n")?)?;
        ensure_no_pending_connection(&listener)?;
        Ok(request)
    });

    let focus = Command::new(env!("CARGO_BIN_EXE_spindle-aerospace"))
        .arg("focus-workspace")
        .arg("2")
        .env("AEROSPACESOCK", &socket_path)
        .output()?;

    assert!(
        focus.status.success(),
        "focus-workspace failed: {}",
        String::from_utf8_lossy(&focus.stderr)
    );
    let request = handle
        .join()
        .map_err(|_payload| anyhow::anyhow!("server thread panicked"))??;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request)?,
        json!({
            "command": "",
            "args": ["workspace", "2"],
            "stdin": "",
            "windowId": null,
            "workspace": null
        })
    );

    Ok(())
}

#[test]
fn cli_uses_focused_workspace_env_fallback() -> anyhow::Result<()> {
    let (_dir, socket_path) = test_socket_path()?;
    let listener = UnixListener::bind(&socket_path)?;
    let handle = thread::spawn(move || -> anyhow::Result<String> {
        let request = handle_ipc_connection(&listener, &aerospace_success_response("")?)?;
        handle_ipc_connection(&listener, &aerospace_success_response("dev\n3\n")?)?;
        ensure_no_pending_connection(&listener)?;
        Ok(request)
    });

    let focus = Command::new(env!("CARGO_BIN_EXE_spindle-aerospace"))
        .arg("focus-workspace")
        .arg("2")
        .env("AEROSPACESOCK", &socket_path)
        .env_remove("AEROSPACE_WORKSPACE")
        .env("AEROSPACE_FOCUSED_WORKSPACE", "dev")
        .output()?;

    assert!(
        focus.status.success(),
        "focus-workspace failed: {}",
        String::from_utf8_lossy(&focus.stderr)
    );
    let request = handle
        .join()
        .map_err(|_payload| anyhow::anyhow!("server thread panicked"))??;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request)?["workspace"],
        "dev"
    );

    Ok(())
}

#[test]
fn host_focus_workspace_emits_workspace_snapshot() -> anyhow::Result<()> {
    let (_dir, socket_path) = test_socket_path()?;
    let listener = UnixListener::bind(&socket_path)?;
    let server = thread::spawn(move || -> anyhow::Result<()> {
        handle_ipc_connection(&listener, &aerospace_success_response("")?)?;
        handle_ipc_connection(&listener, &aerospace_success_response("2\n3\n")?)?;
        ensure_no_pending_connection(&listener)?;
        Ok(())
    });

    let (response, _stderr) = run_host_focus(&socket_path, server)?;

    assert_eq!(response["type"], "action-output");
    assert_eq!(
        response["output"]["events"][0]["type"],
        "aerospace.workspace.snapshot"
    );
    assert_eq!(response["output"]["events"][0]["data"]["active"], "2");
    Ok(())
}

#[test]
fn host_focus_workspace_snapshot_failure_returns_empty_output() -> anyhow::Result<()> {
    let (_dir, socket_path) = test_socket_path()?;
    let listener = UnixListener::bind(&socket_path)?;
    let server = thread::spawn(move || -> anyhow::Result<()> {
        handle_ipc_connection(&listener, &aerospace_success_response("")?)?;
        handle_ipc_connection(&listener, &aerospace_error_response()?)?;
        ensure_no_pending_connection(&listener)?;
        Ok(())
    });

    let (response, stderr) = run_host_focus(&socket_path, server)?;

    assert_eq!(response["type"], "action-output");
    assert!(
        response["output"]["events"].is_null()
            || response["output"]["events"]
                .as_array()
                .is_some_and(std::vec::Vec::is_empty),
        "expected empty events on snapshot failure: {response}"
    );
    assert!(
        stderr.contains("snapshot read failed"),
        "expected snapshot failure log on stderr: {stderr}"
    );
    Ok(())
}

#[test]
fn host_focus_workspace_failure_returns_error() -> anyhow::Result<()> {
    let (_dir, socket_path) = test_socket_path()?;
    let listener = UnixListener::bind(&socket_path)?;
    let server = thread::spawn(move || -> anyhow::Result<()> {
        handle_ipc_connection(&listener, &aerospace_error_response()?)?;
        ensure_no_pending_connection(&listener)?;
        Ok(())
    });

    let (response, _stderr) = run_host_focus(&socket_path, server)?;

    assert_eq!(response["type"], "error");
    Ok(())
}

#[test]
fn cli_register_ignores_unrelated_spindle_env() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_spindle-aerospace"))
        .arg("register")
        .env("SPINDLE_UNUSED_TEST_ENV", "{not json")
        .output()?;

    assert!(
        output.status.success(),
        "register failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains(r#""aerospace.workspace.snapshot""#));
    Ok(())
}
