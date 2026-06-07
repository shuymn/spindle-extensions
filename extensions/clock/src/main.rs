#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use serde_json::json;
use spindle_clock_extension::{OUTPUT_EVENT, build_clock_message};
use spindle_extension_sdk::{
    ActionContext, ActionHandler, ActionInvocation, ActionOutput, ExtensionContext,
    ExtensionRegistration, RegistrationAction, RegistrationRoute, serve_stdio_jsonl_actions,
};

const ACTION_RENDER_CLOCK: &str = "clock.render";
const OUTPUT_ACTION: &str = "sketchybar.message.send";
const SKETCHYBAR_UI_WRITE_CAPABILITY: &str = "sketchybar.ui.write";

fn main() -> Result<()> {
    Cli::parse().run()
}

#[derive(Debug, Parser)]
#[command(name = "spindle-clock", version)]
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
            CliCommand::Render(args) => {
                let context = cli_context(
                    ACTION_RENDER_CLOCK,
                    json!({ "item": args.item, "name": args.name }),
                    args.extension_context,
                )?;
                print_output(&render_clock_action(&context)?)?;
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
    ACTION_RENDER_CLOCK,
    render_clock_host_action,
)];

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Register this extension's spindle surface.
    Register,
    /// Project the current time into a generic `SketchyBar` message request.
    Render(RenderArgs),
}

#[derive(Debug, Args)]
struct RenderArgs {
    #[arg(long, value_name = "ITEM")]
    item: Option<String>,
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    #[arg(long, value_name = "JSON")]
    extension_context: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RenderClockActionArgs {
    item: Option<String>,
    name: Option<String>,
}

fn render_clock_host_action(context: &ActionContext) -> Result<ActionOutput> {
    render_clock_action(context)
}

fn render_clock_action(context: &ActionContext) -> Result<ActionOutput> {
    ensure_output_surface(context.extension())?;
    let action_args = context.args::<RenderClockActionArgs>()?;
    let item = resolve_item(action_args)?;
    let request = build_clock_message(&item, Local::now());
    Ok(ActionOutput::event(request.into_event()))
}

fn resolve_item(action_args: RenderClockActionArgs) -> Result<String> {
    action_args
        .item
        .or(action_args.name)
        .or_else(|| std::env::var("NAME").ok())
        .context("NAME is not set and neither item nor name were provided")
}

fn ensure_output_surface(extension: Option<&ExtensionContext>) -> Result<()> {
    let Some(extension) = extension else {
        return Ok(());
    };

    if !extension.has_event(OUTPUT_EVENT) {
        anyhow::bail!("required event is not available: {OUTPUT_EVENT}");
    }
    if !extension.has_action(OUTPUT_ACTION) {
        anyhow::bail!("required action is not available: {OUTPUT_ACTION}");
    }
    Ok(())
}

fn print_output(output: &ActionOutput) -> Result<()> {
    println!("{}", output.to_json_string()?);
    Ok(())
}

fn print_registration(registration: &ExtensionRegistration) -> Result<()> {
    println!("{}", registration.to_json_string()?);
    Ok(())
}

fn cli_context(
    action: &str,
    args: serde_json::Value,
    extension_context: Option<String>,
) -> Result<ActionContext> {
    let extension = extension_context
        .map(|raw| serde_json::from_str::<ExtensionContext>(&raw))
        .transpose()?;
    Ok(ActionContext::from_invocation(
        ActionInvocation::new(action, args).with_extension(extension),
    ))
}

fn registration() -> ExtensionRegistration {
    ExtensionRegistration::new()
        .produce(OUTPUT_EVENT)
        .action(ACTION_RENDER_CLOCK, RegistrationAction::new())
        .route(
            RegistrationRoute::new(OUTPUT_EVENT, OUTPUT_ACTION)
                .source("clock")
                .capability(SKETCHYBAR_UI_WRITE_CAPABILITY),
        )
}
