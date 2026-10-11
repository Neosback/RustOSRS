//! The few places where the desktop and the browser build differ.

#[cfg(target_arch = "wasm32")]
pub mod web;

/// Open a link in the user's browser.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        open::that(url).map_err(|error| error.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::open_url(url)
    }
}

/// Resident memory of this process in MiB, where the platform can say.
pub fn process_memory_mib() -> Option<f32> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        memory_stats::memory_stats().map(|usage| usage.physical_mem as f32 / (1024.0 * 1024.0))
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::wasm_memory_mib()
    }
}
