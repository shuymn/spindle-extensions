#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use spindle_extension_sdk::{
    ActionContext, ActionHandler, ActionInvocation, ActionOutput, ExtensionRegistration,
    RegistrationAction, serve_stdio_jsonl_actions,
};
use spindle_sketchybar_extension::{
    CachedMessageRequest, MachEndpointProbe, SketchybarMachClient, invalidate_cache,
    send_cached_message,
};

const ACTION_SEND_MESSAGE: &str = "sketchybar.message.send";
const EVENT_WORKSPACE_CLICKED: &str = "sketchybar.workspace.clicked";
const CAPABILITY_UI_WRITE: &str = "sketchybar.ui.write";

fn main() -> Result<()> {
    Cli::parse().run()
}

#[derive(Debug, Parser)]
#[command(name = "spindle-sketchybar", version)]
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
            CliCommand::SendMessage(args) => {
                let context = cli_context(
                    ACTION_SEND_MESSAGE,
                    serde_json::json!({
                        "state_dir": args.state_dir,
                        "cache_key": args.cache_key,
                        "cache_value": args.cache_value,
                    }),
                );
                send_message_action(None, args.args, &context)?;
            }
            CliCommand::InvalidateCache(args) => {
                invalidate_cache(args.state_dir, args.key.as_deref())
                    .map_err(anyhow::Error::from)?;
            }
        }
        Ok(())
    }
}

fn serve_host() -> Result<()> {
    let registration = registration();
    Ok(serve_stdio_jsonl_actions(&registration, ACTION_HANDLERS)?)
}

const ACTION_HANDLERS: &[ActionHandler<anyhow::Error>] = &[ActionHandler::new(
    ACTION_SEND_MESSAGE,
    send_message_host_action,
)];

fn send_message_host_action(context: &ActionContext) -> Result<ActionOutput> {
    send_message_action(None, Vec::new(), context)?;
    Ok(ActionOutput::empty())
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Register this extension's spindle surface.
    Register,
    /// Send raw `SketchyBar` command arguments.
    SendMessage(SendMessageArgs),
    /// Delete write-cache entries used by `sketchybar.message.send`.
    InvalidateCache(InvalidateCacheArgs),
}

#[derive(Debug, Args)]
struct SendMessageArgs {
    #[arg(long, value_name = "DIR")]
    state_dir: Option<PathBuf>,
    #[arg(long, value_name = "KEY")]
    cache_key: Option<String>,
    #[arg(long, value_name = "VALUE")]
    cache_value: Option<String>,
    #[arg(value_name = "ARG")]
    args: Vec<String>,
}

#[derive(Debug, Args)]
struct InvalidateCacheArgs {
    #[arg(long, value_name = "DIR")]
    state_dir: Option<PathBuf>,
    #[arg(long, value_name = "KEY")]
    key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SendMessageActionArgs {
    args: Vec<String>,
    state_dir: Option<PathBuf>,
    cache_key: Option<String>,
    cache_value: Option<String>,
}

fn resolve_message_args(cli_args: Vec<String>, action_args: Vec<String>) -> Result<Vec<String>> {
    let args = if cli_args.is_empty() {
        action_args
    } else {
        cli_args
    };
    if args.is_empty() {
        return Err(anyhow::anyhow!("no SketchyBar arguments were provided"));
    }
    if let Some(arg) = args.iter().find(|arg| arg.chars().any(char::is_control)) {
        anyhow::bail!("SketchyBar argument contains control characters: {arg:?}");
    }
    Ok(args)
}

fn send_message_action(
    state_dir: Option<PathBuf>,
    cli_args: Vec<String>,
    context: &ActionContext,
) -> Result<()> {
    let action_args = context.args::<SendMessageActionArgs>()?;
    let resolved_state_dir = action_args.state_dir.or(state_dir);
    let cache = action_args.cache_key.zip(action_args.cache_value);
    let sketchybar_args = resolve_message_args(cli_args, action_args.args)?;
    let client = SketchybarMachClient::from_env();
    send_cached_message(
        &CachedMessageRequest {
            state_dir: resolved_state_dir,
            args: &sketchybar_args,
            cache: cache.as_ref(),
            bar_name: client.bar_name(),
        },
        &client,
        &MachEndpointProbe,
    )
    .map_err(anyhow::Error::from)?;
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
        .emit(EVENT_WORKSPACE_CLICKED)
        .capability(CAPABILITY_UI_WRITE)
        .action(
            ACTION_SEND_MESSAGE,
            RegistrationAction::new().capability(CAPABILITY_UI_WRITE),
        )
}
