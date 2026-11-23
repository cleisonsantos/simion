//! Garante que o instalador Windows nao mostre o contrato de licenca
//! placeholder do WiX ("Lorem ipsum") — issue #77.

use std::path::PathBuf;

fn root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name)
}

#[test]
fn msi_license_dialog_uses_the_real_mit_license() {
    let wxs = std::fs::read_to_string(root("wix/main.wxs")).expect("wix/main.wxs existe");
    assert!(
        wxs.contains(r#"Id="WixUILicenseRtf""#),
        "o WixUI deve apontar para um RTF proprio"
    );

    let declared = wxs
        .split("WixUILicenseRtf")
        .nth(1)
        .expect("variavel declarada")
        .split('/')
        .next()
        .unwrap_or_default();
    assert!(declared.contains("License.rtf"), "caminho do RTF declarado");

    let rtf = std::fs::read_to_string(root("wix/License.rtf")).expect("wix/License.rtf existe");
    assert!(rtf.starts_with(r"{\rtf1"), "precisa ser um RTF valido");
    assert!(
        rtf.contains("MIT License"),
        "o texto deve ser a licenca MIT"
    );
    assert!(
        !rtf.to_ascii_lowercase().contains("lorem ipsum"),
        "nenhum texto placeholder no contrato"
    );
}
