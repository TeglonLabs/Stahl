use std::{error::Error, path::PathBuf, process::Command};

use cargo_metadata::{Message, MetadataCommand, Package};
use std::process::Stdio;

fn package_contains_dependency_on_stahl(packages: &[Package]) -> Option<&Package> {
    packages.iter().find(|x| x.name == "stahl-core")
}

/*
TODO:
- Desired output directory / do not copy to native automatically
- Specify target architecture
*/

pub fn stahl_home() -> Option<PathBuf> {
    std::env::var("STAHL_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let home = home::home_dir();

            home.map(|mut x: PathBuf| {
                x.push(".stahl");

                // Just go ahead and initialize the directory, even though
                // this is probably not the best place to do this. This almost
                // assuredly could be lifted out of this check since failing here
                // could cause some annoyance.
                if !x.exists() {
                    if let Err(_) = std::fs::create_dir(&x) {
                        eprintln!("Unable to create stahl home directory {:?}", x)
                    }
                }

                x
            })
        })
}

pub fn run(args: Vec<String>, env_vars: Vec<(String, String)>) -> Result<(), Box<dyn Error>> {
    let mut stahl_home = stahl_home().expect("Unable to find STAHL_HOME");

    stahl_home.push("native");

    // --manifest-path
    let mut metadata_command = MetadataCommand::new();

    for pair in args.chunks(2) {
        match &pair {
            &[arg, path] if arg == "--manifest-path" => {
                metadata_command.manifest_path(path);
            }
            _ => {}
        }
    }

    let metadata = metadata_command.exec()?;

    let package = match metadata.root_package() {
        Some(p) => p,
        None => return Err("cargo stahl-lib must be run from within a crate".into()),
    };

    println!("Attempting to install: {:#?}", package.name);

    if package_contains_dependency_on_stahl(&metadata.packages).is_none() {
        return Err(
            "Cannot install package as a stahl dylib - does not contain a dependency on stahl!"
                .into(),
        );
    }

    let mut command = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--message-format=json-render-diagnostics",
        ])
        .args(args)
        .envs(env_vars)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let reader = std::io::BufReader::new(command.stdout.take().unwrap());

    let artifacts = cargo_metadata::Message::parse_stream(reader).filter_map(|x| {
        if let Ok(Message::CompilerArtifact(artifact)) = x {
            Some(artifact)
        } else {
            None
        }
    });

    for last in artifacts {
        if last
            .target
            .kind
            .iter()
            .find(|x| x.as_str() == "cdylib")
            .is_some()
        {
            for file in last.filenames {
                if matches!(file.extension(), Some("so") | Some("dylib") | Some("lib")) {
                    println!("Found a cdylib!");
                    let filename = file.file_name().unwrap();

                    stahl_home.push(filename);

                    println!("Copying {} to {}", file, &stahl_home.to_str().unwrap());

                    if stahl_home.exists() {
                        std::fs::remove_file(&stahl_home)
                            .expect("Unable to delete the existing dylib");
                    }

                    std::fs::copy(file, &stahl_home).unwrap();

                    stahl_home.pop();
                    break;
                }
            }
        }
    }

    println!("Done!");

    command.wait().expect("Couldn't get cargo's exit status");

    Ok(())
}
