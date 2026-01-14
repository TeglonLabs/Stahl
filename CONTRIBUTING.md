# Mitwirken an Steel

## Erste Schritte

Das Folgende klont Steel aus dem primären Repository und führt die Test-Suite aus. Sie sollten zuvor eine [aktuelle Rust-Toolchain](https://www.rust-lang.org/tools/install) eingerichtet haben.

```bash
git clone https://github.com/mattwparas/steel.git &&
cd steel &&
cargo xtask install &&
cargo test --all
```

Dies installiert:

* Den Steel-Interpreter, `steel`
* Den Dylib-Installer, `cargo-steel-lib` (auch über den Interpreter verfügbar)
* Den Steel Language Server
* Die Standardbibliothek, zu finden im `cogs`-Verzeichnis

## Commit-Nachrichten-Stil

Steel verwendet keinen strengen Commit-Nachrichten-Stil oder eine Konvention. Versuchen Sie, Best Practices zu folgen, indem Sie die erste Zeile prägnant und beschreibend halten, ansonsten verwenden Sie Ihr bestes Urteilsvermögen.

## Einreichen von Patches

WICHTIG: Durch das Einreichen eines Patches erklären Sie sich damit einverstanden, dass Ihre Arbeit unter der vom Projekt verwendeten Lizenz lizenziert wird.

Befolgen Sie allgemeine Ratschläge beim Einreichen eines Patches:

- Halten Sie den Patch fokussiert.
- Fügen Sie Tests hinzu.
- Arbeiten Sie in einem separaten Branch, nicht `master`.
- Bevor Sie an "großen Ideen" arbeiten, öffnen Sie ein Issue, um Ansätze oder Anforderungen zu diskutieren, um verschwendeten Aufwand zu begrenzen.
