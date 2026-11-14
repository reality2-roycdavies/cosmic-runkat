//! CLI settings protocol for cosmic-applet-settings hub integration.
//!
//! Supports `--settings-describe`, `--settings-set`, and `--settings-action`.

use crate::config::{AnimationSource, Config, Runner};

/// Output the settings schema as JSON to stdout.
pub fn describe() {
    let schema = schema(&Config::load());
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

/// Build the settings schema for the given config.  The hub reads it again
/// after every change, so it can depend on the current values.
fn schema(config: &Config) -> serde_json::Value {
    // Determine dynamic slider range/unit based on animation source
    let (threshold_min, threshold_max, threshold_step, threshold_unit, threshold_value) =
        match config.animation_source {
            AnimationSource::CpuUsage => (0.0, 100.0, 1.0, "%", config.sleep_threshold_cpu as f64),
            AnimationSource::Frequency => {
                (0.0, 10000.0, 100.0, " MHz", config.sleep_threshold_freq as f64)
            }
            AnimationSource::Temperature => {
                (0.0, 150.0, 1.0, "°C", config.sleep_threshold_temp as f64)
            }
            AnimationSource::Memory => (0.0, 100.0, 1.0, "%", config.sleep_threshold_mem as f64),
            AnimationSource::Gpu => (0.0, 100.0, 1.0, "%", config.sleep_threshold_gpu as f64),
        };

    let source_value = SOURCE_KEYS[AnimationSource::ALL
        .iter()
        .position(|&s| s == config.animation_source)
        .unwrap_or(0)];
    let options = |keys: &[&str], names: &[&str]| -> Vec<serde_json::Value> {
        keys.iter()
            .zip(names)
            .map(|(k, n)| serde_json::json!({"value": k, "label": n}))
            .collect()
    };

    let mut items = vec![
        serde_json::json!({
            "type": "select",
            "key": "runner",
            "label": "Runner",
            "value": RUNNER_KEYS[Runner::ALL.iter().position(|&r| r == config.runner).unwrap_or(0)],
            "options": options(RUNNER_KEYS, Runner::NAMES)
        }),
        serde_json::json!({
            "type": "select",
            "key": "animation_source",
            "label": "Monitor",
            "value": source_value,
            "options": options(SOURCE_KEYS, AnimationSource::NAMES)
        }),
        serde_json::json!({
            "type": "slider",
            "key": "sleep_threshold",
            "label": "Sleep Below",
            "value": threshold_value,
            "min": threshold_min,
            "max": threshold_max,
            "step": threshold_step,
            "unit": threshold_unit
        }),
    ];

    // The % label is only drawn for percentage sources, so only offer the
    // toggle for those.  (visible_when can match just one value, so the
    // item is left out instead.)
    if config.animation_source.is_percentage() {
        items.push(serde_json::json!({
            "type": "toggle",
            "key": "show_percentage",
            "label": "Show % on Icon",
            "value": config.show_percentage
        }));
    }

    serde_json::json!({
        "title": "RunKat Settings",
        "description": "The cat runs faster based on the selected metric.",
        "sections": [
            {
                "title": "Behavior",
                "items": items
            }
        ],
        "actions": [
            {"id": "reset", "label": "Reset to Defaults", "style": "destructive"}
        ]
    })
}

/// Values used for select options, in the same order as the `ALL` arrays
const SOURCE_KEYS: &[&str] = &["CpuUsage", "Frequency", "Temperature", "Memory", "Gpu"];
const RUNNER_KEYS: &[&str] = &["Cat", "Horse", "Parrot"];

/// Look up a JSON string value in a key list, returning its index
fn select_index(keys: &[&str], value: &str) -> Option<usize> {
    let parsed: String = serde_json::from_str(value).ok()?;
    keys.iter().position(|k| *k == parsed)
}

/// Set a single setting by key. Prints JSON result to stdout.
pub fn set(key: &str, value: &str) {
    let mut config = Config::load();

    let result = match key {
        "animation_source" => match select_index(SOURCE_KEYS, value) {
            Some(i) => {
                config.animation_source = AnimationSource::ALL[i];
                Ok("Updated animation source")
            }
            None => Err(format!("Invalid animation_source: {value}")),
        },
        "runner" => match select_index(RUNNER_KEYS, value) {
            Some(i) => {
                config.runner = Runner::ALL[i];
                Ok("Updated runner")
            }
            None => Err(format!("Invalid runner: {value}")),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn has_percentage_toggle(source: AnimationSource) -> bool {
        let config = Config { animation_source: source, ..Config::default() };
        schema(&config)["sections"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["key"] == "show_percentage")
    }

    #[test]
    fn percentage_toggle_only_for_percentage_sources() {
        assert!(has_percentage_toggle(AnimationSource::CpuUsage));
        assert!(has_percentage_toggle(AnimationSource::Memory));
        assert!(has_percentage_toggle(AnimationSource::Gpu));
        assert!(!has_percentage_toggle(AnimationSource::Frequency));
        assert!(!has_percentage_toggle(AnimationSource::Temperature));
    }
}
