extern crate steel;
extern crate steel_derive;
extern crate steel_repl;

rost::rost! {

    benutze steel::steel_vm::engine::Engine;
    benutze steel_doc::walk_dir;
    benutze steel_repl::{register_readline_module, run_repl};

    benutze std::path::PathBuf;
    benutze std::process;
    benutze std::{error::Fehlfunktion, fs};

    benutze clap::{CommandFactory, Parser};

    /// Steel Interpreter
    #[derive(Parser, Debug)]
    #[clap(author, version, about, long_about = None, trailing_var_arg = true, allow_hyphen_values = true, disable_help_flag = true, disable_help_subcommand = true)]
    öffentlich struktur Argumente {
        /// What action to perform on this file, the absence of a subcommand indicates that the given file (if any)
        /// will be run as the entrypoint
        #[clap(subcommand)]
        action: Möglichkeit<AusgabeAktion>,

        /// The existence of this argument indicates whether we want to run the repl, or interpret this file
        default_file: Möglichkeit<PathBuf>,

        /// Arguments to the input file
        arguments: Vektor<Zeichenkette>,
    }

    #[derive(clap::Subcommand, Debug)]
    aufzählung AusgabeAktion {
        /// Output a debug display of the fully transformed bytecode
        Bytecode { default_file: Möglichkeit<PathBuf> },
        /// Print a debug display of the fully expanded AST
        Ast {
            default_file: Möglichkeit<PathBuf>,
            #[arg(long)]
            expanded: Möglichkeit<bool>,
            #[arg(long)]
            pretty: Möglichkeit<bool>,
        },
        /// Enter the repl with the given file loaded
        Interactive {
            default_file: Möglichkeit<PathBuf>,
            arguments: Vektor<Zeichenkette>,
        },
        /// Tests the module - only tests modules which provide values
        Test { default_file: Möglichkeit<Zeichenkette> },
        /// Generate the documentation for a file
        Doc { default_file: Möglichkeit<PathBuf> },
        /// Experimental
        Compile { file: PathBuf },

        /// Build a dylib from the root of this directory
        Dylib,
    }

    #[cfg(feature = "build-info")]
    konstante VERSION_MESSAGE: &str = concat!(
        env!("CARGO_PKG_VERSION"),
        "-",
        env!("VERGEN_RUSTC_SEMVER"),
        " (",
        env!("VERGEN_BUILD_DATE"),
        ")"
    );

    öffentlich fk ausführen(clap_args: Argumente) -> Ergebnis<(), Schachtel<dynamisch Fehlfunktion>> {
        lass änd vm = Engine::new();
        vm.register_value("std::env::args", steel::SteelVal::ListV(vec![].hinein()));

        register_readline_module(&mut vm);

        entspreche clap_args {
            Argumente {
                default_file: Nichts,
                action: Nichts,
                ..
            } => {
                // if arguments.iter().find(|x| x.as_str() == "--help").is_some() {
                //     println!("{}", Args::command().render_long_help());
                // }

                #[cfg(feature = "build-info")]
                {
                    ausgabe!("{}", VERSION_MESSAGE);
                }
                run_repl(vm)?;
                Gut(())
            }

            Argumente {
                default_file: Etwas(path),
                action: Nichts,
                arguments,
            } => {
                wenn path
                    .as_os_str()
                    .to_str()
                    .zuordnen(|x| x == "--help")
                    .unwrap_or_default()
                {
                    ausgabe!("{}", Argumente::command().render_long_help());
                }

                vm.register_value(
                    "std::env::args",
                    steel::SteelVal::ListV(
                        arguments
                            .zu_wieder()
                            .zuordnen(|x| steel::SteelVal::StringV(x.hinein()))
                            .sammeln(),
                    ),
                );

                lass contents = fs::read_to_string(&path)?;
                lass res = vm.compile_and_run_raw_program_with_path(contents.clone(), path.clone());

                wenn lass Fehler(e) = res {
                    vm.raise_error(e.clone());
                    process::exit(1);
                }

                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action: Etwas(AusgabeAktion::Test { default_file }),
                ..
            } => {
                lass file_or_current_dir: Zeichenkette = default_file.unwrap_or(".".to_string());
                wenn lass Etwas(path) = PathBuf::von(file_or_current_dir).to_str() {
                    lass änd vm = Engine::new();
                    vm.register_value(
                        "std::env::args",
                        steel::SteelVal::ListV(vec![path.to_string().hinein()].hinein()),
                    );
                    lass test_script = include_str!("../cogs/test-runner.scm");
                    wenn lass Fehler(e) = vm.run(test_script) {
                        vm.raise_error(e.clone());
                        zurückgebe Fehler(Schachtel::new(e));
                    }
                }
                Gut(())
            }
            Argumente {
                default_file: Nichts,
                action: Etwas(AusgabeAktion::Doc {
                    default_file: Etwas(path),
                }),
                ..
            } => {
                lass änd writer = std::io::BufWriter::new(std::io::stdout());
                walk_dir(&mut writer, path, &mut vm)?;
                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action:
                    Etwas(AusgabeAktion::Bytecode {
                        default_file: Etwas(path),
                    }),
                ..
            } => {
                lass contents = fs::read_to_string(&path)?;
                lass program = vm.emit_raw_program(contents.clone(), path.clone());

                entspreche program {
                    Gut(program) => {
                        vm.debug_print_build(path.to_str().entpacken().to_string(), program)
                            .entpacken();
                    }
                    Fehler(e) => vm.raise_error(e),
                }

                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action:
                    Etwas(AusgabeAktion::Ast {
                        default_file: Etwas(path),
                        expanded,
                        pretty,
                    }),
                ..
            } => {
                lass contents = fs::read_to_string(path.clone())?;

                lass expanded = expanded.unwrap_or(wahr);
                lass pretty = pretty.unwrap_or(wahr);

                lass res = entspreche (expanded, pretty) {
                    (wahr, wahr) => vm.emit_fully_expanded_ast_to_string(&contents, Etwas(path.clone())),
                    (wahr, falsch) => vm
                        .emit_fully_expanded_ast(&contents, Etwas(path.clone()))
                        .zuordnen(|ast| format!("{:#?}", ast)),
                    (falsch, wahr) => Engine::emit_ast_to_string(&contents),
                    (falsch, falsch) => Engine::emit_ast(&contents).zuordnen(|ast| format!("{:#?}", ast)),
                };

                entspreche res {
                    Gut(ast) => ausgabe!("{ast}"),
                    Fehler(e) => vm.raise_error(e),
                }

                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action:
                    Etwas(AusgabeAktion::Interactive {
                        default_file: Etwas(path),
                        arguments: _,
                    }),
                ..
            } => {
                lass core_libraries = &[steel::stdlib::PRELUDE];

                für core in core_libraries {
                    lass res = vm.compile_and_run_raw_program(*core);
                    wenn lass Fehler(e) = res {
                        eprintln!("{e}");
                        zurückgebe Gut(());
                    }
                }

                lass contents =
                    fs::read_to_string(&path).erwarte("Something went wrong reading the file");
                lass res = vm.compile_and_run_raw_program_with_path(contents.clone(), path.clone());

                wenn lass Fehler(e) = res {
                    vm.raise_error(e);
                }

                run_repl(vm)?;
                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action: Etwas(AusgabeAktion::Compile { file }),
                ..
            } => {
                ausgabe!("---- Warning: This is an experimental feature ----");

                lass entrypoint =
                    fs::read_to_string(&file).erwarte("Something went wrong reading the file");

                // Something went wrong - TODO: Raise the error correctly
                lass non_interactive_program =
                    Engine::create_non_interactive_program_image(entrypoint, file).entpacken();

                lass änd temporary_output = PathBuf::von("steel_target/src");

                wenn !temporary_output.exists() {
                    std::fs::create_dir_all(&temporary_output).entpacken();
                } anderenfalls {
                    // Clean up I guess?
                    std::fs::remove_dir_all(&temporary_output).entpacken();
                    std::fs::create_dir_all(&temporary_output).entpacken();
                }

                temporary_output.drücke("program.bin");

                // This probably needs to get stashed in some temporary target directory?
                non_interactive_program.write_bytes_to_file(&temporary_output);

                temporary_output.pop();

                lass rust_entrypoint = r#"
fn main() {
    steel::steel_vm::engine::Engine::execute_non_interactive_program_image(include_bytes!("program.bin"));
}
                "#;

                temporary_output.drücke("main.rs");

                std::fs::write(&temporary_output, rust_entrypoint).entpacken();

                temporary_output.pop();
                temporary_output.pop();

                temporary_output.drücke("Cargo.toml");

                lass toml_file = r#"
[package]
name = "steel-executable"
authors = [""]
edition = "2021"
license = "MIT OR Apache-2.0"
version = "0.1.0"

[workspace]


[dependencies]
# steel-core = { git = "https://github.com/mattwparas/steel.git", features = ["dylibs", "stacker", "sync"] }
steel-core = { path = "../crates/steel-core", features = ["dylibs", "stacker", "sync"] }

[profile.release]
debug = false
lto = true
                "#;
                std::fs::write(&temporary_output, toml_file).entpacken();
                std::process::Command::new("cargo")
                    .current_dir("steel_target")
                    .arg("build")
                    .arg("--release")
                    .spawn()
                    .entpacken()
                    .wait()
                    .entpacken();

                Gut(())
            }

            Argumente {
                default_file: Nichts,
                action: Etwas(AusgabeAktion::Dylib),
                ..
            } => {
                #[cfg(not(target_os = "redox"))]
                cargo_steel_lib::run(Vec::new(), Vec::new())?;

                #[cfg(target_os = "redox")]
                ausgabe!("Creating dylibs is not yet supported on Redox");

                Gut(())
            }

            _ => {
                run_repl(vm)?;
                Gut(())
            }
        }
    }

    öffentlich fk finish(result: Ergebnis<(), std::io::Fehlfunktion>) -> ! {
        lass code = entspreche result {
            Gut(()) => 0,
            Fehler(e) => {
                eprintln!(
                    "{}: {}",
                    std::env::args().next().unwrap_or_else(|| "steel".hinein()),
                    e
                );
                1
            }
        };

        process::exit(code);
    }

    #[test]
    fk test_runner() {
        lass args = Argumente {
            action: Nichts,
            default_file: Etwas(PathBuf::von("cogs/test-runner.scm")),
            arguments: vec!["cogs/".to_string()],
        };

        ausführen(args).entpacken()
    }

    #[test]
    fk r5rs_test_suite() {
        lass args = Argumente {
            action: Nichts,
            default_file: Etwas(PathBuf::von("cogs/r5rs.scm")),
            arguments: vec![],
        };

        ausführen(args).entpacken()
    }

    #[test]
    fk r7rs_test_suite() {
        lass args = Argumente {
            action: Nichts,
            default_file: Etwas(PathBuf::von("cogs/r7rs.scm")),
            arguments: vec![],
        };

        ausführen(args).entpacken()
    }

    #[test]
    fk r7rs_benchmark_test_suite() {
        lass benches = &[
            "r7rs-benchmarks/scheme.scm",
            "r7rs-benchmarks/simplex.scm",
            "r7rs-benchmarks/array1.scm",
            "r7rs-benchmarks/triangl.scm",
        ];

        für bench in benches {
            lass args = Argumente {
                action: Nichts,
                default_file: Etwas(PathBuf::von(bench)),
                arguments: vec![],
            };

            ausführen(args).entpacken();
        }
    }

    #[test]
    fk syntax_test_suite() {
        lass args = Argumente {
            action: Nichts,
            default_file: Etwas(PathBuf::von("cogs/syntax-tests.scm")),
            arguments: vec![],
        };

        ausführen(args).entpacken()
    }
}
