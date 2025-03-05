// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rayon::iter::{ParallelBridge, ParallelExtend, ParallelIterator};
use rfd::AsyncFileDialog;
use slint::{ToSharedString, Weak};
use std::{
    error::Error,
    fs,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use walkdir::WalkDir;
slint::include_modules!();

const START_DIRECTORY: &str = "C:\\";

fn main() -> Result<(), Box<dyn Error>> {
    let app = AppWindow::new()?;

    setup(&app);

    app.run()?;

    Ok(())
}

fn setup(app: &AppWindow) {
    root_button_clicked(&app);
    output_button_clicked(&app);
    search_button_clicked(&app);
}

fn root_button_clicked(app: &AppWindow) {
    let app_weak = app.as_weak();

    app.on_ButtonSelectRootClicked(move || {
        if let Some(app_upgrade) = app_weak.upgrade() {
            // Spawn the async block to avoid blocking the main thread
            let _ = slint::spawn_local(async move {
                if let Some(file) = AsyncFileDialog::new()
                    .set_directory(START_DIRECTORY)
                    .pick_folder()
                    .await
                {
                    let path = file.path().to_string_lossy().to_string();

                    app_upgrade.set_root_path(path.into());
                }
            });
        }
    });
}

fn output_button_clicked(app: &AppWindow) {
    let app_weak = app.as_weak();
    app.on_ButtonSelectOutputClicked(move || {
        // Spawn the async block to avoid blocking the main thread
        if let Some(app_upgrade) = app_weak.upgrade() {
            let _ = slint::spawn_local(async move {
                if let Some(file) = AsyncFileDialog::new()
                    .set_directory(START_DIRECTORY)
                    .pick_folder()
                    .await
                {
                    let path = file.path().to_string_lossy().to_string();

                    app_upgrade.set_output_path(path.into());
                }
            });
        }
    });
}

fn search_button_clicked(app: &AppWindow) {
    let app_weak = app.as_weak();
    app.on_ButtonSearchClicked(move |root_path, output_path| {
        if let Some(app_upgrade) = app_weak.upgrade() {
            // Ensure the destination directory exists
            if let Ok(()) = fs::create_dir_all(&output_path) {}

            let file_extensions = get_avaliable_extentions(app_upgrade.clone_strong());

            let app_weak = app_upgrade.as_weak();
            thread::spawn(move || {
                process_files_concurrently(&root_path, &output_path, &file_extensions, app_weak);
            });
        }
    });
}

// process files logic
fn process_files_concurrently(
    root_path: &str,
    output_path: &str,
    file_extensions: &[&str],
    app_weak: Weak<AppWindow>,
) {
    // Callback
    let show_wating_screen = || {
        let _ = app_weak.upgrade_in_event_loop(move |app| {
            app.invoke_UpdateWatingText("Processing files... This may take a few moments depending on the number of files in the directory.
             Please wait."
            .to_shared_string());

            app.invoke_ShowWatingScreen(true);
            app.set_can_exit_wating_screen(false);
        });
    };

    let output_folder = output_path.to_string();
    // Callback
    let can_exit_wating_screen = || {
        let _ = app_weak.upgrade_in_event_loop(move |app| {
            app.invoke_UpdateWatingText(
                "File processing is complete. Click anywhere to close this message and continue."
                    .to_shared_string(),
            );

            let output = output_folder;
            app.on_ExitWatingScreen(move || {
                open_file_explorer(&output); // Safe to use String here
            });

            app.set_can_exit_wating_screen(true);
        });
    };
    show_wating_screen();

    let time = Instant::now();
    // Shared progress counter for tracking
    let progress_counter = Arc::new(Mutex::new(0));

    // Spawn a thread for file discovery
    let discovered_files = WalkDir::new(root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| entry.ok()) // Filter out errors
        .filter(|entry| {
            entry.path().is_file() && has_valid_extension(entry.path(), &file_extensions)
        });

    // Process files concurrently as they are discovered
    discovered_files.par_bridge().for_each(|entry| {
        let file_path = entry.path();
        let destination_path = Path::new(&output_path).join(entry.file_name());

        if let Err(e) = fs::copy(&file_path, &destination_path) {
            eprintln!(
                "Failed to copy {:?} to {:?}: {:?}",
                file_path, destination_path, e
            );
        } else {
            println!("Copied {:?} to {:?}", file_path, destination_path);

            // Update progress
            if let Ok(mut progress) = progress_counter.lock() {
                *progress += 1;
                println!("Files processed: {}", *progress);
            };
        }
    });

    can_exit_wating_screen();
    println!("Duration: {:#?}", time.elapsed());
    println!("All files processed.");
}

// open the file manager on os the user is using
fn open_file_explorer(path: &str) {
    #[cfg(target_os = "windows")]
    {
        if let Err(err) = Command::new("explorer").arg(path).spawn() {
            println!("Failed to open File Explorer: {}", err);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(err) = Command::new("open").arg(path).spawn() {
            println!("Failed to open Finder: {}", err);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Err(err) = Command::new("xdg-open").arg(path).spawn() {
            println!("Failed to open file manager: {err}");
        }
    }
}

// Helper function to check for valid extensions
fn has_valid_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map_or(false, |ext| {
            extensions.contains(&ext.to_lowercase().as_str())
        })
}

// add files extensions
fn get_avaliable_extentions<'a>(app: AppWindow) -> Vec<&'a str> {
    let mut extensions: Vec<&str> = Vec::new();
    if app.get_can_search_images() {
        extensions.par_extend(["jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp", "svg"]);
    }

    if app.get_can_search_videos() {
        extensions.par_extend([
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "mpeg", "3gp",
        ]);
    }

    if app.get_can_search_compressed_files() {
        extensions.par_extend(["zip", "rar", "7z", "tar", "gz", "bz2", "xz"]);
    }

    if app.get_can_search_sounds() {
        extensions.par_extend([
            "mp3", "wav", "ogg", "flac", "aac", "m4a", "alac", "wma", "aiff", "opus",
        ]);
    }
    
    return extensions;
}
