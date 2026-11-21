fn main() -> Result<(), std::env::VarError> {
    let target = std::env::var("TARGET")?;
    println!("cargo:rustc-env=SUNRISE_MOD_SOURCE_TARGET={target}");
    // The standalone SDK tool has no Cargo build-script environment at runtime.
    for key in [
        "CARGO_CFG_TARGET_ABI",
        "CARGO_CFG_TARGET_ENV",
        "CARGO_CFG_TARGET_FAMILY",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_CFG_TARGET_HAS_ATOMIC",
        "CARGO_CFG_TARGET_VENDOR",
    ] {
        println!(
            "cargo:rustc-env={key}={}",
            std::env::var(key).unwrap_or_default()
        );
    }
    Ok(())
}
