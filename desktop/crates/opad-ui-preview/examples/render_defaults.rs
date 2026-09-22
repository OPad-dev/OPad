//! Render the built-in screens with sample data: cargo run -p opad-ui-preview --example render_defaults -- <out_dir>

use opad_ui_preview::{
    default_layout, encode_png, render, set_values, SCREEN_IDLE, SCREEN_PLAYING,
};

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());

    let sample = opad_ui_preview::sample_values();
    set_values(sample.iter().map(|(s, v)| (*s, v)));

    for (screen, name) in [(SCREEN_IDLE, "idle"), (SCREEN_PLAYING, "playing")] {
        let layout = default_layout(screen).unwrap();
        let rgba = render(&layout).expect("layout renders");
        let path = format!("{}/opad-{}.png", out_dir, name);
        std::fs::write(&path, encode_png(&rgba, 2)).unwrap();
        println!("wrote {}", path);
    }
}
