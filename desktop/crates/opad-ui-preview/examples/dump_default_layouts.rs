//! Print the built-in layouts as designer JSON: cargo run -p opad-ui-preview --example dump_default_layouts

fn main() {
    for screen in opad_layout::Screen::ALL {
        println!(
            "{}",
            serde_json::json!({ "screen": screen, "layout": opad_ui_preview::default_model(*screen) })
        );
    }
}
