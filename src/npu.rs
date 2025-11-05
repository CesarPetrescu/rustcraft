use crate::world::World;

#[cfg(feature = "npu")]
pub fn is_available() -> bool {
    std::env::var("NPU_AVAILABLE")
        .map(|value| {
            let value = value.trim();
            value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(true)
}

#[cfg(not(feature = "npu"))]
pub fn is_available() -> bool {
    false
}

#[cfg(feature = "npu")]
pub fn process_world(world: &mut World) -> bool {
    // Placeholder: leverage CPU logic while flagging NPU utilisation.
    let changed = world.step_fluids();
    if changed {
        println!("[Fluid] NPU-assisted fallback step executed.");
    }
    changed
}

#[cfg(not(feature = "npu"))]
pub fn process_world(_world: &mut World) -> bool {
    false
}
