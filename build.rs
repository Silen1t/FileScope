use {
    std::{env, io},
    winresource::WindowsResource,
};

fn main() -> io::Result<()> {
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");
    // For windows only
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("ui/assets/images/FileScopeIcon.ico")
            .compile()?;
    }

    Ok(())
}
