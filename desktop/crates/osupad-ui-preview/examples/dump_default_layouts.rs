//! Print the built-in layouts as designer JSON: cargo run -p osupad-ui-preview --example dump_default_layouts

fn main() {
    for screen in osupad_layout::Screen::ALL {
        println!(
            "{}",
            serde_json::json!({ "screen": screen, "layout": osupad_ui_preview::default_model(*screen) })
        );
    }
}
