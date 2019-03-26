use vki::{Extent3D, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags};

pub mod support;

#[test]
fn create_texture() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::Texture2D,
            format: TextureFormat::R8G8B8A8Unorm,
        };

        let _texture = device.create_texture(descriptor)?;

        Ok(instance)
    });
}