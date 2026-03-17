use std::path::Path;

/// Load a compiled extension dynamic library and return the plugin it exports.
///
/// # Safety
/// The `.so`/`.dll` must export `create_plugin` with the expected signature.
pub fn load_extension_lib(lib_path: &Path) -> anyhow::Result<Box<dyn crate::plugin::Plugin>> {
    use libloading::{Library, Symbol};
    unsafe {
        let lib = Library::new(lib_path)?;
        let constructor: Symbol<unsafe extern "C" fn() -> *mut dyn crate::plugin::Plugin> =
            lib.get(b"create_plugin")?;
        let raw = constructor();
        if raw.is_null() {
            anyhow::bail!("create_plugin returned null");
        }
        // We intentionally leak the library so the plugin remains valid.
        std::mem::forget(lib);
        Ok(Box::from_raw(raw))
    }
}
