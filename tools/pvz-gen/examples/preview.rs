use renamite_behavior_common::ViewTransform;
use renamite_player::Player;
use renamite_render_bridge::SceneRenderer;
use renamite_render_offscreen::OffscreenRenderer;

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

fn snap(
    bridge: &mut SceneRenderer,
    gpu: &mut OffscreenRenderer,
    player: &Player,
    view: &ViewTransform,
    path: &str,
) {
    let prepared = bridge.prepare(player.scene(), view);
    let mut repose = repose_core_scene();
    bridge.append_repose_scene(&prepared, &mut repose);
    let png = gpu.render_png(&repose, Some([1.0, 1.0, 1.0, 1.0])).unwrap();
    std::fs::write(path, png).unwrap();
    println!("wrote {path}");
}

fn repose_core_scene() -> repose_core::Scene {
    repose_core::Scene::default()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ren_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anims/zombie.ren");
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../out/preview");
    std::fs::create_dir_all(out_dir)?;

    let project = renamite_io_ren::open(&std::fs::read_to_string(ren_path)?)?;
    let comp = project.document.compositions[project.document.main].size;
    let view = contain_view(comp, comp.0, comp.1);

    let mut bridge = SceneRenderer::new();
    let mut gpu = OffscreenRenderer::new_blocking(comp.0, comp.1, 4)?;
    gpu.sync_document_images(&project.document)?;

    let mut player = Player::new(project.clone())?;
    player.set_bool("eating", true);
    for i in 0..30 {
        player.tick(1.0 / 60.0);
        if i == 8 {
            snap(
                &mut bridge,
                &mut gpu,
                &player,
                &view,
                &format!("{out_dir}/eat_open.png"),
            );
        }
        if i == 14 {
            snap(
                &mut bridge,
                &mut gpu,
                &player,
                &view,
                &format!("{out_dir}/eat_closed.png"),
            );
        }
    }

    let mut player = Player::new(project)?;
    player.fire("die");
    for i in 0..45 {
        player.tick(1.0 / 60.0);
        if i == 8 {
            snap(
                &mut bridge,
                &mut gpu,
                &player,
                &view,
                &format!("{out_dir}/fall_arm_off.png"),
            );
        }
        if i == 34 {
            snap(
                &mut bridge,
                &mut gpu,
                &player,
                &view,
                &format!("{out_dir}/fall_down.png"),
            );
        }
    }

    Ok(())
}
