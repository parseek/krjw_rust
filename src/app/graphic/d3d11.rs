use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
    },
    core::Interface,
};
use winit::{raw_window_handle::HasWindowHandle, window::Window};

pub mod test_triangle;

#[derive(Debug)]
pub struct D3D11 {
    pub device: ID3D11Device,
    pub swap_chain: IDXGISwapChain,
    pub imm_context: ID3D11DeviceContext,
    render_target_view: Option<ID3D11RenderTargetView>,
}

fn get_hwnd(window: &Window) -> HWND {
    let handle = window.window_handle().unwrap();
    let handle = handle.as_raw();
    if let winit::raw_window_handle::RawWindowHandle::Win32(windows_handle) = handle {
        HWND(windows_handle.hwnd.get() as *mut _)
    } else {
        panic!("only Win32 windows are supported");
    }
}

impl D3D11 {
    pub fn rtv(&self) -> &ID3D11RenderTargetView {
        self.render_target_view
            .as_ref()
            .expect("render_target_view is None — did ResizeBuffers fail without rebuilding it?")
    }

    pub fn clear_screen(&self, color_rgba: &[f32; 4]) {
        unsafe {
            self.imm_context
                .ClearRenderTargetView(self.rtv(), color_rgba);
        }
    }
    pub fn set_viewport(&self, top_x: f32, top_y: f32, width: f32, height: f32) {
        unsafe {
            self.imm_context
                .RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: top_x,
                    TopLeftY: top_y,
                    Width: width,
                    Height: height,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
        }
    }
    pub fn present(&self) -> Result<()> {
        unsafe {
            self.swap_chain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .context("IDXGISwapChain::Present failed")?;
            Ok(())
        }
    }
}

impl D3D11 {
    pub fn init_on_hwnd(hwnd: HWND) -> Result<Self> {
        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: 0,
                Height: 0,
                RefreshRate: DXGI_RATIONAL {
                    Numerator: 0,
                    Denominator: 0,
                },
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            OutputWindow: hwnd,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            Flags: 0,
        };

        let feature_levels = [
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];
        let mut swap_chain = None;
        let mut device = None;
        let mut imm_context = None;
        let mut feature_level = D3D_FEATURE_LEVEL(0);

        #[allow(unused)]
        let mut creation_flag = D3D11_CREATE_DEVICE_FLAG(0);
        #[cfg(debug_assertions)]
        {
            creation_flag = D3D11_CREATE_DEVICE_DEBUG;
        }

        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                creation_flag,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&swap_chain_desc),
                Some(&mut swap_chain),
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut imm_context),
            )
            .context("D3D11CreateDeviceAndSwapChain failed")?;
        }

        let device = device.unwrap();
        let swap_chain = swap_chain.unwrap();
        let imm_context = imm_context.unwrap();

        // Disable DXGI Alt+Enter fullscreen
        unsafe {
            let dxgi_device: IDXGIDevice = device
                .cast()
                .context("ID3D11Device::cast<IDXGIDevice> failed")?;
            let dxgi_adapter: IDXGIAdapter = dxgi_device
                .GetParent()
                .context("IDXGIDevice::GetParent<IDXGIAdapter> failed")?;
            let dxgi_factory: IDXGIFactory = dxgi_adapter
                .GetParent()
                .context("IDXGIAdapter::GetParent<IDXGIFactory> failed")?;
            dxgi_factory
                .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)
                .context("IDXGIFactory::MakeWindowAssociation failed")?;
        }

        let render_target_view = Self::create_rtv(&device, &swap_chain)?;

        Ok(Self {
            device,
            swap_chain,
            imm_context,
            render_target_view: Some(render_target_view),
        })
    }

    fn create_rtv(
        device: &ID3D11Device,
        swap_chain: &IDXGISwapChain,
    ) -> Result<ID3D11RenderTargetView> {
        unsafe {
            let back_buffer = swap_chain
                .GetBuffer::<ID3D11Texture2D>(0)
                .context("IDXGISwapChain::GetBuffer failed")?;

            let mut rtv = None;
            device
                .CreateRenderTargetView(
                    &back_buffer,
                    Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                        Format: DXGI_FORMAT_UNKNOWN,
                        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                        ..Default::default()
                    }),
                    Some(&mut rtv),
                )
                .context("ID3D11Device::CreateRenderTargetView failed")?;

            Ok(rtv.unwrap())
        }
    }

    pub fn init_on_window(window: &Window) -> Result<Self> {
        let hwnd = get_hwnd(window);
        Self::init_on_hwnd(hwnd)
    }

    pub fn reset_rtv(
        &mut self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        swapchain_flags: DXGI_SWAP_CHAIN_FLAG,
    ) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        self.render_target_view = None;

        unsafe {
            self.swap_chain
                .ResizeBuffers(0, width, height, format, swapchain_flags)
                .context("IDXGISwapChain::ResizeBuffers failed")?;
        }

        let new_rtv = Self::create_rtv(&self.device, &self.swap_chain)?;
        self.render_target_view = Some(new_rtv);

        Ok(())
    }

    pub fn on_resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.reset_rtv(width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))
    }
}