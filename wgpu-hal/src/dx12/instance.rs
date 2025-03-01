use winapi::shared::{dxgi1_5, minwindef};

use super::SurfaceTarget;
use crate::auxil::{self, dxgi::result::HResult as _};
use std::{mem, sync::Arc};

impl Drop for super::Instance {
    fn drop(&mut self) {
        unsafe { self.factory.destroy() };
        crate::auxil::dxgi::exception::unregister_exception_handler();
    }
}

impl crate::Instance<super::Api> for super::Instance {
    unsafe fn init(desc: &crate::InstanceDescriptor) -> Result<Self, crate::InstanceError> {
        let lib_main = native::D3D12Lib::new().map_err(|_| crate::InstanceError)?;

        if desc.flags.contains(crate::InstanceFlags::VALIDATION) {
            // Enable debug layer
            match lib_main.get_debug_interface() {
                Ok(pair) => match pair.into_result() {
                    Ok(debug_controller) => {
                        debug_controller.enable_layer();
                        debug_controller.Release();
                    }
                    Err(err) => {
                        log::warn!("Unable to enable D3D12 debug interface: {}", err);
                    }
                },
                Err(err) => {
                    log::warn!("Debug interface function for D3D12 not found: {:?}", err);
                }
            }
        }

        // Create DXGIFactory4
        let (lib_dxgi, factory) = auxil::dxgi::factory::create_factory(
            auxil::dxgi::factory::DxgiFactoryType::Factory4,
            desc.flags,
        )?;

        let mut supports_allow_tearing = false;
        #[allow(trivial_casts)]
        if let Some(factory5) = factory.as_factory5() {
            let mut allow_tearing: minwindef::BOOL = minwindef::FALSE;
            let hr = factory5.CheckFeatureSupport(
                dxgi1_5::DXGI_FEATURE_PRESENT_ALLOW_TEARING,
                &mut allow_tearing as *mut _ as *mut _,
                mem::size_of::<minwindef::BOOL>() as _,
            );

            match hr.into_result() {
                Err(err) => log::warn!("Unable to check for tearing support: {}", err),
                Ok(()) => supports_allow_tearing = true,
            }
        }

        Ok(Self {
            // The call to create_factory will only succeed if we get a factory4, so this is safe.
            factory,
            library: Arc::new(lib_main),
            _lib_dxgi: lib_dxgi,
            supports_allow_tearing,
            flags: desc.flags,
        })
    }

    unsafe fn create_surface(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<super::Surface, crate::InstanceError> {
        match has_handle.raw_window_handle() {
            raw_window_handle::RawWindowHandle::Win32(handle) => Ok(super::Surface {
                factory: self.factory,
                target: SurfaceTarget::WndHandle(handle.hwnd as *mut _),
                supports_allow_tearing: self.supports_allow_tearing,
                swap_chain: None,
            }),
            _ => Err(crate::InstanceError),
        }
    }
    unsafe fn destroy_surface(&self, _surface: super::Surface) {
        // just drop
    }

    unsafe fn enumerate_adapters(&self) -> Vec<crate::ExposedAdapter<super::Api>> {
        let adapters = auxil::dxgi::factory::enumerate_adapters(self.factory);

        adapters
            .into_iter()
            .filter_map(|raw| super::Adapter::expose(raw, &self.library, self.flags))
            .collect()
    }
}
