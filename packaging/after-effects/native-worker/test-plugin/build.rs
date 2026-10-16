fn main() {
    for (key, value) in [
        ("PIPL_KIND", "1699105620"),
        ("PIPL_AE_SPEC_VER_MAJOR", "13"),
        ("PIPL_AE_SPEC_VER_MINOR", "28"),
        ("PIPL_AE_RESERVED", "0"),
        ("PIPL_VERSION", "1"),
        ("PIPL_OUTFLAGS", "0"),
        ("PIPL_OUTFLAGS2", "5120"),
        ("PIPL_SUPPORT_URL", "https://github.com/simoescortez1-star/effectcraft"),
        ("PIPL_ENTRYPOINT", "EffectMain"),
        ("PIPL_CATEGORY", "EffectCraft Test"),
        ("PIPL_MATCH_NAME", "org.effectcraft.test.gain"),
        ("PIPL_NAME", "EffectCraft Gain Fixture"),
    ] {
        println!("cargo:rustc-env={key}={value}");
    }
}
