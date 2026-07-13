use anyhow::Result;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::d3d11_utils::{self, create_input_layout, write_buffer};

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
        let vs_blob = d3d11_utils::compile_shader(
            include_bytes!("test_sprite_vs.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;
        let ps_blob = d3d11_utils::compile_shader(
            include_bytes!("test_sprite_ps.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"ps_5_0\0".as_ptr()),
        )?;

        let vertex_shader = d3d11_utils::create_vs(device, include_bytes!("test_sprite_vs.hlsl"))?;
        let pixel_shader = d3d11_utils::create_ps(device, include_bytes!("test_sprite_ps.hlsl"))?;
        let input_layout = create_input_layout(device, &INPUT_LAYOUT_DESC, &vs_blob)?;

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

        let vertex_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            std::mem::size_of::<[SpriteVertex; 4]>() as u32,
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        let indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
        let index_buffer = d3d11_utils::create_immutable_buffer(
            device,
            d3d11_utils::as_u8_slice(&indices),
            D3D11_BIND_INDEX_BUFFER.0 as u32,
        )?;

        let cb_world = d3d11_utils::create_constant_buffer::<CbWorld>(device)?;
        let cb_sprite = d3d11_utils::create_constant_buffer::<CbSprite>(device)?;

        // Texture
        let texture = d3d11_utils::create_texture_2d(
            device,
            tex_width,
            tex_height,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D11_BIND_SHADER_RESOURCE.0 as u32,
            D3D11_USAGE_DEFAULT,
            0,
            Some((tex_data, tex_width * 4)),
        )?;

        let texture_srv = d3d11_utils::create_srv(device, &texture, DXGI_FORMAT_R8G8B8A8_UNORM)?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            cb_world,
            cb_sprite,
            vertex_shader,
            pixel_shader,
            input_layout,
            texture_srv,
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

        write_buffer(context, &self.vertex_buffer, &verts)?;
        write_buffer(context, &self.cb_world, &[CbWorld { mvp: *mvp }])?;
        write_buffer(
            context,
            &self.cb_sprite,
            &[CbSprite {
                transform_spr: *spr,
                color,
            }],
        )?;

        unsafe {
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
