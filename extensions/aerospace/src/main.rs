#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use serde_json::json;
use spindle_aerospace_extension::{
    focus_workspace, read_layout, read_mode, read_workspace_snapshot,
};
use spindle_extension_sdk::{
    ActionContext, ActionHandler, ActionInvocation, ActionOutput, ActionOutputEvent,
    ExtensionRegistration, RegistrationAction, serve_stdio_jsonl_actions,
};

const EVENT_WORKSPACE_CHANGED: &str = "aerospace.workspace.changed";
const EVENT_FOCUS_CHANGED: &str = "aerospace.focus.changed";
const EVENT_MONITOR_CHANGED: &str = "aerospace.monitor.changed";
const EVENT_MODE_CHANGED: &str = "aerospace.mode.changed";
const EVENT_LAYOUT_CHANGED: &str = "aerospace.layout.changed";
const ACTION_WORKSPACE_FOCUS: &str = "aerospace.workspace.focus";
const ACTION_WORKSPACE_SNAPSHOT: &str = "aerospace.workspace.snapshot";
const ACTION_MODE_SNAPSHOT: &str = "aerospace.mode.snapshot";
const ACTION_LAYOUT_SNAPSHOT: &str = "aerospace.layout.snapshot";
const CAPABILITY_STATE_READ: &str = "aerospace.state.read";
const CAPABILITY_WINDOW_CONTROL: &str = "aerospace.window.control";

fn main() -> Result<()> {
    Cli::parse().run()
}

#[derive(Debug, Parser)]
#[command(name = "spindle-aerospace", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

impl Cli {
    fn run(self) -> Result<()> {
        let Some(command) = self.command else {
            return serve_host();
        };
        match command {
            CliCommand::Register => {
                print_registration(&registration())?;
            }
            CliCommand::FocusWorkspace(args) => {
                let context = cli_context(
                    ACTION_WORKSPACE_FOCUS,
                    json!({ "workspace": &args.workspace }),
                );
                let workspace = focus_workspace_action(args.workspace, &context)?;
                print_output(&workspace_snapshot_output(Some(&workspace))?)?;
            }
            CliCommand::EmitWorkspaceSnapshot(args) => {
                let context = cli_context(
                    ACTION_WORKSPACE_SNAPSHOT,
                    json!({
                        "focused_workspace": args.focused_workspace,
                    }),
                );
                print_output(&workspace_snapshot_action(&context)?)?;
            }
            CliCommand::EmitModeSnapshot => {
                print_output(&mode_snapshot_action()?)?;
            }
            CliCommand::EmitLayoutSnapshot => {
                print_output(&layout_snapshot_action()?)?;
            }
        }
        Ok(())
    }
}

fn serve_host() -> Result<()> {
    let registration = registration();
    Ok(serve_stdio_jsonl_actions(&registration, ACTION_HANDLERS)?)
}

const ACTION_HANDLERS: &[ActionHandler<anyhow::Error>] = &[
    ActionHandler::new(ACTION_WORKSPACE_FOCUS, focus_workspace_host_action),
    ActionHandler::new(ACTION_WORKSPACE_SNAPSHOT, workspace_snapshot_action),
    ActionHandler::new(ACTION_MODE_SNAPSHOT, mode_snapshot_host_action),
    ActionHandler::new(ACTION_LAYOUT_SNAPSHOT, layout_snapshot_host_action),
];

fn focus_workspace_host_action(context: &ActionContext) -> Result<ActionOutput> {
    let workspace = focus_workspace_action(None, context)?;
    workspace_snapshot_output(Some(&workspace)).or_else(|error| {
        eprintln!("aerospace: workspace focus applied but snapshot read failed: {error:#}");
        Ok(ActionOutput::empty())
    })
}

fn mode_snapshot_host_action(_context: &ActionContext) -> Result<ActionOutput> {
    mode_snapshot_action()
}

fn layout_snapshot_host_action(_context: &ActionContext) -> Result<ActionOutput> {
    layout_snapshot_action()
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Register this extension's spindle surface.
    Register,
    /// Focus an `AeroSpace` workspace.
    FocusWorkspace(FocusWorkspaceArgs),
    /// Emit current `AeroSpace` workspace state.
    EmitWorkspaceSnapshot(WorkspaceSnapshotArgs),
    /// Emit current `AeroSpace` mode state.
    EmitModeSnapshot,
    /// Emit current focused-window layout state.
    EmitLayoutSnapshot,
}

#[derive(Debug, Args)]
struct FocusWorkspaceArgs {
    #[arg(value_name = "WORKSPACE")]
    workspace: Option<String>,
}

#[derive(Debug, Args)]
struct WorkspaceSnapshotArgs {
    #[arg(long, value_name = "WORKSPACE")]
    focused_workspace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FocusWorkspaceActionArgs {
    name: Option<String>,
    workspace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceSnapshotActionArgs {
    focused_workspace: Option<String>,
}

fn resolve_workspace(
    explicit: Option<String>,
    action_args: &FocusWorkspaceActionArgs,
) -> Result<String> {
    explicit
        .or_else(|| action_args.workspace.clone())
        .or_else(|| action_args.name.clone())
        .context("workspace was not provided")
}

fn focus_workspace_action(explicit: Option<String>, context: &ActionContext) -> Result<String> {
    let action_args = context.args::<FocusWorkspaceActionArgs>()?;
    let workspace = resolve_workspace(explicit, &action_args)?;
    focus_workspace(&workspace)?;
    Ok(workspace)
}

fn workspace_snapshot_action(context: &ActionContext) -> Result<ActionOutput> {
    let action_args = context.args::<WorkspaceSnapshotActionArgs>()?;
    workspace_snapshot_output(action_args.focused_workspace.as_deref())
}

fn workspace_snapshot_output(focused_workspace: Option<&str>) -> Result<ActionOutput> {
    let snapshot = read_workspace_snapshot(focused_workspace)?;
    Ok(ActionOutput::event(
        ActionOutputEvent::new(ACTION_WORKSPACE_SNAPSHOT)
            .with_data(serde_json::to_value(snapshot)?),
    ))
}

fn mode_snapshot_action() -> Result<ActionOutput> {
    let mode = read_mode()?;
    Ok(ActionOutput::event(
        ActionOutputEvent::new(ACTION_MODE_SNAPSHOT).with_data(json!({ "mode": mode })),
    ))
}

fn layout_snapshot_action() -> Result<ActionOutput> {
    let window_info = read_layout()?;
    Ok(ActionOutput::event(
        ActionOutputEvent::new(ACTION_LAYOUT_SNAPSHOT)
            .with_data(json!({ "window_info": window_info })),
    ))
}

fn print_output(output: &ActionOutput) -> Result<()> {
    println!("{}", output.to_json_string()?);
    Ok(())
}

fn print_registration(registration: &ExtensionRegistration) -> Result<()> {
    println!("{}", registration.to_json_string()?);
    Ok(())
}

fn cli_context(action: &str, args: serde_json::Value) -> ActionContext {
    ActionContext::from_invocation(ActionInvocation::new(action, args))
}

fn registration() -> ExtensionRegistration {
    ExtensionRegistration::new()
        .emit(EVENT_WORKSPACE_CHANGED)
        .emit(EVENT_FOCUS_CHANGED)
        .emit(EVENT_MONITOR_CHANGED)
        .emit(EVENT_MODE_CHANGED)
        .emit(EVENT_LAYOUT_CHANGED)
        .produce(ACTION_WORKSPACE_SNAPSHOT)
        .produce(ACTION_MODE_SNAPSHOT)
        .produce(ACTION_LAYOUT_SNAPSHOT)
        .capability(CAPABILITY_STATE_READ)
        .capability(CAPABILITY_WINDOW_CONTROL)
        .action(
            ACTION_WORKSPACE_FOCUS,
            RegistrationAction::new().capability(CAPABILITY_WINDOW_CONTROL),
        )
        .action(
            ACTION_WORKSPACE_SNAPSHOT,
            RegistrationAction::new().capability(CAPABILITY_STATE_READ),
        )
        .action(
            ACTION_MODE_SNAPSHOT,
            RegistrationAction::new().capability(CAPABILITY_STATE_READ),
        )
        .action(
            ACTION_LAYOUT_SNAPSHOT,
            RegistrationAction::new().capability(CAPABILITY_STATE_READ),
        )
}
