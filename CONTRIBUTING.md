# Mitwirken an Stahl

## Erste Schritte

Das Folgende klont Stahl aus dem primären Repository und führt die Test-Suite aus. Sie sollten zuvor eine [aktuelle Rust-Toolchain](https://www.rust-lang.org/tools/install) eingerichtet haben.

```bash
git clone https://github.com/TeglonLabs/Stahl.git &&
cd stahl &&
cargo xtask install &&
cargo test --all
```

Dies installiert:

* Den Stahl-Interpreter, `stahl`
* Den Dylib-Installer, `cargo-stahl-lib` (auch über den Interpreter verfügbar)
* Den Stahl Language Server
* Die Standardbibliothek, zu finden im `cogs`-Verzeichnis

## Commit-Nachrichten-Stil

Stahl verwendet keinen strengen Commit-Nachrichten-Stil oder eine Konvention. Versuchen Sie, Best Practices zu folgen, indem Sie die erste Zeile prägnant und beschreibend halten, ansonsten verwenden Sie Ihr bestes Urteilsvermögen.

## Einreichen von Patches

WICHTIG: Durch das Einreichen eines Patches erklären Sie sich damit einverstanden, dass Ihre Arbeit unter der vom Projekt verwendeten Lizenz lizenziert wird.

Befolgen Sie allgemeine Ratschläge beim Einreichen eines Patches:

- Halten Sie den Patch fokussiert.
- Fügen Sie Tests hinzu.
- Arbeiten Sie in einem separaten Branch, nicht `master`.
- Bevor Sie an "großen Ideen" arbeiten, öffnen Sie ein Issue, um Ansätze oder Anforderungen zu diskutieren, um verschwendeten Aufwand zu begrenzen.
