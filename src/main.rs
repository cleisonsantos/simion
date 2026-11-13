#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod icon;
mod icons;
mod markdown;
mod model;
mod overlays;
mod search;
mod sidebar;
mod store;
mod syntax;
mod theme;
mod ui;
mod updater;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("Simion {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            concat!(
                "Simion {} — um caderno de blocos local.\n\n",
                "Uso:\n",
                "  simion                 abre o caderno\n",
                "  simion --version       mostra a versão\n",
                "  simion --check-update  verifica se há uma nova versão\n",
                "  simion --update        baixa e instala a nova versão, se houver\n",
                "  simion --help          mostra esta ajuda\n\n",
                "Dados: {}\n",
                "Projeto: https://github.com/cleisonsantos/simion",
            ),
            env!("CARGO_PKG_VERSION"),
            store::data_dir().join("simion.db").display()
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--check-update") {
        match updater::check_latest() {
            Ok(Some(release)) => {
                println!(
                    "Simion {} — nova versão {} disponível: {}",
                    env!("CARGO_PKG_VERSION"),
                    release.version,
                    release.html_url
                );
                std::process::exit(1);
            }
            Ok(None) => {
                println!("Simion {} — atualizado", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Err(err) => {
                eprintln!("Erro ao verificar atualizações: {err}");
                std::process::exit(2);
            }
        }
    }

    if args.iter().any(|arg| arg == "--update") {
        let release = match updater::check_latest() {
            Ok(Some(release)) => release,
            Ok(None) => {
                println!("Simion {} — atualizado", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Err(err) => {
                eprintln!("Erro ao verificar atualizações: {err}");
                std::process::exit(2);
            }
        };
        let platform = updater::detect_platform();
        let asset = match updater::select_asset(&release, platform) {
            Ok(asset) => asset,
            Err(err) => {
                eprintln!("Erro: {err}");
                std::process::exit(2);
            }
        };
        eprintln!("Baixando Simion {} ({})…", release.version, asset.name);
        let mut last_percent = -1i32;
        let path = match updater::download_asset(asset, |progress| {
            let percent = (progress * 100.0).round() as i32;
            if percent != last_percent {
                last_percent = percent;
                eprintln!("  {percent}%");
            }
        }) {
            Ok(path) => path,
            Err(err) => {
                eprintln!("Erro ao baixar atualização: {err}");
                std::process::exit(2);
            }
        };
        if updater::replaces_running_app(&path) {
            println!("Fechando e reabrindo o Simion {}…", release.version);
        }
        match updater::apply_update(&path) {
            Ok(updater::ApplyOutcome::InstallerOpened) => {
                println!("Instalador aberto — conclua a instalação");
                return Ok(());
            }
            Ok(updater::ApplyOutcome::Restarted) => return Ok(()),
            Err(err) => {
                eprintln!("Erro ao aplicar atualização: {err}");
                std::process::exit(2);
            }
        }
    }

    // Windows: sem um AppUserModelID explicito o shell trata o processo como
    // "genérico" e o icone na barra de tarefas / pinado nao casa com o atalho.
    // Precisa ser identico ao System.AppUserModel.ID declarado nos atalhos.
    #[cfg(windows)]
    set_app_user_model_id("com.cleisonsantos.simion");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Simion")
            .with_app_id("com.cleisonsantos.simion")
            // Primeira execucao apenas: depois disso a geometria vem do storage
            // de persistencia do eframe (issue #6).
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([720.0, 520.0])
            .with_icon(Arc::new(icon::app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Simion",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

/// Registra o AppUserModelID do processo no shell do Windows.
///
/// Sem isso, o Windows agrupa o Simion como um processo anonimo: o icone
/// pinado/fixado na barra nao casa com o atalho instalado pelo MSI e pode
/// aparecer um icone generico ao lado do correto (#7). O valor tem de ser o
/// mesmo `System.AppUserModel.ID` gravado nos atalhos (wix/main.wxs).
#[cfg(windows)]
fn set_app_user_model_id(id: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Rust 2024 exige `unsafe extern` para blocos externos.
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(id).encode_wide().chain([0]).collect();
    // Falha aqui nao e fatal: o app abre normalmente, so perde o agrupamento.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
    }
}
