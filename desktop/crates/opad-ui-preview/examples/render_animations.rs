//! Render frames of the built-in result animations over the idle screen:
//! cargo run -p opad-ui-preview --example render_animations -- <out_dir> [grade]

use opad_model::result_anim::BUILTINS;
use opad_ui_preview::{default_layout, encode_png, render_animation, set_values, SCREEN_IDLE};

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let grade = std::env::args().nth(2).unwrap_or_else(|| "S".into());

    let sample = opad_ui_preview::sample_values();
    set_values(sample.iter().map(|(s, v)| (*s, v)));
    let layout = default_layout(SCREEN_IDLE).unwrap();

    for (id, name) in BUILTINS {
        for t in [100, 250, 450, 1200, 2400] {
            let rgba = render_animation(&layout, *id as u8, &grade, t).expect("layout renders");
            let path = format!(
                "{}/anim-{}-{}-{:04}.png",
                out_dir,
                name.to_lowercase(),
                grade,
                t
            );
            std::fs::write(&path, encode_png(&rgba, 1)).unwrap();
            println!("wrote {}", path);
        }
    }
}
