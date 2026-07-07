use anyhow::Result;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::d3d11_utils::{self, create_input_layout};

pub struct TestTriangleRender {
    vertex_buffer: ID3D11Buffer,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,
}

#[repr(C)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 4],
}

const INPUT_LAYOUT_DESC: [D3D11_INPUT_ELEMENT_DESC; 2] = [
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"POSITION\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 0,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"COLOR\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 12,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

impl TestTriangleRender {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = d3d11_utils::compile_shader(
            include_bytes!("test_triangle_vs.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;

        let vertex_shader =
            d3d11_utils::create_vs(device, include_bytes!("test_triangle_vs.hlsl"))?;
        let pixel_shader = d3d11_utils::create_ps(device, include_bytes!("test_triangle_ps.hlsl"))?;
        let input_layout = create_input_layout(device, &INPUT_LAYOUT_DESC, &vs_blob)?;

        let vertices: [Vertex; 3] = [
            Vertex {
                pos: [0.0, 0.5, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex {
                pos: [0.5, -0.5, 0.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
            Vertex {
                pos: [-0.5, -0.5, 0.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
        ];

        let vertex_buffer = d3d11_utils::create_immutable_buffer(
            device,
            d3d11_utils::as_u8_slice(&vertices),
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        Ok(Self {
            vertex_buffer,
            vertex_shader,
            pixel_shader,
            input_layout,
        })
    }

    pub fn draw(&self, gfx: &D3D11) {
        let context = &gfx.imm_context;
        let rtv = gfx.rtv();
        unsafe {
            context.IASetInputLayout(&self.input_layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            let stride = std::mem::size_of::<Vertex>() as u32;
            let offset = 0u32;
            context.IASetVertexBuffers(
                0,
                1,
                Some([Some(self.vertex_buffer.clone())].as_ptr()),
                Some([stride].as_ptr()),
                Some([offset].as_ptr()),
            );
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);
            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            context.Draw(3, 0);
        }
    }
}
