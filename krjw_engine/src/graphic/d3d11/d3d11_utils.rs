use anyhow::{Context, Result};
use glam::Vec2;
use image::GenericImageView;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

// ═══════════════════════════════════════════════════════════════
// Shader helpers
// ═══════════════════════════════════════════════════════════════

/// Compile HLSL source into shader bytecode.
pub fn compile_shader(source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Result<Vec<u8>> {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    let mut shader_blob = None;
    let mut error_blob = None;

    let hr = unsafe {
        D3DCompile(
            source.as_ptr() as *const _,
            source.len(),
            PCSTR::null(),
            None,
            None,
            entrypoint,
            target,
            0,
            0,
            &mut shader_blob,
            Some(&mut error_blob),
        )
    };

    let blob = shader_blob.ok_or_else(|| {
        let msg = error_blob
            .as_ref()
            .map(|blob| unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    blob.GetBufferPointer() as *const u8,
                    blob.GetBufferSize(),
                ))
                .into_owned()
            })
            .unwrap_or_else(|| format!("D3DCompile returned {:?}", hr));
        anyhow::anyhow!("D3DCompile failed\n{}", msg)
    })?;

    Ok(unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
            .to_vec()
    })
}

/// Compile HLSL and create a vertex shader.
pub fn create_vs(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11VertexShader> {
    let blob = compile_shader(
        hlsl_bytes,
        PCSTR(b"main\0".as_ptr()),
        PCSTR(b"vs_5_0\0".as_ptr()),
    )?;
    let mut shader = None;
    unsafe {
        device
            .CreateVertexShader(&blob, None, Some(&mut shader))
            .context("create_vs failed")?;
    }
    Ok(shader.unwrap())
}

/// Compile HLSL and create a pixel shader.
pub fn create_ps(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11PixelShader> {
    let blob = compile_shader(
        hlsl_bytes,
        PCSTR(b"main\0".as_ptr()),
        PCSTR(b"ps_5_0\0".as_ptr()),
    )?;
    let mut shader = None;
    unsafe {
        device
            .CreatePixelShader(&blob, None, Some(&mut shader))
            .context("create_ps failed")?;
    }
    Ok(shader.unwrap())
}

/// Create an input layout from a vertex shader bytecode blob.
pub fn create_input_layout(
    device: &ID3D11Device,
    desc: &[D3D11_INPUT_ELEMENT_DESC],
    vs_blob: &[u8],
) -> Result<ID3D11InputLayout> {
    let mut layout = None;
    unsafe {
        device
            .CreateInputLayout(desc, vs_blob, Some(&mut layout))
            .context("create_input_layout failed")?;
    }
    Ok(layout.unwrap())
}

// ═══════════════════════════════════════════════════════════════
// Buffer helpers
// ═══════════════════════════════════════════════════════════════

/// Create a dynamic (CPU-writeable) buffer.
pub fn create_dynamic_buffer(
    device: &ID3D11Device,
    byte_width: u32,
    bind_flags: u32,
) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: byte_width,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: bind_flags,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut buffer = None;
    unsafe {
        device
            .CreateBuffer(&desc, None, Some(&mut buffer))
            .context("create_dynamic_buffer failed")?;
    }
    Ok(buffer.unwrap())
}

/// Create an immutable buffer with initial data.
pub fn create_immutable_buffer(
    device: &ID3D11Device,
    data: &[u8],
    bind_flags: u32,
) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: data.len() as u32,
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: bind_flags,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let init = D3D11_SUBRESOURCE_DATA {
        pSysMem: data.as_ptr() as *const _ as *mut _,
        SysMemPitch: 0,
        SysMemSlicePitch: 0,
    };
    let mut buffer = None;
    unsafe {
        device
            .CreateBuffer(&desc, Some(&init), Some(&mut buffer))
            .context("create_immutable_buffer failed")?;
    }
    Ok(buffer.unwrap())
}

/// Create a constant buffer sized for `T`.
pub fn create_constant_buffer<T>(device: &ID3D11Device) -> Result<ID3D11Buffer> {
    create_dynamic_buffer(
        device,
        std::mem::size_of::<T>() as u32,
        D3D11_BIND_CONSTANT_BUFFER.0 as u32,
    )
}

/// Map (DISCARD), write, and unmap a buffer.
pub fn write_buffer<T>(
    context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut mapped = std::mem::zeroed();
        context
            .Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
            .context("write_buffer Map failed")?;
        std::ptr::copy_nonoverlapping(
            data.as_ptr() as *const u8,
            mapped.pData as *mut u8,
            data.len() * std::mem::size_of::<T>(),
        );
        context.Unmap(buffer, 0);
    }
    Ok(())
}

/// View any slice as raw bytes (type-erase helper for buffer creation).
pub fn as_u8_slice<T>(data: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<T>(),
        )
    }
}

// ═══════════════════════════════════════════════════════════════
// Texture helpers
// ═══════════════════════════════════════════════════════════════

/// Create a 2D texture with the given parameters.
#[allow(clippy::too_many_arguments)]
pub fn create_texture_2d(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    bind_flags: u32,
    usage: D3D11_USAGE,
    cpu_access_flags: u32,
    initial_data: Option<(&[u8], u32)>, // (data, row_pitch)
) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: usage,
        BindFlags: bind_flags,
        CPUAccessFlags: cpu_access_flags,
        MiscFlags: 0,
    };
    let subres = initial_data.map(|(data, pitch)| D3D11_SUBRESOURCE_DATA {
        pSysMem: data.as_ptr() as *const _ as *mut _,
        SysMemPitch: pitch,
        SysMemSlicePitch: 0,
    });
    let subres_ptr = subres.as_ref().map(|s| s as *const _);
    let mut texture = None;
    unsafe {
        device
            .CreateTexture2D(&desc, subres_ptr, Some(&mut texture))
            .context("create_texture_2d failed")?;
    }
    Ok(texture.unwrap())
}

/// Create a shader resource view for a 2D texture.
pub fn create_srv(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
    format: DXGI_FORMAT,
) -> Result<ID3D11ShaderResourceView> {
    let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: format,
        ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
            },
        },
    };
    let mut srv = None;
    unsafe {
        device
            .CreateShaderResourceView(texture, Some(&desc), Some(&mut srv))
            .context("create_srv failed")?;
    }
    Ok(srv.unwrap())
}

#[derive(Debug)]
/// Information about a loaded texture.
pub struct TextureInfo {
    pub texture: ID3D11Texture2D,
    pub srv: ID3D11ShaderResourceView,
    pub width: u32,
    pub height: u32,
    pub format: DXGI_FORMAT,
}

impl TextureInfo {
    pub fn size_vec2f(&self) -> Vec2 {
        Vec2 {
            x: self.width as f32,
            y: self.height as f32,
        }
    }
}

/// Load a texture from a `DynamicImage`, automatically selecting the
/// smallest compatible DXGI_FORMAT (L8 → R8, L16 → R16, HDR → R32G32B32A32_FLOAT, etc.).
pub fn load_texture_from_dynamic_image(
    device: &ID3D11Device,
    img: &image::DynamicImage,
) -> Result<TextureInfo> {
    let (width, height) = img.dimensions();

    // Determine the best format and get raw pixel data.
    let (format, raw_data, row_pitch) = match img {
        image::DynamicImage::ImageLuma8(i) => {
            let data = i.as_raw();
            (DXGI_FORMAT_R8_UNORM, data.clone(), width as usize)
        }
        image::DynamicImage::ImageLumaA8(i) => {
            let data = i.as_raw();
            (DXGI_FORMAT_R8G8_UNORM, data.clone(), width as usize * 2)
        }
        image::DynamicImage::ImageRgb8(i) => {
            // D3D11 does not support R8G8B8_UNORM, expand to RGBA.
            let data: Vec<u8> = i
                .pixels()
                .flat_map(|p| [p.0[0], p.0[1], p.0[2], 255])
                .collect();
            (DXGI_FORMAT_R8G8B8A8_UNORM, data, width as usize * 4)
        }
        image::DynamicImage::ImageRgba8(i) => {
            let data = i.as_raw();
            (DXGI_FORMAT_R8G8B8A8_UNORM, data.clone(), width as usize * 4)
        }
        image::DynamicImage::ImageLuma16(i) => {
            let data = unsafe {
                std::slice::from_raw_parts::<u8>(
                    i.as_ptr() as *const _,
                    i.len() * std::mem::size_of::<u16>(),
                )
            }
            .to_vec();
            (DXGI_FORMAT_R16_UNORM, data, width as usize * 2)
        }
        image::DynamicImage::ImageRgba16(i) => {
            let data = unsafe {
                std::slice::from_raw_parts::<u8>(
                    i.as_ptr() as *const _,
                    i.len() * std::mem::size_of::<u16>(),
                )
            }
            .to_vec();
            (DXGI_FORMAT_R16G16B16A16_UNORM, data, width as usize * 8)
        }
        image::DynamicImage::ImageRgb32F(i) => {
            // D3D11 does not support R32G32B32_FLOAT, expand to RGBA.
            let data: Vec<f32> = i
                .pixels()
                .flat_map(|p| [p.0[0], p.0[1], p.0[2], 1.0])
                .collect();
            let raw = unsafe {
                std::slice::from_raw_parts(
                    data.as_ptr() as *const u8,
                    data.len() * std::mem::size_of::<f32>(),
                )
                .to_vec()
            };
            (DXGI_FORMAT_R32G32B32A32_FLOAT, raw, width as usize * 16)
        }
        image::DynamicImage::ImageRgba32F(i) => {
            let raw = i.as_raw();
            let data = unsafe {
                std::slice::from_raw_parts(raw.as_ptr() as *const u8, raw.len() * 4).to_vec()
            };
            (DXGI_FORMAT_R32G32B32A32_FLOAT, data, width as usize * 16)
        }
        _ => {
            // Fallback: convert to RGBA8
            let rgba = img.to_rgba8();
            let data = rgba.as_raw().clone();
            (DXGI_FORMAT_R8G8B8A8_UNORM, data, width as usize * 4)
        }
    };

    let texture = create_texture_2d(
        device,
        width,
        height,
        format,
        D3D11_BIND_SHADER_RESOURCE.0 as u32,
        D3D11_USAGE_DEFAULT,
        0,
        Some((&raw_data, row_pitch as u32)),
    )?;

    let srv = create_srv(device, &texture, format)?;

    Ok(TextureInfo {
        texture,
        srv,
        width,
        height,
        format,
    })
}
