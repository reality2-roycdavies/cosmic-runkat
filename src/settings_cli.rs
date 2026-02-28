//! CLI settings protocol for cosmic-applet-settings hub integration.
//!
//! Supports `--settings-describe`, `--settings-set`, and `--settings-action`.

use crate::config::{AnimationSource, Config};

/// Output the settings schema as JSON to stdout.
pub fn describe() {
    let config = Config::load();

    // Determine dynamic slider range/unit based on animation source
    let (threshold_min, threshold_max, threshold_step, threshold_unit, threshold_value) =
        match config.animation_source {
            AnimationSource::CpuUsage => (0.0, 30.0, 1.0, "%", config.sleep_threshold_cpu as f64),
            AnimationSource::Frequency => {
                (0.0, 10000.0, 100.0, " MHz", config.sleep_threshold_freq as f64)
            }
            AnimationSource::Temperature => {
                (0.0, 150.0, 1.0, "°C", config.sleep_threshold_temp as f64)
            }
        };

    let source_value = match config.animation_source {
        AnimationSource::CpuUsage => "CpuUsage",
        AnimationSource::Frequency => "Frequency",
        AnimationSource::Temperature => "Temperature",
    };

    let schema = serde_json::json!({
        "title": "RunKat Settings",
        "description": "The cat runs faster based on the selected metric.",
        "sections": [
            {
                "title": "Behavior",
                "items": [
                    {
                        "type": "select",
                        "key": "animation_source",
                        "label": "Monitor",
                        "value": source_value,
                        "options": [
                            {"value": "CpuUsage", "label": "CPU Usage"},
                            {"value": "Frequency", "label": "CPU Frequency"},
                            {"value": "Temperature", "label": "CPU Temperature"}
                        ]
                    },
                    {
                        "type": "slider",
                        "key": "sleep_threshold",
                        "label": "Sleep Below",
                        "value": threshold_value,
                        "min": threshold_min,
                        "max": threshold_max,
                        "step": threshold_step,
                        "unit": threshold_unit
                    },
                    {
                        "type": "toggle",
                        "key": "show_percentage",
                        "label": "Show % on Icon",
                        "value": config.show_percentage,
                        "visible_when": {"key": "animation_source", "equals": "CpuUsage"}
                    }
                ]
            }
        ],
        "actions": [
            {"id": "reset", "label": "Reset to Defaults", "style": "destructive"}
        ]
    });

    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

/// Set a single setting by key. Prints JSON result to stdout.
pub fn set(key: &str, value: &str) {
    let mut config = Config::load();

    let result = match key {
        "animation_source" => {
            let parsed: Result<String, _> = serde_json::from_str(value);
            match parsed.as_deref() {
                Ok("CpuUsage") => {
                    config.animation_source = AnimationSource::CpuUsage;
                    Ok("Updated animation source")
                }
                Ok("Frequency") => {
                    config.animation_source = AnimationSource::Frequency;
                    Ok("Updated animation source")
                }
                Ok("Temperature") => {
                    config.animation_source = AnimationSource::Temperature;
                    Ok("Updated animation source")
                }
                _ => Err(format!("Invalid animation_source: {value}")),
            }
        }
        "sleep_threshold" => match serde_json::from_str::<f64>(value) {
            Ok(v) => {
                config.set_current_threshold(v as f32);
                Ok("Updated sleep threshold")
            }
            Err(e) => Err(format!("Invalid number: {e}")),
        },
        "show_percentage" => match serde_json::from_str::<bool>(value) {
            Ok(v) => {
                config.show_percentage = v;
                Ok("Updated show percentage")
            }
            Err(e) => Err(format!("Invalid boolean: {e}")),
        },
        _ => Err(format!("Unknown key: {key}")),
    };

    match result {
        Ok(msg) => {
            if let Err(e) = config.save() {
                print_response(false, &format!("Save failed: {e}"));
            } else {
                print_response(true, msg);
            }
        }
        Err(msg) => print_response(false, &msg),
    }
}

/// Execute an action by ID. Prints JSON result to stdout.
pub fn action(id: &str) {
    match id {
        "reset" => {
            let config = Config::default();
            match config.save() {
                Ok(()) => print_response(true, "Reset to defaults"),
                Err(e) => print_response(false, &format!("Reset failed: {e}")),
            }
        }
        _ => print_response(false, &format!("Unknown action: {id}")),
    }
}

fn print_response(ok: bool, message: &str) {
    let resp = serde_json::json!({"ok": ok, "message": message});
    println!("{}", resp);
}
