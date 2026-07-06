use anyhow::{Context, Result};
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::shader_utils::compile_shader;

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

const VS_SOURCE: &[u8] = include_bytes!("test_triangle_vs.hlsl");
const PS_SOURCE: &[u8] = include_bytes!("test_triangle_ps.hlsl");

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
        let vs_blob = compile_shader(
            VS_SOURCE,
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;
        let ps_blob = compile_shader(
            PS_SOURCE,
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"ps_5_0\0".as_ptr()),
        )?;

        let mut vertex_shader = None;
        unsafe {
            device
                .CreateVertexShader(&vs_blob, None, Some(&mut vertex_shader))
                .context("ID3D11Device::CreateVertexShader failed")?;
        }

        let mut pixel_shader = None;
        unsafe {
            device
                .CreatePixelShader(&ps_blob, None, Some(&mut pixel_shader))
                .context("ID3D11Device::CreatePixelShader failed")?;
        }

        let mut input_layout = None;
        unsafe {
            device
                .CreateInputLayout(&INPUT_LAYOUT_DESC, &vs_blob, Some(&mut input_layout))
                .context("ID3D11Device::CreateInputLayout failed")?;
        }

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

        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<[Vertex; 3]>() as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let init_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: vertices.as_ptr() as *const _ as *mut _,
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };

        let mut vertex_buffer = None;
        unsafe {
            device
                .CreateBuffer(&buffer_desc, Some(&init_data), Some(&mut vertex_buffer))
                .context("ID3D11Device::CreateBuffer failed")?;
        }

        Ok(Self {
            vertex_buffer: vertex_buffer.unwrap(),
            vertex_shader: vertex_shader.unwrap(),
            pixel_shader: pixel_shader.unwrap(),
            input_layout: input_layout.unwrap(),
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
