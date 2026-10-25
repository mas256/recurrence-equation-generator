// Hide the console window of the release build on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use tauri::{webview::DownloadEvent, Manager, WebviewUrl, WebviewWindowBuilder};

/// TeX and Recipe files keep the page's own file names and always go to the
/// Downloads folder (the WebView's default differs between platforms), without
/// overwriting an earlier file.
fn download_destination(suggested: &Path, downloads: Option<PathBuf>) -> PathBuf {
    let Some(dir) = downloads else {
        return suggested.to_path_buf();
    };
    let target = dir.join(suggested.file_name().unwrap_or("download".as_ref()));
    let stem = target
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = target
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()));
    let mut candidate = target.clone();
    for n in 2.. {
        if !candidate.exists() {
            break;
        }
        candidate = target.with_file_name(format!(
            "{stem} ({n}){}",
            extension.as_deref().unwrap_or("")
        ));
    }
    candidate
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Fall back to ~/Downloads where the platform has no configured folder.
            let downloads = app.path().download_dir().ok().or_else(|| {
                let dir = app.path().home_dir().ok()?.join("Downloads");
                dir.is_dir().then_some(dir)
            });
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("漸化式ラボ")
                .inner_size(1280.0, 900.0)
                .min_inner_size(360.0, 600.0)
                .on_download(move |_, event| {
                    if let DownloadEvent::Requested { destination, .. } = event {
                        *destination = download_destination(destination, downloads.clone());
                    }
                    true
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("漸化式ラボを起動できませんでした");
}

#[cfg(test)]
mod tests {
    use super::download_destination;
    use std::fs;

    #[test]
    fn downloads_get_a_free_name_in_the_downloads_folder() {
        let dir = std::env::temp_dir().join("recurrence-lab-download-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let name = "recurrence-0123456789ab.tex".as_ref();
        assert_eq!(
            download_destination(name, Some(dir.clone())),
            dir.join(name)
        );
        fs::write(dir.join(name), "").unwrap();
        assert_eq!(
            download_destination(name, Some(dir.clone())),
            dir.join("recurrence-0123456789ab (2).tex")
        );
        // A WebView default elsewhere is redirected to the Downloads folder.
        let elsewhere = std::env::temp_dir().join("recipe-0123456789ab.json");
        assert_eq!(
            download_destination(&elsewhere, Some(dir.clone())),
            dir.join("recipe-0123456789ab.json")
        );
        assert_eq!(download_destination(&elsewhere, None), elsewhere);
        fs::remove_dir_all(&dir).unwrap();
    }
}
