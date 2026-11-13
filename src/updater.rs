//! Descoberta, download e instalação de atualizações via GitHub Releases.
//!
//! As anotações continuam locais. Este módulo só faz requisições GET para
//! `api.github.com`, `github.com` e `objects.githubusercontent.com`.
//! Nenhum dado do caderno é enviado.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

pub const REPO: &str = "cleisonsantos/simion";
pub const RELEASES_API: &str = "https://api.github.com/repos/cleisonsantos/simion/releases/latest";
pub const GITHUB_DOWNLOAD_PREFIX: &str =
    "https://github.com/cleisonsantos/simion/releases/download/";
pub const GITHUB_OBJECTS_PREFIX: &str = "https://objects.githubusercontent.com/";
pub const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;
pub const CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

/// Limite curto para a consulta à API (DNS + TLS + resposta).
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// Download pode ser grande; o limite é maior, mas ainda existe.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Versão sem o prefixo `v`, por exemplo `0.1.10`.
    pub version: String,
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Debug, Deserialize)]
struct RawAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Plataformas para as quais o release publica um instalador.
///
/// Variantes de outras plataformas existem em todas as builds para que a
/// seleção de assets seja testável (e para o mesmo código compilar em todos
/// os alvos), por isso o `allow` abaixo.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    LinuxAppImage,
    LinuxDeb,
    LinuxRpm,
    LinuxTar,
    WindowsMsi,
    MacDmg,
}

impl Platform {
    pub fn asset_extension(self) -> &'static str {
        match self {
            Self::LinuxAppImage => ".appimage",
            Self::LinuxDeb => ".deb",
            Self::LinuxRpm => ".rpm",
            Self::LinuxTar => ".tar.xz",
            Self::WindowsMsi => ".msi",
            Self::MacDmg => ".dmg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LinuxAppImage => "AppImage",
            Self::LinuxDeb => "pacote .deb",
            Self::LinuxRpm => "pacote .rpm",
            Self::LinuxTar => "Linux portátil",
            Self::WindowsMsi => "Windows",
            Self::MacDmg => "macOS",
        }
    }
}

#[cfg(target_os = "windows")]
pub fn detect_platform() -> Platform {
    Platform::WindowsMsi
}

#[cfg(target_os = "macos")]
pub fn detect_platform() -> Platform {
    Platform::MacDmg
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn detect_platform() -> Platform {
    if std::env::var_os("APPIMAGE").is_some_and(|value| !value.is_empty()) {
        return Platform::LinuxAppImage;
    }
    // Decide pelo executável que está rodando, não por uma cópia fixa em
    // /usr/bin: numa máquina com as duas, /usr/bin pode ser o pacote errado.
    let Ok(exe) = std::env::current_exe() else {
        return Platform::LinuxTar;
    };
    // Resolve symlinks para consultar o arquivo real no dpkg/rpm.
    let path = fs::canonicalize(&exe).unwrap_or(exe);
    let Some(path) = path.to_str() else {
        return Platform::LinuxTar;
    };
    if command_succeeds("dpkg", &["-S", path]) {
        return Platform::LinuxDeb;
    }
    if command_succeeds("rpm", &["-qf", path]) {
        return Platform::LinuxRpm;
    }
    Platform::LinuxTar
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
pub fn detect_platform() -> Platform {
    Platform::LinuxTar
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Interpreta `major.minor.patch`, com ou sem o prefixo `v`.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let text = text
        .strip_prefix('v')
        .or_else(|| text.strip_prefix('V'))
        .unwrap_or(text);
    let mut parts = text.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

pub fn check_due(last_check: i64, now: i64) -> bool {
    last_check <= 0 || now.saturating_sub(last_check) >= CHECK_INTERVAL_SECS
}

/// Faz o parsing do JSON de um release. Retorna `None` para pré-releases.
pub fn parse_release(body: &str) -> Result<Option<Release>, String> {
    let raw: RawRelease =
        serde_json::from_str(body).map_err(|err| format!("JSON de release inválido: {err}"))?;
    if raw.prerelease {
        return Ok(None);
    }
    let tag_name = raw.tag_name.trim().to_owned();
    if tag_name.is_empty() {
        return Err("Release sem tag_name".to_owned());
    }
    let version = tag_name
        .strip_prefix('v')
        .or_else(|| tag_name.strip_prefix('V'))
        .unwrap_or(&tag_name)
        .to_owned();
    if parse_version(&version).is_none() {
        return Err(format!("Versão de release inválida: {tag_name}"));
    }
    let html_url = raw.html_url.trim().to_owned();
    if html_url.is_empty() {
        return Err("Release sem html_url".to_owned());
    }
    let assets = raw
        .assets
        .into_iter()
        .filter(|asset| {
            !asset.name.trim().is_empty() && !asset.browser_download_url.trim().is_empty()
        })
        .map(|asset| Asset {
            name: asset.name,
            url: asset.browser_download_url,
            size: asset.size,
        })
        .collect();
    Ok(Some(Release {
        version,
        tag_name,
        html_url,
        assets,
    }))
}

/// Tokens de arquitetura aceitos nos nomes de asset da plataforma atual.
fn arch_tokens() -> &'static [&'static str] {
    match std::env::consts::ARCH {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    }
}

pub fn asset_for(assets: &[Asset], platform: Platform) -> Option<&Asset> {
    let extension = platform.asset_extension();
    let matching: Vec<&Asset> = assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().ends_with(extension))
        .collect();
    // Windows (MSI x64) e macOS (DMG universal) têm um asset só por release.
    if matches!(platform, Platform::WindowsMsi | Platform::MacDmg) {
        return matching.into_iter().next();
    }
    // Linux: prefere o asset da arquitetura atual quando o release traz mais
    // de uma; sem correspondência, cai no primeiro com a extensão.
    let tokens = arch_tokens();
    if !tokens.is_empty() {
        if let Some(asset) = matching.iter().copied().find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            tokens.iter().any(|token| name.contains(token))
        }) {
            return Some(asset);
        }
    }
    matching.into_iter().next()
}

/// Escolhe o asset da plataforma e devolve um erro legível quando não existe.
pub fn select_asset(release: &Release, platform: Platform) -> Result<&Asset, String> {
    asset_for(&release.assets, platform).ok_or_else(|| {
        format!(
            "O release v{} não tem instalador para {} ({})",
            release.version,
            platform.label(),
            platform.asset_extension()
        )
    })
}

/// Valida a URL de download. Espelhos locais só são aceitos quando
/// `SIMION_UPDATE_API` está definido (usado por testes).
pub fn asset_url_allowed(url: &str) -> bool {
    let allow_local = std::env::var_os("SIMION_UPDATE_API").is_some();
    asset_url_allowed_with(url, allow_local)
}

fn asset_url_allowed_with(url: &str, allow_local: bool) -> bool {
    let url = url.to_ascii_lowercase();
    if url.starts_with(GITHUB_DOWNLOAD_PREFIX) || url.starts_with(GITHUB_OBJECTS_PREFIX) {
        return true;
    }
    if !allow_local {
        return false;
    }
    local_url_prefix(&url, "http://127.0.0.1") || local_url_prefix(&url, "http://localhost")
}

fn local_url_prefix(url: &str, base: &str) -> bool {
    url.strip_prefix(base)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with(':'))
}

/// Reduz o nome do asset a um nome de arquivo seguro, sem diretórios.
pub fn sanitize_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let mut clean = String::with_capacity(base.len());
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            clean.push(ch);
        } else {
            clean.push('_');
        }
    }
    if clean.is_empty() || clean.trim_matches('.').is_empty() {
        "simion-update".to_owned()
    } else {
        clean.chars().take(180).collect()
    }
}

pub fn user_agent() -> String {
    format!("Simion/{}", env!("CARGO_PKG_VERSION"))
}

/// Endpoint da API. `SIMION_UPDATE_API` pode apontar para um servidor local,
/// completo (`.../releases/latest`) ou apenas a raiz (`http://127.0.0.1:8080`).
pub fn api_url() -> String {
    let Some(base) = std::env::var("SIMION_UPDATE_API").ok() else {
        return RELEASES_API.to_owned();
    };
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return RELEASES_API.to_owned();
    }
    // Uma URL com caminho próprio é usada como endpoint completo; uma raiz
    // (ex.: http://127.0.0.1:8080) recebe o caminho canônico do GitHub.
    let after_scheme = base.split_once("://").map(|(_, rest)| rest).unwrap_or(base);
    let has_path = after_scheme
        .find('/')
        .is_some_and(|slash| !after_scheme[slash + 1..].is_empty());
    if has_path {
        base.to_owned()
    } else {
        format!("{base}/repos/{REPO}/releases/latest")
    }
}

fn check_agent() -> ureq::Agent {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(CHECK_TIMEOUT))
        .build()
        .into();
    agent
}

fn download_agent() -> ureq::Agent {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .build()
        .into();
    agent
}

/// Consulta a API e retorna o release apenas quando ele é mais novo que o app.
pub fn check_latest() -> Result<Option<Release>, String> {
    let body = check_agent()
        .get(&api_url())
        .header("User-Agent", user_agent())
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| format!("Falha ao consultar o GitHub: {err}"))?
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("Falha ao ler a resposta do GitHub: {err}"))?;

    let Some(release) = parse_release(&body)? else {
        return Ok(None);
    };
    if is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

/// Baixa o asset para a pasta temporária do Simion e devolve o caminho.
///
/// `progress` recebe valores de 0 a 1. O limite de 200 MB é aplicado mesmo
/// quando o servidor não informa `Content-Length`.
pub fn download_asset(asset: &Asset, mut progress: impl FnMut(f32)) -> Result<PathBuf, String> {
    if !asset_url_allowed(&asset.url) {
        return Err(format!("URL de download não permitida: {}", asset.url));
    }
    if asset.size > MAX_DOWNLOAD_BYTES {
        return Err(format!(
            "O instalador tem {} MB; o limite é 200 MB",
            asset.size / (1024 * 1024)
        ));
    }

    let dir = std::env::temp_dir().join("simion-update");
    fs::create_dir_all(&dir).map_err(|err| format!("Não foi possível criar {dir:?}: {err}"))?;
    let dest = dir.join(sanitize_file_name(&asset.name));

    let mut response = download_agent()
        .get(&asset.url)
        .header("User-Agent", user_agent())
        .call()
        .map_err(|err| format!("Falha ao baixar o instalador: {err}"))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(asset.size);

    let mut reader = response.body_mut().as_reader();
    let mut file =
        fs::File::create(&dest).map_err(|err| format!("Não foi possível criar {dest:?}: {err}"))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    progress(0.0);
    let read_result = loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break Ok(()),
            Ok(read) => read,
            Err(err) => break Err(format!("Falha durante o download: {err}")),
        };
        written += read as u64;
        if written > MAX_DOWNLOAD_BYTES {
            break Err("Download grande demais (limite de 200 MB)".to_owned());
        }
        if let Err(err) = file.write_all(&buffer[..read]) {
            break Err(format!("Falha ao gravar o download: {err}"));
        }
        if total > 0 {
            progress((written as f32 / total as f32).clamp(0.0, 1.0));
        }
    };
    let flush_result = read_result.and_then(|()| {
        file.flush()
            .map_err(|err| format!("Falha ao finalizar o download: {err}"))
    });
    drop(file);
    if let Err(err) = flush_result {
        // Nunca deixa arquivo parcial para trás: o instalador seria inválido.
        let _ = fs::remove_file(&dest);
        return Err(err);
    }
    if written == 0 {
        let _ = fs::remove_file(&dest);
        return Err("Download vazio: o servidor não enviou nenhum byte".to_owned());
    }
    progress(1.0);
    Ok(dest)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// O instalador do sistema foi aberto; o usuário conclui a instalação.
    InstallerOpened,
    /// O novo binário foi iniciado; o processo atual já foi encerrado.
    Restarted,
}

/// Aplica um download já concluído, escolhendo o método pelo nome do arquivo.
pub fn apply_update(path: &Path) -> Result<ApplyOutcome, String> {
    if !path.is_file() {
        return Err(format!("Instalador não encontrado: {}", path.display()));
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if name.ends_with(".appimage") {
        apply_appimage(path)?;
        Ok(ApplyOutcome::Restarted)
    } else if name.ends_with(".tar.xz") {
        apply_portable_tar(path)?;
        Ok(ApplyOutcome::Restarted)
    } else if name.ends_with(".deb")
        || name.ends_with(".rpm")
        || name.ends_with(".msi")
        || name.ends_with(".dmg")
    {
        launch_installer(path)?;
        Ok(ApplyOutcome::InstallerOpened)
    } else {
        Err(format!(
            "Formato de atualização não reconhecido: {}",
            path.display()
        ))
    }
}

fn apply_appimage(download: &Path) -> Result<(), String> {
    let target = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "APPIMAGE não está definido; não sei qual AppImage substituir".to_owned())?;

    replace_file(download, &target)?;
    Command::new(&target)
        .spawn()
        .map_err(|err| format!("Falha ao reiniciar o AppImage: {err}"))?;
    std::process::exit(0);
}

fn apply_portable_tar(download: &Path) -> Result<(), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("simion-extract-{}-{stamp}", std::process::id()));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Não foi possível criar a pasta temporária: {err}"))?;

    let status = Command::new("tar")
        .arg("-xJf")
        .arg(download)
        .arg("-C")
        .arg(&dir)
        .status()
        .map_err(|err| format!("Não foi possível executar tar: {err}"))?;
    if !status.success() {
        let _ = fs::remove_dir_all(&dir);
        return Err("Falha ao extrair o pacote .tar.xz".to_owned());
    }

    let Some(binary) = find_named(&dir, "simion", 0) else {
        let _ = fs::remove_dir_all(&dir);
        return Err("O pacote .tar.xz não contém o binário simion".to_owned());
    };
    let current = std::env::current_exe()
        .map_err(|err| format!("Não foi possível localizar o executável atual: {err}"))?;
    replace_file(&binary, &current)?;
    let _ = fs::remove_dir_all(&dir);

    Command::new(&current)
        .spawn()
        .map_err(|err| format!("Falha ao reiniciar o Simion: {err}"))?;
    std::process::exit(0);
}

fn find_named(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    if depth > 6 {
        return None;
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            directories.push(path);
        } else if path.file_name().is_some_and(|file| file == name) {
            return Some(path);
        }
    }
    for directory in directories {
        if let Some(found) = find_named(&directory, name, depth + 1) {
            return Some(found);
        }
    }
    None
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    // Se o destino é um symlink (ex.: /usr/local/bin/simion -> .../simion),
    // substitui o alvo real e preserva o link. Se o alvo ainda não existe,
    // mantém o caminho original.
    let resolved = fs::symlink_metadata(destination)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
        .then(|| fs::canonicalize(destination).ok())
        .flatten();
    let destination = resolved.as_deref().unwrap_or(destination);

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| format!("Destino inválido: {}", destination.display()))?;
    let staged = parent.join(format!("{file_name}.new"));

    fs::copy(source, &staged)
        .map_err(|err| format!("Não foi possível copiar para {}: {err}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).map_err(|err| {
            format!(
                "Não foi possível tornar {} executável: {err}",
                staged.display()
            )
        })?;
    }
    if let Err(err) = fs::rename(&staged, destination) {
        let _ = fs::remove_file(&staged);
        return Err(format!(
            "Não foi possível substituir {}: {err}",
            destination.display()
        ));
    }
    Ok(())
}

fn launch_installer(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("msiexec")
            .arg("/i")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Falha ao abrir o instalador MSI: {err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Falha ao abrir o DMG: {err}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Falha ao abrir o instalador: {err}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Abertura automática de instalador não é suportada nesta plataforma".to_owned())
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading { release: Release, progress: f32 },
    Ready { release: Release, path: PathBuf },
    Error(String),
}

impl UpdateStatus {
    pub fn release(&self) -> Option<&Release> {
        match self {
            Self::Available(release)
            | Self::Downloading { release, .. }
            | Self::Ready { release, .. } => Some(release),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdateState {
    pub status: UpdateStatus,
    pub last_check: i64,
    /// `true` quando a checagem foi automática; controla o silêncio dos erros.
    pub auto: bool,
    /// Preferência do usuário (`settings.auto_update_check`).
    pub auto_enabled: bool,
    /// `SIMION_NO_UPDATE_CHECK` desliga a checagem automática e trava a preferência.
    pub env_disabled: bool,
    /// Versão cuja faixa o usuário escondeu com “Depois”.
    pub dismissed: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            last_check: 0,
            auto: true,
            auto_enabled: true,
            env_disabled: false,
            dismissed: None,
        }
    }
}

#[derive(Debug)]
pub enum UpdateMsg {
    CheckResult(Result<Option<Release>, String>, bool),
    DownloadProgress(Release, f32),
    DownloadReady(Release, PathBuf),
    DownloadError(Release, String, bool),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RELEASE: &str = r#"
    {
      "tag_name": "v0.1.10",
      "html_url": "https://github.com/cleisonsantos/simion/releases/tag/v0.1.10",
      "prerelease": false,
      "assets": [
        {
          "name": "Simion-0.1.10-x86_64.AppImage",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/Simion-0.1.10-x86_64.AppImage",
          "size": 15000000
        },
        {
          "name": "simion_0.1.10_amd64.deb",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion_0.1.10_amd64.deb",
          "size": 12000000
        },
        {
          "name": "simion-0.1.10-1.x86_64.rpm",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion-0.1.10-1.x86_64.rpm",
          "size": 12000000
        },
        {
          "name": "simion_0.1.10_linux_x86_64.tar.xz",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion_0.1.10_linux_x86_64.tar.xz",
          "size": 9000000
        },
        {
          "name": "Simion-Setup-0.1.10-x64.msi",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/Simion-Setup-0.1.10-x64.msi",
          "size": 13000000
        },
        {
          "name": "Simion-0.1.10-universal.dmg",
          "browser_download_url": "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/Simion-0.1.10-universal.dmg",
          "size": 14000000
        }
      ]
    }
    "#;

    #[test]
    fn parse_version_accepts_v_prefix_and_plain_versions() {
        assert_eq!(parse_version("v0.1.4"), Some((0, 1, 4)));
        assert_eq!(parse_version("0.1.10"), Some((0, 1, 10)));
        assert_eq!(parse_version(" V12.34.56 "), Some((12, 34, 56)));
    }

    #[test]
    fn parse_version_rejects_invalid_values() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v0.1"), None);
        assert_eq!(parse_version("v0.1.2.3"), None);
        assert_eq!(parse_version("v0.x.2"), None);
        assert_eq!(parse_version("v-1.0.0"), None);
    }

    #[test]
    fn is_newer_compares_numbers_not_text() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("v0.2.0", "0.1.99"));
        assert!(!is_newer("0.1.9", "0.1.10"));
        assert!(!is_newer("0.1.4", "0.1.4"));
        assert!(!is_newer("invalida", "0.1.4"));
    }

    #[test]
    fn parses_release_json_with_assets() {
        let release = parse_release(SAMPLE_RELEASE).unwrap().unwrap();
        assert_eq!(release.version, "0.1.10");
        assert_eq!(release.tag_name, "v0.1.10");
        assert_eq!(
            release.html_url,
            "https://github.com/cleisonsantos/simion/releases/tag/v0.1.10"
        );
        assert_eq!(release.assets.len(), 6);
        assert_eq!(release.assets[0].size, 15_000_000);
        assert!(release.assets[0].url.starts_with(GITHUB_DOWNLOAD_PREFIX));
    }

    #[test]
    fn rejects_prerelease() {
        let json = r#"{
            "tag_name": "v0.2.0-rc.1",
            "html_url": "https://github.com/cleisonsantos/simion/releases/tag/v0.2.0-rc.1",
            "prerelease": true,
            "assets": []
        }"#;
        assert!(parse_release(json).unwrap().is_none());
    }

    fn release() -> Release {
        parse_release(SAMPLE_RELEASE).unwrap().unwrap()
    }

    #[test]
    fn selects_linux_appimage_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::LinuxAppImage).unwrap();
        assert_eq!(asset.name, "Simion-0.1.10-x86_64.AppImage");
    }

    #[test]
    fn selects_linux_deb_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::LinuxDeb).unwrap();
        assert_eq!(asset.name, "simion_0.1.10_amd64.deb");
    }

    #[test]
    fn selects_linux_rpm_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::LinuxRpm).unwrap();
        assert_eq!(asset.name, "simion-0.1.10-1.x86_64.rpm");
    }

    #[test]
    fn selects_linux_tar_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::LinuxTar).unwrap();
        assert_eq!(asset.name, "simion_0.1.10_linux_x86_64.tar.xz");
    }

    #[test]
    fn prefers_asset_matching_current_architecture() {
        let assets = vec![
            Asset {
                name: "simion_0.1.10_linux_aarch64.tar.xz".to_owned(),
                url: "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion_0.1.10_linux_aarch64.tar.xz".to_owned(),
                size: 9_000_000,
            },
            Asset {
                name: "simion_0.1.10_linux_x86_64.tar.xz".to_owned(),
                url: "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion_0.1.10_linux_x86_64.tar.xz".to_owned(),
                size: 9_000_000,
            },
        ];
        let chosen = asset_for(&assets, Platform::LinuxTar).unwrap();
        if cfg!(target_arch = "x86_64") {
            assert_eq!(chosen.name, "simion_0.1.10_linux_x86_64.tar.xz");
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(chosen.name, "simion_0.1.10_linux_aarch64.tar.xz");
        } else {
            // Outras arquiteturas mantêm a compatibilidade: primeiro asset.
            assert_eq!(chosen.name, "simion_0.1.10_linux_aarch64.tar.xz");
        }
    }

    #[test]
    fn selects_windows_msi_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::WindowsMsi).unwrap();
        assert_eq!(asset.name, "Simion-Setup-0.1.10-x64.msi");
    }

    #[test]
    fn selects_macos_dmg_asset() {
        let release = release();
        let asset = select_asset(&release, Platform::MacDmg).unwrap();
        assert_eq!(asset.name, "Simion-0.1.10-universal.dmg");
    }

    #[test]
    fn missing_platform_asset_is_a_clear_error() {
        let release = Release {
            version: "0.1.10".to_owned(),
            tag_name: "v0.1.10".to_owned(),
            html_url: "https://example.invalid".to_owned(),
            assets: vec![Asset {
                name: "simion_0.1.10_amd64.deb".to_owned(),
                url: "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion_0.1.10_amd64.deb".to_owned(),
                size: 1,
            }],
        };
        let err = select_asset(&release, Platform::MacDmg).unwrap_err();
        assert!(err.contains("não tem instalador"));
        assert!(err.contains(".dmg"));
    }

    #[test]
    fn accepts_github_download_urls() {
        assert!(asset_url_allowed(
            "https://github.com/cleisonsantos/simion/releases/download/v0.1.10/simion.tar.xz"
        ));
        assert!(asset_url_allowed(
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/123"
        ));
    }

    #[test]
    fn rejects_strange_asset_urls() {
        assert!(!asset_url_allowed("https://evil.example/simion.tar.xz"));
        assert!(!asset_url_allowed(
            "https://github.com/outro/repo/releases/download/v0.1.10/simion.tar.xz"
        ));
        assert!(!asset_url_allowed_with(
            "http://127.0.0.1:8000/simion.tar.xz",
            false
        ));
        assert!(!asset_url_allowed(
            "https://github.com.evil.example/simion.tar.xz"
        ));
    }

    #[test]
    fn local_urls_only_with_local_mode() {
        assert!(asset_url_allowed_with(
            "http://127.0.0.1:8000/simion.tar.xz",
            true
        ));
        assert!(asset_url_allowed_with(
            "http://localhost:8000/simion.tar.xz",
            true
        ));
        assert!(!asset_url_allowed_with(
            "http://localhost.evil.example/simion.tar.xz",
            true
        ));
    }

    #[test]
    fn sanitizes_download_file_names() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(
            sanitize_file_name("pasta\\simion_0.1.10_amd64.deb"),
            "simion_0.1.10_amd64.deb"
        );
        assert_eq!(
            sanitize_file_name("Simion 0.1.10.AppImage"),
            "Simion_0.1.10.AppImage"
        );
        assert_eq!(sanitize_file_name(".."), "simion-update");
        assert_eq!(sanitize_file_name("/"), "simion-update");
    }

    #[test]
    fn check_due_uses_24_hour_window() {
        let now = 1_000_000;
        assert!(check_due(0, now));
        assert!(check_due(now - CHECK_INTERVAL_SECS - 1, now));
        assert!(!check_due(now - CHECK_INTERVAL_SECS + 1, now));
        assert!(!check_due(now + 60, now));
    }
}
