use std::process::Command;

use spindle_extension_sdk::ExtensionRegistration;

const SURFACE_CONTEXT: &str = r#"{
  "id": "clock",
  "events": [
    {
      "type": "clock.sketchybar.message.requested",
      "source_extension": "clock"
    }
  ],
  "actions": [
    {
      "name": "sketchybar.message.send",
      "extension": "sketchybar",
      "capabilities": ["sketchybar.ui.write"]
    }
  ],
  "capabilities": ["sketchybar.ui.write"]
}"#;

#[test]
fn cli_render_uses_registered_surface_before_emitting_message_request() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_spindle-clock"))
        .arg("render")
        .arg("--extension-context")
        .arg(SURFACE_CONTEXT)
        .arg("--item")
        .arg("clock")
        .output()?;

    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains(r#""type":"clock.sketchybar.message.requested""#));
    assert!(stdout.contains(r#""args":["--set","clock","label="#));
    Ok(())
}

#[test]
fn cli_render_accepts_name_as_item_alias() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_spindle-clock"))
        .arg("render")
        .arg("--extension-context")
        .arg(SURFACE_CONTEXT)
        .arg("--name")
        .arg("clock")
        .output()?;

    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains(r#""args":["--set","clock","label="#));
    Ok(())
}

#[test]
fn cli_render_rejects_missing_registered_output_action() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_spindle-clock"))
        .arg("render")
        .arg("--extension-context")
        .arg(
            r#"{
              "id": "clock",
              "events": [
                {
                  "type": "clock.sketchybar.message.requested",
                  "source_extension": "clock"
                }
              ],
              "actions": [],
              "capabilities": []
            }"#,
        )
        .arg("--item")
        .arg("clock")
        .output()?;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("required action is not available: sketchybar.message.send")
    );
    Ok(())
}

#[test]
fn cli_register_routes_clock_output_to_sketchybar() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_spindle-clock"))
        .arg("register")
        .output()?;

    assert!(
        output.status.success(),
        "register failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    let registration = serde_json::from_str::<ExtensionRegistration>(&stdout)?;

    assert!(
        registration
            .produces
            .contains(&String::from("clock.sketchybar.message.requested"))
    );
    assert!(registration.actions.contains_key("clock.render"));
    let route = registration
        .routes
        .iter()
        .find(|route| route.event == "clock.sketchybar.message.requested")
        .ok_or_else(|| anyhow::anyhow!("missing clock output route"))?;
    assert_eq!(route.source.as_deref(), Some("clock"));
    assert_eq!(route.action, "sketchybar.message.send");
    assert_eq!(route.capabilities, ["sketchybar.ui.write"]);
    Ok(())
}
