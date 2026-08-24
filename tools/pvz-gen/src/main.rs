use pvz_gen::zombie_file;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anims");
    std::fs::create_dir_all(out_dir)?;
    let out_path = format!("{out_dir}/zombie.ren");
    let ron = renamite_io_ren::save(&zombie_file())?;
    std::fs::write(&out_path, ron)?;
    println!("wrote {out_path}");
    Ok(())
}
