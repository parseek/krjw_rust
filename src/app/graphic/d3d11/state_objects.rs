use anyhow::{Context, Result};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

#[allow(unused)]
#[derive(Debug)]
pub struct StateObjects {
    // Blend states
    pub blend_opaque: ID3D11BlendState,
    pub blend_alpha: ID3D11BlendState,
    pub blend_additive: ID3D11BlendState,

    // Samplers
    pub sampler_point_clamp: ID3D11SamplerState,
    pub sampler_linear_clamp: ID3D11SamplerState,
    pub sampler_linear_wrap: ID3D11SamplerState,

    // Rasterizer states
    pub rasterizer_solid_cull_none: ID3D11RasterizerState,
    pub rasterizer_solid_cull_back: ID3D11RasterizerState,
    pub rasterizer_wireframe: ID3D11RasterizerState,

    // Depth-stencil states
    pub depth_none: ID3D11DepthStencilState,
    pub depth_less: ID3D11DepthStencilState,

    // Built-in 1×1 white texture (for solid-color rendering via SpriteBatch2D)
    pub white_texture_srv: ID3D11ShaderResourceView,
}

impl StateObjects {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        unsafe {
            // ── Blend states ───────────────────────────────────────
            let blend_opaque = {
                let mut desc = D3D11_BLEND_DESC::default();
                desc.AlphaToCoverageEnable = false.into();
                desc.IndependentBlendEnable = false.into();
                desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: false.into(),
                    SrcBlend: D3D11_BLEND_ONE,
                    DestBlend: D3D11_BLEND_ZERO,
                    BlendOp: D3D11_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                };
                let mut state = None;
                device
                    .CreateBlendState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateBlendState (opaque) failed")?;
                state.unwrap()
            };

            let blend_alpha = {
                let mut desc = D3D11_BLEND_DESC::default();
                desc.AlphaToCoverageEnable = false.into();
                desc.IndependentBlendEnable = false.into();
                desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: true.into(),
                    SrcBlend: D3D11_BLEND_SRC_ALPHA,
                    DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                    BlendOp: D3D11_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                };
                let mut state = None;
                device
                    .CreateBlendState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateBlendState (alpha) failed")?;
                state.unwrap()
            };

            let blend_additive = {
                let mut desc = D3D11_BLEND_DESC::default();
                desc.AlphaToCoverageEnable = false.into();
                desc.IndependentBlendEnable = false.into();
                desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: true.into(),
                    SrcBlend: D3D11_BLEND_SRC_ALPHA,
                    DestBlend: D3D11_BLEND_ONE,
                    BlendOp: D3D11_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                };
                let mut state = None;
                device
                    .CreateBlendState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateBlendState (additive) failed")?;
                state.unwrap()
            };

            // ── Samplers ───────────────────────────────────────────
            let sampler_point_clamp = {
                let desc = D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MipLODBias: 0.0,
                    MaxAnisotropy: 0,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    BorderColor: [0.0, 0.0, 0.0, 0.0],
                    MinLOD: 0.0,
                    MaxLOD: f32::MAX,
                };
                let mut sampler = None;
                device
                    .CreateSamplerState(&desc, Some(&mut sampler))
                    .context("ID3D11Device::CreateSamplerState (point_clamp) failed")?;
                sampler.unwrap()
            };

            let sampler_linear_clamp = {
                let desc = D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MipLODBias: 0.0,
                    MaxAnisotropy: 0,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    BorderColor: [0.0, 0.0, 0.0, 0.0],
                    MinLOD: 0.0,
                    MaxLOD: f32::MAX,
                };
                let mut sampler = None;
                device
                    .CreateSamplerState(&desc, Some(&mut sampler))
                    .context("ID3D11Device::CreateSamplerState (linear_clamp) failed")?;
                sampler.unwrap()
            };

            let sampler_linear_wrap = {
                let desc = D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                    AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                    AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                    MipLODBias: 0.0,
                    MaxAnisotropy: 0,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    BorderColor: [0.0, 0.0, 0.0, 0.0],
                    MinLOD: 0.0,
                    MaxLOD: f32::MAX,
                };
                let mut sampler = None;
                device
                    .CreateSamplerState(&desc, Some(&mut sampler))
                    .context("ID3D11Device::CreateSamplerState (linear_wrap) failed")?;
                sampler.unwrap()
            };

            // ── Rasterizer states ──────────────────────────────────
            let rasterizer_solid_cull_none = {
                let desc = D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    FrontCounterClockwise: true.into(),
                    DepthBias: 0,
                    DepthBiasClamp: 0.0,
                    SlopeScaledDepthBias: 0.0,
                    DepthClipEnable: true.into(),
                    ScissorEnable: false.into(),
                    MultisampleEnable: false.into(),
                    AntialiasedLineEnable: false.into(),
                };
                let mut state = None;
                device
                    .CreateRasterizerState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateRasterizerState (solid_cull_none) failed")?;
                state.unwrap()
            };

            let rasterizer_solid_cull_back = {
                let desc = D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_BACK,
                    FrontCounterClockwise: true.into(),
                    DepthBias: 0,
                    DepthBiasClamp: 0.0,
                    SlopeScaledDepthBias: 0.0,
                    DepthClipEnable: true.into(),
                    ScissorEnable: false.into(),
                    MultisampleEnable: false.into(),
                    AntialiasedLineEnable: false.into(),
                };
                let mut state = None;
                device
                    .CreateRasterizerState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateRasterizerState (solid_cull_back) failed")?;
                state.unwrap()
            };

            let rasterizer_wireframe = {
                let desc = D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_WIREFRAME,
                    CullMode: D3D11_CULL_NONE,
                    FrontCounterClockwise: true.into(),
                    DepthBias: 0,
                    DepthBiasClamp: 0.0,
                    SlopeScaledDepthBias: 0.0,
                    DepthClipEnable: true.into(),
                    ScissorEnable: false.into(),
                    MultisampleEnable: false.into(),
                    AntialiasedLineEnable: false.into(),
                };
                let mut state = None;
                device
                    .CreateRasterizerState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateRasterizerState (wireframe) failed")?;
                state.unwrap()
            };

            // ── Depth-stencil states ───────────────────────────────
            let depth_none = {
                let desc = D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: false.into(),
                    DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
                    DepthFunc: D3D11_COMPARISON_LESS,
                    StencilEnable: false.into(),
                    StencilReadMask: 0,
                    StencilWriteMask: 0,
                    FrontFace: D3D11_DEPTH_STENCILOP_DESC::default(),
                    BackFace: D3D11_DEPTH_STENCILOP_DESC::default(),
                };
                let mut state = None;
                device
                    .CreateDepthStencilState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateDepthStencilState (none) failed")?;
                state.unwrap()
            };

            let depth_less = {
                let desc = D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                    DepthFunc: D3D11_COMPARISON_LESS,
                    StencilEnable: false.into(),
                    StencilReadMask: 0,
                    StencilWriteMask: 0,
                    FrontFace: D3D11_DEPTH_STENCILOP_DESC::default(),
                    BackFace: D3D11_DEPTH_STENCILOP_DESC::default(),
                };
                let mut state = None;
                device
                    .CreateDepthStencilState(&desc, Some(&mut state))
                    .context("ID3D11Device::CreateDepthStencilState (less) failed")?;
                state.unwrap()
            };

            // ── Built-in 1×1 white texture ───────────────────────────
            let white_texture_srv = {
                use super::d3d11_utils;

                let white_pixel: [u8; 4] = [255; 4];
                let tex = d3d11_utils::create_texture_2d(
                    device,
                    1,
                    1,
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                    D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    D3D11_USAGE_IMMUTABLE,
                    0,
                    Some((&white_pixel, 4)),
                )
                .context("create_white_texture failed")?;
                d3d11_utils::create_srv(device, &tex, DXGI_FORMAT_R8G8B8A8_UNORM)
                    .context("create_white_srv failed")?
            };

            Ok(Self {
                blend_opaque,
                blend_alpha,
                blend_additive,
                sampler_point_clamp,
                sampler_linear_clamp,
                sampler_linear_wrap,
                rasterizer_solid_cull_none,
                rasterizer_solid_cull_back,
                rasterizer_wireframe,
                depth_none,
                depth_less,
                white_texture_srv,
            })
        }
    }
}
