use pvz_gen::zombie_file;
use renamite_behavior_common::ViewTransform;
use renamite_machine::Clip;
use renamite_model::{evaluate_with, Overrides};
use renamite_render_bridge::SceneRenderer;
use renamite_render_offscreen::OffscreenRenderer;

const OUT_W: u32 = 128;
const OUT_H: u32 = 160;
const COLS: u32 = 8;
const SAMPLE_STEP: i64 = 2;

struct StateSpec {
    name: &'static str,
    clip: &'static str,
    looping: bool,
}

const STATES: [StateSpec; 3] = [
    StateSpec { name: "walk", clip: "walk", looping: true },
    StateSpec { name: "eat", clip: "eat", looping: true },
    StateSpec { name: "fall", clip: "fall", looping: false },
];

fn contain_view(comp: (u32, u32), out_w: u32, out_h: u32) -> ViewTransform {
    let (cw, ch) = (comp.0 as f64, comp.1 as f64);
    let scale = (out_w as f64 / cw).min(out_h as f64 / ch);
    ViewTransform {
        scale,
        offset: glam::DVec2::new(
            (out_w as f64 - cw * scale) * 0.5,
            (out_h as f64 - ch * scale) * 0.5,
        ),
    }
}

fn render_frame_rgba(
    bridge: &mut SceneRenderer,
    gpu: &mut OffscreenRenderer,
    file: &renamite_io_ren::RenFile,
    clip: &Clip,
    frame: f64,
    view: &ViewTransform,
) -> image::RgbaImage {
    let mut ov = Overrides::default();
    clip.sample_into(frame, &mut ov.values);
    let scene = evaluate_with(&file.document, file.document.main, 0.0, &ov);

    let prepared = bridge.prepare(&scene, view);
    let mut repose = repose_core::Scene::default();
    bridge.append_repose_scene(&prepared, &mut repose);
    let png = gpu.render_png(&repose, None).expect("offscreen render");

    image::load_from_memory(&png)
        .expect("decode rendered png")
        .to_rgba8()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anims");
    std::fs::create_dir_all(out_dir)?;

    let mut file = zombie_file();
    file.normalize();
    file.garbage_collect();

    let mut gpu = OffscreenRenderer::new_blocking(OUT_W, OUT_H, 4)?;
    gpu.sync_document_images(&file.document)?;
    let mut bridge = SceneRenderer::new();
    let view = contain_view((256, 320), OUT_W, OUT_H);

    let mut frames: Vec<(String, bool, image::RgbaImage)> = Vec::new();
    for state in &STATES {
        let clip = file
            .clips
            .values()
            .find(|c| c.name == state.clip)
            .ok_or("missing clip")?;
        let end = clip.range.1.0;
        let mut f = clip.range.0.0;
        while f < end {
            let img = render_frame_rgba(&mut bridge, &mut gpu, &file, clip, f as f64, &view);
            frames.push((state.name.to_string(), state.looping, img));
            f += SAMPLE_STEP;
        }
        if !state.looping {
            let img =
                render_frame_rgba(&mut bridge, &mut gpu, &file, clip, end as f64, &view);
            frames.push((state.name.to_string(), state.looping, img));
        }
    }

    let total = frames.len() as u32;
    let rows = total.div_ceil(COLS);
    let mut atlas = image::RgbaImage::new(COLS * OUT_W, rows * OUT_H);
    for (i, (_, _, img)) in frames.iter().enumerate() {
        let (col, row) = ((i as u32) % COLS, (i as u32) / COLS);
        image::imageops::replace(
            &mut atlas,
            img,
            (col * OUT_W) as i64,
            (row * OUT_H) as i64,
        );
    }

    let png_path = format!("{out_dir}/zombie_atlas.png");
    atlas.save_with_format(&png_path, image::ImageFormat::Png)?;
    println!("wrote {png_path} ({}x{}, {} frames)", atlas.width(), atlas.height(), total);

    let mut cursor = 0u32;
    println!("// paste into src/game/zombie_anim.rs");
    println!("pub const ATLAS_COLS: u32 = {COLS};");
    println!("pub const FRAME_W: u32 = {OUT_W};");
    println!("pub const FRAME_H: u32 = {OUT_H};");
    for state in &STATES {
        let count = frames.iter().filter(|(n, _, _)| n == state.name).count() as u32;
        println!(
            "pub const {}_START: usize = {};",
            state.name.to_uppercase(),
            cursor
        );
        println!(
            "pub const {}_COUNT: usize = {};",
            state.name.to_uppercase(),
            count
        );
        cursor += count;
    }
    let fps = 60.0 / SAMPLE_STEP as f32;
    println!("pub const ANIM_FPS: f32 = {fps};");
    Ok(())
}
