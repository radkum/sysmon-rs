use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use std::io::Stdout;

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("client") => produce_client()?,
        Some("driver") => produce_driver()?,
        Some("clean") => clean()?,
        Some("sign") => sign(
            "target\\release\\sysmon.sys",
            "sysmon-km\\DriverCertificate.cer",
        )?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    println!("{:?}", env::args());
    eprintln!(
        "Tasks:
         - driver: builds application and man pages
"
    )
}

fn clean() -> Result<(), DynError> {
    let _ = fs::remove_dir_all(&release_dir());
    //fs::create_dir_all(&release_dir())?;

    Ok(())
}

fn produce_driver() -> Result<(), DynError> {
    build_release_binary("sysmon")?;
    std::fs::rename("target\\release\\sysmon.dll", "target\\release\\sysmon.sys")?;
    sign(
        "target\\release\\sysmon.sys",
        "sysmon-km\\DriverCertificate.cer",
    )?;
    Ok(())
}

fn sign(driver_path: &str, cert_path: &str) -> Result<(), DynError> {
    let (code, output, error) = run_script::run_script!(
        r#"
    call "%ProgramFiles(x86)%\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",

    # Sign the driver
    signtool sign /fd SHA256 /a /v /s PrivateCertStore /n DriverCertificate /t http://timestamp.digicert.com %TARGET_PATH%/%DRIVER_NAME%.sys
         "#
    )
        .unwrap();
    // let mut command = Command::new("cmd.exe");
    // command.current_dir("target\\release");
    // command.args(["\"%ProgramFiles(x86)%\\Windows Kits\\10\\bin\\10.0.26100.0\\x64\\signtool.exe\"",
    //     "sign",
    //     "/fd",
    //     "SHA256",
    //     "/a",
    //     "/v",
    //     "/s",
    //     "PrivateCertStore",
    //     "/n",
    //     "DriverCertificate.cer",
    //     "/t",
    //     "http://timestamp.digicert.com",
    //     "sysmon.sys"]);
    //
    // command.stdout(std::io::stdout());
    // let s = command.output()?;
    // println!("Statsu: {s:?}");

    //let output = shutil::pipe(vec![
        // vec![
        //     "call",
        //     "\"%ProgramFiles(x86)%\\Microsoft Visual \
        //      Studio\\2019\\Professional\\VC\\Auxiliary\\Build\\vcvars64.bat\"",
        // ],
        //vec!["if", "not", cert_path, "( makecert -r -pe -ss PrivateCertStore -n CN=DriverCertificate DriverCertificate.cer ) else ( echo Certificate already exists. )", "1"],
        // vec![
        //     "signtool",
        //     "sign",
        //     "/fd",
        //     "SHA256",
        //     "/a",
        //     "/v",
        //     "/s",
        //     "PrivateCertStore",
        //     "/n",
        //     cert_path,
        //     "/t",
        //     "http://timestamp.digicert.com",
        //     driver_path,
        // ],
    //]);
    //println!("{}", output.unwrap());
    Ok(())
}

fn produce_client() -> Result<(), DynError> {
    build_release_binary("sysmon-client")?;
    Ok(())
}

fn build_release_binary(project: &str) -> Result<(), DynError> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(&["build", "--release", "-p", project])
        .status()?;

    if !status.success() {
        Err("cargo build failed")?;
    }

    Ok(())
}
// fn build_release_binary(project: &str) -> Result<(), DynError> {
// if Command::new("strip")
//     .arg("--version")
//     .stdout(Stdio::null())
//     .status()
//     .is_ok()
// {
//     eprintln!("stripping the binary");
//     let status = Command::new("strip").arg(&dst).status()?;
//     if !status.success() {
//         Err("strip failed")?;
//     }
// } else {
//     eprintln!("no `strip` utility found")
// }
// Ok(())
// }

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn release_dir() -> PathBuf {
    project_root().join("target/release")
}
