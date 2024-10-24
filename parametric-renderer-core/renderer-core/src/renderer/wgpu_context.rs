use std::sync::Arc;

use glam::UVec2;
use tracing::info;
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings};
use winit::window::Window;

use super::WindowOrFallback;

// TODO: Wrap this in an Arc (and move the size out of the context)
pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub surface: SurfaceOrFallback,
    pub _adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    size: UVec2,
    pub view_format: wgpu::TextureFormat,
}

impl WgpuContext {
    pub async fn new(window: WindowOrFallback) -> anyhow::Result<Self> {
        let size = window.size().max(UVec2::ONE);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = window
            .as_window()
            .map(|window| instance.create_surface(window))
            .transpose()?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                // Setting this is only needed for a fallback adapter. Which we don't want.
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No adapter found"))?;
        info!("Adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::default()
                        | (adapter.features() & GpuProfiler::ALL_WGPU_TIMER_FEATURES)
                        | (adapter.features() & wgpu::Features::POLYGON_MODE_LINE),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .unwrap();

        let (view_format, surface_format) = match &surface {
            Some(surface) => {
                let surface_format = surface
                    .get_capabilities(&adapter)
                    .formats
                    .first()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No valid surface format found"))?;
                let view_format = if surface_format.is_srgb() {
                    surface_format
                } else {
                    surface_format.add_srgb_suffix()
                };
                (view_format, surface_format)
            }
            None => (
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ),
        };

        let surface_or_fallback = match surface {
            Some(surface) => {
                let config = wgpu::SurfaceConfiguration {
                    format: surface_format,
                    view_formats: vec![view_format],
                    present_mode: wgpu::PresentMode::AutoVsync,
                    ..surface
                        .get_default_config(&adapter, size.x, size.y)
                        .ok_or_else(|| anyhow::anyhow!("No default surface config found"))?
                };
                surface.configure(&device, &config);
                SurfaceOrFallback::Surface {
                    surface,
                    config,
                    window: window
                        .as_window()
                        .expect("Expected window if there is a surface"),
                }
            }
            None => SurfaceOrFallback::Fallback {
                texture: Self::create_fallback_texture(&device, size, view_format),
            },
        };

        Ok(WgpuContext {
            instance,
            surface: surface_or_fallback,
            _adapter: adapter,
            device,
            queue,
            size,
            view_format,
        })
    }

    /// Tries to resize the swapchain to the new size.
    /// Returns the actual size of the swapchain if it was resized.
    pub fn try_resize(&mut self, new_size: UVec2) -> Option<UVec2> {
        let new_size = new_size.max(UVec2::new(1, 1));
        if new_size == self.size {
            return None;
        }
        self.size = new_size;
        match &mut self.surface {
            SurfaceOrFallback::Surface {
                surface, config, ..
            } => {
                config.width = new_size.x;
                config.height = new_size.y;
                surface.configure(&self.device, config);
                Some(new_size)
            }
            SurfaceOrFallback::Fallback { texture } => {
                *texture = Self::create_fallback_texture(&self.device, new_size, self.view_format);
                Some(new_size)
            }
        }
    }

    pub fn recreate_swapchain(&self) {
        match &self.surface {
            SurfaceOrFallback::Surface {
                surface, config, ..
            } => {
                surface.configure(&self.device, config);
            }
            SurfaceOrFallback::Fallback { .. } => {
                // No-op
            }
        }
    }

    pub fn size(&self) -> UVec2 {
        self.size
    }

    fn create_fallback_texture(
        device: &wgpu::Device,
        size: UVec2,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fallback surface"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    pub fn surface_texture(&self) -> Result<SurfaceTexture, wgpu::SurfaceError> {
        match &self.surface {
            SurfaceOrFallback::Surface { surface, .. } => {
                surface.get_current_texture().map(|surface_texture| {
                    let view = self.create_view(&surface_texture.texture);
                    SurfaceTexture::Surface(surface_texture, view)
                })
            }
            SurfaceOrFallback::Fallback { texture } => {
                Ok(SurfaceTexture::Fallback(self.create_view(&texture)))
            }
        }
    }

    fn create_view(&self, texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        })
    }
}

pub enum SurfaceTexture {
    Surface(wgpu::SurfaceTexture, wgpu::TextureView),
    Fallback(wgpu::TextureView),
}

impl SurfaceTexture {
    pub fn texture_view(&self) -> &wgpu::TextureView {
        match self {
            SurfaceTexture::Surface(_, view) => &view,
            SurfaceTexture::Fallback(view) => &view,
        }
    }

    pub fn present(self) {
        match self {
            SurfaceTexture::Surface(surface_texture, _) => {
                surface_texture.present();
            }
            SurfaceTexture::Fallback(_) => {}
        }
    }
}

pub enum SurfaceOrFallback {
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        window: Arc<Window>,
    },
    Fallback {
        texture: wgpu::Texture,
    },
}

pub fn create_profiler(_context: &WgpuContext) -> GpuProfiler {
    let gpu_profiler_settings = GpuProfilerSettings {
        enable_timer_queries: false, // Disabled by default
        ..GpuProfilerSettings::default()
    };

    #[cfg(feature = "tracy")]
    let profiler = GpuProfiler::new_with_tracy_client(
        gpu_profiler_settings.clone(),
        _context._adapter.get_info().backend,
        &_context.device,
        &_context.queue,
    )
    .unwrap_or_else(|e| match e {
        wgpu_profiler::CreationError::TracyClientNotRunning
        | wgpu_profiler::CreationError::TracyGpuContextCreationError(_) => {
            tracing::warn!("Failed to connect to Tracy. Continuing without Tracy integration.");
            GpuProfiler::new(gpu_profiler_settings).expect("Failed to create profiler")
        }
        _ => {
            panic!("Failed to create profiler: {}", e);
        }
    });

    #[cfg(not(feature = "tracy"))]
    let profiler = GpuProfiler::new(gpu_profiler_settings).expect("Failed to create profiler");
    profiler
}
