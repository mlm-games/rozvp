fn main() -> anyhow::Result<()> {
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    {
        rozvp::pilot::runner::desktop_main()
    }

    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    {
        // Mobile/web enter through android_main/wasm_start in the lib.
        Ok(())
    }
}
