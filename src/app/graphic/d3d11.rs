use windows::{
    core::{Interface, HRESULT},
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
    },
};
use winit::{raw_window_handle::HasWindowHandle, window::Window};

pub mod triangle;

#[derive(Debug, Default)]
pub struct D3D11 {
    pub device: Option<ID3D11Device>,
    pub swap_chain: Option<IDXGISwapChain>,
    pub imm_context: Option<ID3D11DeviceContext>,
    pub render_target_view: Option<ID3D11RenderTargetView>,
}

fn get_hwnd(window: &Window) -> HWND {
    let handle = window.window_handle().unwrap();
    let handle = handle.as_raw();
    if let winit::raw_window_handle::RawWindowHandle::Win32(windows_handle) = handle {
        HWND(windows_handle.hwnd.get() as *mut _)
    } else {
        panic!("Unsupported window handle");
    }
}

impl D3D11 {
    pub fn clear_screen(&self, color_rgba: &[f32; 4]) {
        unsafe {
            self.imm_context
                .as_ref()
                .unwrap()
                .ClearRenderTargetView(self.render_target_view.as_ref().unwrap(), color_rgba);
        }
    }
    pub fn set_viewport(&self, top_x: f32, top_y: f32, width: f32, height: f32) {
        unsafe {
            self.imm_context
                .as_ref()
                .unwrap()
                .RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: top_x,
                    TopLeftY: top_y,
                    Width: width,
                    Height: height,
                    MinDepth: 0.0,
                    MaxDepth: 1.0
                }]));
        }
    }
    pub fn present(&self) -> Result<(), HRESULT> {
        unsafe {
            let hr = self
                .swap_chain
                .as_ref()
                .unwrap()
                .Present(1, DXGI_PRESENT(0));
            if hr.is_err() {
                Err(hr)
            } else {
                Ok(())
            }
        }
    }
}

// Base Functions

impl D3D11 {
    pub fn init_on_hwnd(hwnd: HWND) -> Self {
        // Initialize D3D11 device, swap chain, and render target view here.
        // This is a placeholder for the actual initialization code.

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: 0,  // Unspecified width
                Height: 0, // Unspecified height
                RefreshRate: DXGI_RATIONAL {
                    Numerator: 0,
                    Denominator: 0,
                }, // Unspecified refresh rate
                Format: DXGI_FORMAT_R8G8B8A8_UNORM, // Unspecified format
                ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            }, // No MSAA on swap chain.
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            OutputWindow: hwnd,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD, // Supported on Windows 8 or later.
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

        let creation_flag = D3D11_CREATE_DEVICE_FLAG(0);
        #[cfg(debug_assertions)]
        let creation_flag = D3D11_CREATE_DEVICE_DEBUG;

        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None, // Use default adapter
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(), // No software rasterizer
                creation_flag,      // No creation flags
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&swap_chain_desc),
                Some(&mut swap_chain),    // Swap chain will be created later
                Some(&mut device),        // Device will be created later
                Some(&mut feature_level), // Feature level will be returned here
                Some(&mut imm_context),   // Device context will be created later
            )
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to create the D3D11 device and swap chain. Info: {:?}",
                    e
                );
            })
        }

        // Remove DXGI Alt+Enter Fullscreen

        unsafe {
            let dxgi_device = device
                .as_ref()
                .unwrap_or_else(|| panic!("Device is None."))
                .cast::<IDXGIDevice>()
                .unwrap_or_else(|e| panic!("Failed to query IDXGIDevice. Info: {:?}", e));

            let dxgi_adapter: IDXGIAdapter = dxgi_device.GetParent().unwrap();
            let dxgi_factory: IDXGIFactory = dxgi_adapter.GetParent().unwrap();
            dxgi_factory
                .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)
                .unwrap_or_else(|e| {
                    panic!("Failed to change DXGI Window Association.");
                });
        }

        // RTV

        let mut render_target_view = None;

        unsafe {
            let back_buffer = swap_chain
                .as_ref()
                .unwrap()
                .GetBuffer::<ID3D11Texture2D>(0)
                .unwrap_or_else(|e| {
                    panic!("Failed to get the back buffer. Info: {:?}", e);
                });

            device
                .as_ref()
                .unwrap()
                .CreateRenderTargetView(
                    &back_buffer,
                    Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                        Format: DXGI_FORMAT_UNKNOWN,
                        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                        ..Default::default()
                    }),
                    Some(&mut render_target_view),
                )
                .unwrap_or_else(|e| {
                    panic!("Failed to create the RTV. Info: {:?}", e);
                });
        }

        Self {
            device: device,
            swap_chain: swap_chain,
            imm_context: imm_context,
            render_target_view: render_target_view,
        }
    }
    pub fn init_on_window(window: &Window) -> Self {
        let hwnd = get_hwnd(window);
        Self::init_on_hwnd(hwnd)
    }

    pub fn reset_rtv(
        &mut self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        swapchain_flags: DXGI_SWAP_CHAIN_FLAG,
    ) {
        if width <= 0 || height <= 0 {
            return;
        };

        self.render_target_view = None;

        let device = self.device.as_ref().unwrap();
        let swap_chain = self.swap_chain.as_ref().unwrap();

        unsafe {
            swap_chain
                .ResizeBuffers(0, width, height, format, swapchain_flags)
                .unwrap_or_else(|e| {
                    panic!("Failed to resize the buffers. Info: {:?}", e);
                });
        }

        let mut render_target_view = None;

        unsafe {
            let back_buffer = swap_chain
                .GetBuffer::<ID3D11Texture2D>(0)
                .unwrap_or_else(|e| {
                    panic!("Failed to get the back buffer. Info: {:?}", e);
                });

            device
                .CreateRenderTargetView(
                    &back_buffer,
                    Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                        Format: DXGI_FORMAT_UNKNOWN,
                        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                        ..Default::default()
                    }),
                    Some(&mut render_target_view),
                )
                .unwrap_or_else(|e| {
                    panic!("Failed to create the RTV. Info: {:?}", e);
                });
        }
        self.render_target_view = render_target_view;
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.reset_rtv(width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0));
    }
}
