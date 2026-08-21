use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

const APP_NAME: &str = "SEE";
const APP_NAME_DEV: &str = "SEE (Dev)";
const BUNDLE_ID_DEV: &str = "com.advait.see.dev";
const SIGNING_IDENTITY: &str = "Developer ID Application: Advait Bansode (N8K96VJAHS)";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("dev") => dev()?,
        Some("release") => release()?,
        Some(cmd) => bail!("Unknown command: {}", cmd),
        None => bail!("Usage: cargo xtask [dev|release]"),
    }

    Ok(())
}

fn dev() -> Result<()> {
    let project_root = project_root()?;
    env::set_current_dir(&project_root)?;

    let bundle_path = project_root.join(format!("target/debug/bundle/osx/{}.app", APP_NAME));

    println!("==> Building {} (dev)...", APP_NAME);
    run_command("cargo", &["bundle"])?;

    // Replace icon with dev icon
    println!("==> Applying dev configuration...");
    let dev_icon = project_root.join("assets/icon-dev.icns");
    let bundle_icon = bundle_path.join("Contents/Resources/icon.icns");
    fs::copy(&dev_icon, &bundle_icon)?;

    // Update Info.plist with dev name and identifier
    let plist_path = bundle_path.join("Contents/Info.plist");
    let plist_content = fs::read_to_string(&plist_path)?;
    let plist_content = plist_content
        .replace(
            "<string>SEE</string>",
            &format!("<string>{}</string>", APP_NAME_DEV),
        )
        .replace(
            "<string>com.advait.see</string>",
            &format!("<string>{}</string>", BUNDLE_ID_DEV),
        );
    fs::write(&plist_path, plist_content)?;

    // Ad-hoc sign for local use
    println!("==> Signing (ad-hoc)...");
    run_command("codesign", &[
        "--force",
        "--deep",
        "--sign", "-",
        bundle_path.to_str().unwrap(),
    ])?;

    println!("==> Launching {}...", APP_NAME_DEV);
    let executable = bundle_path.join("Contents/MacOS/see");
    run_command(executable.to_str().unwrap(), &[])?;

    Ok(())
}

fn release() -> Result<()> {
    let project_root = project_root()?;
    env::set_current_dir(&project_root)?;

    // Load .env file
    let env_vars = load_env(&project_root)?;

    let api_key = env_vars.get("APPLE_API_KEY")
        .context("APPLE_API_KEY not set in .env")?;
    let api_key_id = env_vars.get("APPLE_API_KEY_ID")
        .context("APPLE_API_KEY_ID not set in .env")?;
    let api_issuer = env_vars.get("APPLE_API_ISSUER")
        .context("APPLE_API_ISSUER not set in .env")?;

    // Get version from Cargo.toml
    let version = get_version(&project_root)?;
    let bundle_path = project_root.join(format!("target/release/bundle/osx/{}.app", APP_NAME));
    let dmg_path = project_root.join(format!("target/release/{}-{}.dmg", APP_NAME, version));
    let staging_path = project_root.join("target/release/dmg-staging");

    println!("==> Building {} v{}...", APP_NAME, version);
    run_command("cargo", &["bundle", "--release"])?;

    println!("==> Signing app bundle...");
    run_command("codesign", &[
        "--force",
        "--options", "runtime",
        "--deep",
        "--sign", SIGNING_IDENTITY,
        bundle_path.to_str().unwrap(),
    ])?;

    println!("==> Verifying signature...");
    run_command("codesign", &[
        "--verify",
        "--verbose",
        bundle_path.to_str().unwrap(),
    ])?;

    println!("==> Creating DMG...");
    let _ = fs::remove_dir_all(&staging_path);
    fs::create_dir_all(&staging_path)?;

    run_command("cp", &[
        "-R",
        bundle_path.to_str().unwrap(),
        staging_path.to_str().unwrap(),
    ])?;

    std::os::unix::fs::symlink("/Applications", staging_path.join("Applications"))?;

    let _ = fs::remove_file(&dmg_path);
    run_command("hdiutil", &[
        "create",
        "-volname", APP_NAME,
        "-srcfolder", staging_path.to_str().unwrap(),
        "-ov",
        "-format", "UDZO",
        dmg_path.to_str().unwrap(),
    ])?;

    fs::remove_dir_all(&staging_path)?;

    println!("==> Signing DMG...");
    run_command("codesign", &[
        "--force",
        "--sign", SIGNING_IDENTITY,
        dmg_path.to_str().unwrap(),
    ])?;

    println!("==> Notarizing DMG...");
    run_command("xcrun", &[
        "notarytool", "submit",
        dmg_path.to_str().unwrap(),
        "--key", api_key,
        "--key-id", api_key_id,
        "--issuer", api_issuer,
        "--wait",
    ])?;

    println!("==> Stapling notarization ticket...");
    run_command("xcrun", &["stapler", "staple", dmg_path.to_str().unwrap()])?;

    println!("==> Done!");
    println!("Release ready: {}", dmg_path.display());

    Ok(())
}

fn project_root() -> Result<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Ok(Path::new(manifest_dir).parent().unwrap().to_path_buf())
}

fn load_env(project_root: &Path) -> Result<HashMap<String, String>> {
    let env_path = project_root.join(".env");
    let mut vars = HashMap::new();

    if env_path.exists() {
        let contents = fs::read_to_string(&env_path)?;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                vars.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    Ok(vars)
}

fn get_version(project_root: &Path) -> Result<String> {
    let cargo_toml = fs::read_to_string(project_root.join("Cargo.toml"))?;
    let parsed: toml::Value = cargo_toml.parse()?;

    let version = parsed
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .context("Could not find version in Cargo.toml")?;

    Ok(version.to_string())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to run: {} {:?}", cmd, args))?;

    if !status.success() {
        bail!("Command failed: {} {:?}", cmd, args);
    }

    Ok(())
}
