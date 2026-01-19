{
  lib,
  stdenv,
  mkShell,
  stahl,
  cargo,
  rustc,
  libiconv,
  CoreServices,
  SystemConfiguration,
  rust-analyzer,
  rustfmt,
}:
mkShell {
  shellHook = ''
    export STAHL_HOME="${stahl}/lib/"
  '';
  packages =
    [
      cargo
      rustc
      rust-analyzer
      rustfmt
      libiconv
    ]
    ++ lib.optionals stdenv.isDarwin [
      CoreServices
      SystemConfiguration
    ];
    inputsFrom = stahl;
}
