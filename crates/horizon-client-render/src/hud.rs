//! Horizon CPU HUD data and the desktop WGPU presenter.

pub use horizon_client_hud::*;

pub mod cit_glint;
mod desktop;
mod pack_text;
pub use desktop::HudRenderer;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    fn write_strip(path: &Path, width: u32, height: u32, frames: &[[u8; 4]], mcmeta: Option<&str>) {
        let file = std::io::BufWriter::new(fs::File::create(path).unwrap());
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for frame in frames {
            let rows = height / frames.len() as u32;
            pixels.extend((0..width * rows).flat_map(|_| *frame));
        }
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
        if let Some(mcmeta) = mcmeta {
            let meta = path.with_extension("png.mcmeta");
            fs::write(meta, mcmeta).unwrap();
        }
    }

    fn texel_at(atlas: &HudAtlas, rect: Rect, x: u32, y: u32) -> [u8; 4] {
        let px = (rect.u0 * atlas.width as f32).round() as u32 + x;
        let py = (rect.v0 * atlas.height as f32).round() as u32 + y;
        let at = ((py * atlas.width + px) * 4) as usize;
        atlas.pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn content_mod_effect_icons_pack_under_their_namespace() {
        let root =
            std::env::temp_dir().join(format!("horizon-atlas-effect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let icons = root.join("assets/gems/textures/mob_effect");
        fs::create_dir_all(&icons).unwrap();
        write_strip(&icons.join("glow.png"), 18, 18, &[[0, 0, 200, 255]], None);
        fs::write(
            root.join(horizon_items::content_namespaces::MARKER),
            br#"["gems"]"#,
        )
        .unwrap();
        let atlas = HudAtlas::build(&root).unwrap();
        let rect = atlas.sprites[horizon_client_core::potions::sprite("gems:glow").as_str()];
        assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 0, 200, 255]);
        // The HUD and the inventory draw it from the effect's id, as vanilla's.
        let effect = crate::effects::EffectInstance::new("gems:glow", 600, 0);
        let icon = crate::hud_overlays::EffectIcon::from(&effect);
        let draws = crate::hud_overlays::effects(320, false, &[icon]);
        let mut batch = HudBatch::new([320, 240]);
        crate::hud_layout::paint(&mut batch, &atlas, &draws);
        assert!(batch.vertices.iter().any(|v| v.uv == [rect.u0, rect.v0]));
        let panels = crate::inventory_effects::panels(
            320,
            [72, 37],
            176,
            &[effect],
            20.0,
            &|_| None,
            &|_| 60,
        );
        batch.vertices.clear();
        crate::inventory_effects::paint(&mut batch, &atlas, &panels, [0, 0]);
        assert!(batch.vertices.iter().any(|v| v.uv == [rect.u0, rect.v0]));
        // An effect without an icon draws the missing sprite.
        let missing = atlas.sprites[MISSING_SPRITE];
        assert_eq!(texel_at(&atlas, missing, 0, 0), [0, 0, 0, 255]);
        assert_eq!(texel_at(&atlas, missing, 8, 0), [248, 0, 248, 255]);
        let effect = crate::effects::EffectInstance::new("gems:none", 600, 0);
        let draws = crate::hud_overlays::effects(320, false, &[(&effect).into()]);
        batch.vertices.clear();
        crate::hud_layout::paint(&mut batch, &atlas, &draws);
        assert!(
            batch
                .vertices
                .iter()
                .any(|v| v.uv == [missing.u0, missing.v0])
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn effect_icons_are_painted_from_the_mob_effect_directory() {
        use crate::{effects::EffectInstance, hud_layout, hud_overlays, inventory_effects};
        // A synthetic resource tree exercises loading and painting together.
        // No Minecraft installation or GPU is needed for this regression test.
        let root =
            std::env::temp_dir().join(format!("horizon-effect-icons-{}", std::process::id()));
        for (name, size) in [
            ("mob_effect/levitation", 18),
            ("gui/sprites/hud/effect_background", 24),
            ("gui/sprites/container/inventory/effect_background", 32),
        ] {
            let path = root.join(format!("assets/minecraft/textures/{name}.png"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), size, size);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&vec![255; (size * size * 4) as usize])
                .unwrap();
        }
        let atlas = HudAtlas::build(&root).unwrap();
        let effect = EffectInstance::new("levitation", -1, 0);
        let icon = hud_overlays::EffectIcon::from(&effect);
        let draws = hud_overlays::effects(320, false, &[icon]);
        let mut batch = HudBatch::new([320, 240]);
        hud_layout::paint(&mut batch, &atlas, &draws);
        // Both the background and the 18x18 effect icon must reach the batch.
        assert_eq!(batch.vertices.len(), 12);
        let rect = atlas.sprites["mob_effect/levitation"];
        assert_eq!(batch.vertices[6].pos, [298.0, 30.0]);
        assert_eq!(batch.vertices[6].uv, [rect.u0, rect.v0]);
        assert_eq!(batch.vertices[8].uv, [rect.u1, rect.v1]);

        let panels = inventory_effects::panels(
            320,
            [72, 37],
            176,
            std::slice::from_ref(&effect),
            20.0,
            &|_| None,
            &|_| 60,
        );
        batch.vertices.clear();
        let tooltip = inventory_effects::paint(&mut batch, &atlas, &panels, [257, 44]);
        assert_eq!(tooltip.unwrap()[0].0, "levitation");
        assert_eq!(batch.vertices.len(), 24);
        assert_eq!(batch.vertices[18].pos, [257.0, 44.0]);
        assert_eq!(batch.vertices[18].uv, [rect.u0, rect.v0]);
        assert_eq!(batch.vertices[20].uv, [rect.u1, rect.v1]);

        // Wide panels retain the four-pixel border instead of stretching it.
        let panels =
            inventory_effects::panels(480, [72, 37], 176, &[effect], 20.0, &|_| None, &|_| 60);
        batch.vertices.clear();
        assert!(inventory_effects::paint(&mut batch, &atlas, &panels, [0, 0]).is_none());
        assert!(batch.vertices.iter().any(|v| v.color == 0xff808080));
        assert_eq!(batch.vertices[0].pos, [250.0, 37.0]);
        assert_eq!(batch.vertices[1].pos, [254.0, 37.0]);
        let background = atlas.sprites["gui/sprites/container/inventory/effect_background"];
        assert!(
            (batch.vertices[1].uv[0] - background.u0 - (background.u1 - background.u0) / 8.0).abs()
                < 1e-6
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hud_shader_validates_both_fragment_entries() {
        let module = naga::front::wgsl::parse_str(include_str!("hud.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }
}
