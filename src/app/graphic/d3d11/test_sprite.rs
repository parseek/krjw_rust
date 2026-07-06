use anyhow::{Context, Result};
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::shader_utils::compile_shader;

#[allow(unused)]
pub struct TestSpriteRender {
    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    cb_world: ID3D11Buffer,
    cb_sprite: ID3D11Buffer,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,
    texture_srv: ID3D11ShaderResourceView,
    pub tex_width: u32,
    pub tex_height: u32,
}

#[repr(C)]
struct SpriteVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
struct CbWorld {
    mvp: glam::Mat4,
}

#[repr(C)]
struct CbSprite {
    transform_spr: glam::Mat4,
    color: [f32; 4],
}

const INPUT_LAYOUT_DESC: [D3D11_INPUT_ELEMENT_DESC; 2] = [
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"POSITION\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 0,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"TEXCOORD\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 8,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

impl TestSpriteRender {
    #[allow(unused)]
    pub fn new(
        device: &ID3D11Device,
        tex_data: &[u8],
        tex_width: u32,
        tex_height: u32,
    ) -> Result<Self> {
        let vs_blob = compile_shader(
            include_bytes!("test_sprite_vs.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;
        let ps_blob = compile_shader(
            include_bytes!("test_sprite_ps.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"ps_5_0\0".as_ptr()),
        )?;

        let mut vertex_shader = None;
        unsafe {
            device
                .CreateVertexShader(&vs_blob, None, Some(&mut vertex_shader))
                .context("VS failed")?;
        }
        let mut pixel_shader = None;
        unsafe {
            device
                .CreatePixelShader(&ps_blob, None, Some(&mut pixel_shader))
                .context("PS failed")?;
        }

        let mut input_layout = None;
        unsafe {
            device
                .CreateInputLayout(&INPUT_LAYOUT_DESC, &vs_blob, Some(&mut input_layout))
                .context("InputLayout failed")?;
        }

        let initial_verts: [SpriteVertex; 4] = [
            SpriteVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            SpriteVertex {
                pos: [1.0, 0.0],
                uv: [1.0, 0.0],
            },
            SpriteVertex {
                pos: [0.0, 1.0],
                uv: [0.0, 1.0],
            },
            SpriteVertex {
                pos: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
        ];

        let vb_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<[SpriteVertex; 4]>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let init_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: initial_verts.as_ptr() as *const _ as *mut _,
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut vertex_buffer = None;
        unsafe {
            device
                .CreateBuffer(&vb_desc, Some(&init_data), Some(&mut vertex_buffer))
                .context("VB failed")?;
        }

        let indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
        let ib_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<[u16; 6]>() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let ib_init = D3D11_SUBRESOURCE_DATA {
            pSysMem: indices.as_ptr() as *const _ as *mut _,
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut index_buffer = None;
        unsafe {
            device
                .CreateBuffer(&ib_desc, Some(&ib_init), Some(&mut index_buffer))
                .context("IB failed")?;
        }

        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<CbWorld>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_world = None;
        unsafe {
            device
                .CreateBuffer(&cb_desc, None, Some(&mut cb_world))
                .context("CB world failed")?;
        }

        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<CbSprite>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_sprite = None;
        unsafe {
            device
                .CreateBuffer(&cb_desc, None, Some(&mut cb_sprite))
                .context("CB sprite failed")?;
        }

        // Texture
        let tex_desc = D3D11_TEXTURE2D_DESC {
            Width: tex_width,
            Height: tex_height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let subres = D3D11_SUBRESOURCE_DATA {
            pSysMem: tex_data.as_ptr() as *const _ as *mut _,
            SysMemPitch: tex_width * 4,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&tex_desc, Some(&subres), Some(&mut texture))
                .context("CreateTexture2D failed")?;
        }
        let texture = texture.unwrap();

        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut texture_srv = None;
        unsafe {
            device
                .CreateShaderResourceView(&texture, Some(&srv_desc), Some(&mut texture_srv))
                .context("CreateSRV failed")?;
        }

        Ok(Self {
            vertex_buffer: vertex_buffer.unwrap(),
            index_buffer: index_buffer.unwrap(),
            cb_world: cb_world.unwrap(),
            cb_sprite: cb_sprite.unwrap(),
            vertex_shader: vertex_shader.unwrap(),
            pixel_shader: pixel_shader.unwrap(),
            input_layout: input_layout.unwrap(),
            texture_srv: texture_srv.unwrap(),
            tex_width,
            tex_height,
        })
    }

    #[allow(unused)]
    pub fn draw(
        &self,
        gfx: &D3D11,
        origin_px: [f32; 2],
        size_px: [f32; 2],
        uv_tl_px: [f32; 2],
        uv_size_px: [f32; 2],
        color: [f32; 4],
        mvp: &glam::Mat4,
        spr: &glam::Mat4,
    ) -> Result<()> {
        let context = &gfx.imm_context;
        let rtv = gfx.rtv();

        let ox = -origin_px[0];
        let oy = -origin_px[1];
        let positions: [[f32; 2]; 4] = [
            [ox, oy],
            [ox + size_px[0], oy],
            [ox, oy + size_px[1]],
            [ox + size_px[0], oy + size_px[1]],
        ];

        let tw = self.tex_width as f32;
        let th = self.tex_height as f32;
        let u0 = uv_tl_px[0] / tw;
        let v0 = uv_tl_px[1] / th;
        let u1 = (uv_tl_px[0] + uv_size_px[0]) / tw;
        let v1 = (uv_tl_px[1] + uv_size_px[1]) / th;

        let verts: [SpriteVertex; 4] = [
            SpriteVertex {
                pos: positions[0],
                uv: [u0, v0],
            },
            SpriteVertex {
                pos: positions[1],
                uv: [u1, v0],
            },
            SpriteVertex {
                pos: positions[2],
                uv: [u0, v1],
            },
            SpriteVertex {
                pos: positions[3],
                uv: [u1, v1],
            },
        ];

        unsafe {
            // Map + write vertex buffer
            let mut mapped = std::mem::zeroed();
            context
                .Map(
                    &self.vertex_buffer,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .context("context.Map(self.vertex_buffer) failed")?;
            std::ptr::copy_nonoverlapping(
                verts.as_ptr() as *const u8,
                mapped.pData as *mut u8,
                std::mem::size_of::<[SpriteVertex; 4]>(),
            );
            context.Unmap(&self.vertex_buffer, 0);

            // Map + write constant buffers
            let world_data = CbWorld { mvp: *mvp };
            let mut mapped = std::mem::zeroed();
            context
                .Map(
                    &self.cb_world,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .context("context.Map(self.cb_world) failed")?;
            std::ptr::copy_nonoverlapping(
                &world_data as *const _ as *const u8,
                mapped.pData as *mut u8,
                std::mem::size_of::<CbWorld>(),
            );
            context.Unmap(&self.cb_world, 0);

            // Map + write constant buffers
            let sprite_data = CbSprite {
                transform_spr: *spr,
                color,
            };
            let mut mapped = std::mem::zeroed();
            context
                .Map(
                    &self.cb_sprite,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .context("context.Map(self.cb_world) failed")?;
            std::ptr::copy_nonoverlapping(
                &sprite_data as *const _ as *const u8,
                mapped.pData as *mut u8,
                std::mem::size_of::<CbSprite>(),
            );
            context.Unmap(&self.cb_sprite, 0);

            // // Update constant buffers
            // let world_data = CbWorld { mvp: *mvp };
            // context.UpdateSubresource(
            //     &self.cb_world,
            //     0,
            //     None,
            //     &world_data as *const _ as *const _,
            //     0, 0,
            // );
            // let sprite_data = CbSprite { transform_spr: *spr, color };
            // context.UpdateSubresource(
            //     &self.cb_sprite,
            //     0,
            //     None,
            //     &sprite_data as *const _ as *const _,
            //     0, 0,
            // );

            // Bind & draw
            context.IASetInputLayout(&self.input_layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            let stride = std::mem::size_of::<SpriteVertex>() as u32;
            let offset = 0u32;
            context.IASetVertexBuffers(
                0,
                1,
                Some([Some(self.vertex_buffer.clone())].as_ptr()),
                Some([stride].as_ptr()),
                Some([offset].as_ptr()),
            );
            context.IASetIndexBuffer(&self.index_buffer, DXGI_FORMAT_R16_UINT, 0);

            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_world.clone())]));
            context.VSSetConstantBuffers(1, Some(&[Some(self.cb_sprite.clone())]));
            context.PSSetConstantBuffers(1, Some(&[Some(self.cb_sprite.clone())]));

            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);
            context.PSSetShaderResources(0, Some(&[Some(self.texture_srv.clone())]));

            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            context.DrawIndexed(6, 0, 0);
        }

        Ok(())
    }
}
