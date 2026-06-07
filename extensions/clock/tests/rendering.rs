use chrono::{Local, TimeZone};
use spindle_clock_extension::{build_clock_message, format_clock_label};

#[test]
fn clock_label_matches_previous_shell_format() -> anyhow::Result<()> {
    let timestamp = Local
        .with_ymd_and_hms(2026, 6, 8, 9, 4, 5)
        .single()
        .ok_or_else(|| anyhow::anyhow!("test timestamp is ambiguous in local timezone"))?;

    assert_eq!(format_clock_label(timestamp), "06/08 Mon 09:04:05");
    Ok(())
}

#[test]
fn clock_message_sets_target_item_label() -> anyhow::Result<()> {
    let timestamp = Local
        .with_ymd_and_hms(2026, 6, 8, 9, 4, 5)
        .single()
        .ok_or_else(|| anyhow::anyhow!("test timestamp is ambiguous in local timezone"))?;

    let request = build_clock_message("clock", timestamp);

    assert_eq!(
        request.args(),
        ["--set", "clock", "label=06/08 Mon 09:04:05"]
    );
    Ok(())
}
