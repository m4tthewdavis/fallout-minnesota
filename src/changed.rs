//! Writes that only happen when something actually changes.
//!
//! `Assets::get_mut` marks a material modified whether or not anything is
//! written, and a modified material is prepared again and every mesh using it
//! re-queued (in a blizzard that is all 900 snowflakes). Likewise a UI node
//! touched through `Mut` is laid out again and a touched `Text` re-shaped.
//! Per-frame code reads first and only writes a value that differs.

use bevy::prelude::*;

/// Sets a material's base colour if it differs. Returns true if it wrote.
pub fn set_base_color(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, color: Color) -> bool {
    write_if(materials, handle, |m| m.base_color != color, |m| m.base_color = color)
}

/// Sets a material's emissive colour if it differs. Returns true if it wrote.
pub fn set_emissive(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, emissive: LinearRgba) -> bool {
    write_if(materials, handle, |m| m.emissive != emissive, |m| m.emissive = emissive)
}

/// Sets a material's texture transform if it differs. Returns true if it wrote.
pub fn set_uv_transform(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, uv: bevy::math::Affine2) -> bool {
    write_if(materials, handle, |m| m.uv_transform != uv, |m| m.uv_transform = uv)
}

/// Runs `write` on the material only if `differs` says it needs it (and it exists).
pub fn write_if(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    differs: impl FnOnce(&StandardMaterial) -> bool,
    write: impl FnOnce(&mut StandardMaterial),
) -> bool {
    if !materials.get(handle).is_some_and(differs) {
        return false;
    }
    if let Some(m) = materials.get_mut(handle) {
        write(m);
    }
    true
}

/// Sets one `Val` field of a node only if it differs (a touched node is laid out again).
pub fn set_val(node: &mut Mut<Node>, field: impl Fn(&mut Node) -> &mut Val, v: Val) {
    if *field(node.bypass_change_detection()) != v {
        *field(node) = v;
    }
}

/// Shows or hides a node (`Display::Flex` or `None`) only if that changes.
pub fn set_display(node: &mut Mut<Node>, show: bool) {
    let want = if show { Display::Flex } else { Display::None };
    if node.display != want {
        node.display = want;
    }
}

/// Swaps an image only if it's a different one.
pub fn set_image(img: &mut Mut<ImageNode>, h: &Handle<Image>) {
    if img.image != *h {
        img.image = h.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_only_when_the_value_changes() {
        let mut mats = Assets::<StandardMaterial>::default();
        let h = mats.add(StandardMaterial { base_color: Color::WHITE, ..default() });
        assert!(!set_base_color(&mut mats, &h, Color::WHITE));
        assert!(set_base_color(&mut mats, &h, Color::BLACK));
        assert_eq!(mats.get(&h).unwrap().base_color, Color::BLACK);
        assert!(!set_base_color(&mut mats, &h, Color::BLACK));
        // A different representation of a colour counts as a change (as before: it was always written).
        assert!(set_base_color(&mut mats, &h, Color::Srgba(Srgba::BLACK)));
        let e = LinearRgba::rgb(0.5, 1.0, 0.2);
        assert!(set_emissive(&mut mats, &h, e));
        assert!(!set_emissive(&mut mats, &h, e));
        assert_eq!(mats.get(&h).unwrap().emissive, e);
    }

    #[test]
    fn a_missing_material_is_left_alone() {
        let mut mats = Assets::<StandardMaterial>::default();
        let h = mats.add(StandardMaterial::default());
        mats.remove(&h);
        assert!(!set_base_color(&mut mats, &h, Color::BLACK));
    }
}
