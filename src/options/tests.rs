use super::*;
use cranelift::codegen::settings::OptLevel;

#[test]
fn default_profile_is_unoptimized_and_verified() {
    let options = BuildOptions::default();
    let flags = options.flags().unwrap();
    assert_eq!(options.profile, Profile::Debug);
    assert_eq!(options.cpu, Cpu::Generic);
    assert_eq!(flags.opt_level(), OptLevel::None);
    assert!(flags.enable_verifier());
    assert!(flags.regalloc_checker());
    assert!(flags.preserve_frame_pointers());
    assert!(!flags.enable_alias_analysis());
}

#[test]
fn release_enables_optimization_and_reduces_compilation_checks() {
    let options = BuildOptions {
        profile: Profile::Release,
        ..BuildOptions::default()
    };
    let flags = options.flags().unwrap();
    assert_eq!(flags.opt_level(), OptLevel::Speed);
    assert!(flags.enable_alias_analysis());
    assert!(!flags.enable_verifier());
    assert!(!flags.regalloc_checker());
    assert!(!flags.preserve_frame_pointers());
}

#[test]
fn explicit_optimization_overrides_either_profile() {
    for profile in [Profile::Debug, Profile::Release] {
        for (optimization, expected) in [
            (Optimization::None, OptLevel::None),
            (Optimization::Speed, OptLevel::Speed),
            (Optimization::Size, OptLevel::SpeedAndSize),
        ] {
            let options = BuildOptions {
                profile,
                optimization: Some(optimization),
                ..BuildOptions::default()
            };
            assert_eq!(options.flags().unwrap().opt_level(), expected);
        }
    }
}

#[test]
fn release_can_preserve_frames_and_enable_verification() {
    let options = BuildOptions {
        profile: Profile::Release,
        verify: true,
        frame_pointers: true,
        ..BuildOptions::default()
    };
    let flags = options.flags().unwrap();
    assert!(flags.enable_verifier());
    assert!(flags.regalloc_checker());
    assert!(flags.preserve_frame_pointers());
}

#[test]
fn unsupported_settings_are_rejected() {
    for value in ["best", "speed_and_memory", "", "3"] {
        assert!(value.parse::<Optimization>().is_err());
    }
    assert!("avx2".parse::<Cpu>().is_err());
    assert!("asm".parse::<Emit>().is_err());
}

#[test]
fn both_cpu_modes_build_a_host_isa() {
    for cpu in [Cpu::Generic, Cpu::Native] {
        let isa = BuildOptions {
            cpu,
            ..BuildOptions::default()
        }
        .isa()
        .unwrap();
        assert_eq!(isa.triple().to_string(), env!("XCX_TARGET"));
    }
}
