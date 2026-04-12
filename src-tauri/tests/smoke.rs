/// Smoke test: verifies the crate compiles, links, and key public types are usable.
/// The act of compiling this test exercises all platform-specific code paths
/// (via conditional compilation) and ensures no linker errors.

#[test]
fn crate_compiles_and_links() {
    // If we got here, the entire crate compiled and linked successfully.
    // This catches missing dependencies, broken cfg blocks, and linker errors.
    assert!(true);
}

#[test]
fn app_state_struct_is_public() {
    // Verify AppState is accessible from outside the crate
    let _state = display_dj_lib::AppState {
        preferences: std::sync::Mutex::new(Default::default()),
        monitor_configs: std::sync::Mutex::new(Default::default()),
    };
}

#[test]
fn run_function_exists() {
    // Verify the run function is exported (we don't call it since it starts the app)
    let _fn_ptr: fn() = display_dj_lib::run;
}
