use windows::{
    Win32::Graphics::{Direct3D::Fxc::*, Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

pub struct TriangleRender {
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

// ──────────────────────────────────────────
// HLSL Shader Sources (null-terminated)
// ──────────────────────────────────────────

const VS_SOURCE: &[u8] = b"\
struct VS_INPUT {\
    float3 pos : POSITION;\
    float4 color : COLOR;\
};\
struct PS_INPUT {\
    float4 pos : SV_POSITION;\
    float4 color : COLOR;\
};\
PS_INPUT main(VS_INPUT input) {\
    PS_INPUT output;\
    output.pos = float4(input.pos, 1.0);\
    output.color = input.color;\
    return output;\
}\0";

const PS_SOURCE: &[u8] = b"\
struct PS_INPUT {\
    float4 pos : SV_POSITION;\
    float4 color : COLOR;\
};\
float4 main(PS_INPUT input) : SV_Target {\
    return input.color;\
}\0";

// ──────────────────────────────────────────
// Input Layout
// ──────────────────────────────────────────

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
        AlignedByteOffset: 12, // 3 * 4 bytes (pos)
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

// ──────────────────────────────────────────
// Shader Compilation Helper
// ──────────────────────────────────────────

fn compile_shader(source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Vec<u8> {
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
            0, // D3DCOMPILE_OPTIMIZATION_LEVEL0
            0, // no effect flags
            &mut shader_blob,
            Some(&mut error_blob),
        )
    };

    let shader_blob = shader_blob.as_ref().unwrap();

    if hr.is_err() {
        if error_blob.is_some() {
            let error_ref = error_blob.as_ref().unwrap();
            let msg = unsafe {
                std::slice::from_raw_parts(
                    error_ref.GetBufferPointer() as *const u8,
                    error_ref.GetBufferSize(),
                )
            };
            panic!(
                "Shader compilation failed:\n{}",
                String::from_utf8_lossy(msg)
            );
        } else {
            panic!("Shader compilation failed with HRESULT: {:?}", hr);
        }
    }

    // Take ownership of the returned ID3DBlob
    unsafe {
        std::slice::from_raw_parts::<u8>(
            shader_blob.GetBufferPointer() as *const _,
            shader_blob.GetBufferSize(),
        )
        .to_vec()
    }
}

// ──────────────────────────────────────────
// Implementation
// ──────────────────────────────────────────

impl TriangleRender {
    /// Creates a new TriangleRender by compiling shaders and setting up GPU resources.
    pub fn new(device: &ID3D11Device) -> Self {
        // 1. Compile shaders ──────────────────────────────────────────
        let vs_blob = {
            compile_shader(
                VS_SOURCE,
                PCSTR(b"main\0".as_ptr()),
                PCSTR(b"vs_5_0\0".as_ptr()),
            )
        };
        let ps_blob = {
            compile_shader(
                PS_SOURCE,
                PCSTR(b"main\0".as_ptr()),
                PCSTR(b"ps_5_0\0".as_ptr()),
            )
        };

        // 2. Create vertex / pixel shaders ────────────────────────────
        let mut vertex_shader = None;
        unsafe {
            device
                .CreateVertexShader(&vs_blob, None, Some(&mut vertex_shader))
                .unwrap_or_else(|e| panic!("Failed to create vertex shader: {:?}", e))
        };

        let mut pixel_shader = None;
        unsafe {
            device
                .CreatePixelShader(&ps_blob, None, Some(&mut pixel_shader))
                .unwrap_or_else(|e| panic!("Failed to create pixel shader: {:?}", e))
        };

        // 3. Create input layout ──────────────────────────────────────
        let mut input_layout = None;
        unsafe {
            device
                .CreateInputLayout(&INPUT_LAYOUT_DESC, &vs_blob, Some(&mut input_layout))
                .unwrap_or_else(|e| panic!("Failed to create input layout: {:?}", e))
        };

        // 4. Create vertex buffer ─────────────────────────────────────
        // Vertices: a colourful triangle
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
                .unwrap_or_else(|e| panic!("Failed to create vertex buffer: {:?}", e))
        };

        let vertex_buffer = vertex_buffer.unwrap();
        let vertex_shader = vertex_shader.unwrap();
        let pixel_shader = pixel_shader.unwrap();
        let input_layout = input_layout.unwrap();

        Self {
            vertex_buffer,
            vertex_shader,
            pixel_shader,
            input_layout,
        }
    }

    /// Binds all resources and issues the draw call.
    pub fn draw(&self, context: &ID3D11DeviceContext, rtv: &ID3D11RenderTargetView) {
        unsafe {
            // Set input layout
            context.IASetInputLayout(&self.input_layout);

            // Set primitive topology
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            // Bind vertex buffer (stride = sizeof Vertex, offset = 0)
            let stride = std::mem::size_of::<Vertex>() as u32;
            let offset = 0u32;
            context.IASetVertexBuffers(
                0, // start slot
                1, // number of buffers
                Some([Some(self.vertex_buffer.clone())].as_ptr()),
                Some([stride].as_ptr()),
                Some([offset].as_ptr()),
            );

            // Bind shaders
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);

            // RTV
            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);

            // Draw 3 vertices
            context.Draw(3, 0);
        }
    }
}
