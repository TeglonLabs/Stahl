# Stahl

<div align="center">
    <img width="150px" src="images/styled.png">
</div>

<div align="center">

Ein einbettbarer und erweiterbarer Scheme-Dialekt, geschrieben in Rust.

![Aktionsstatus](https://github.com/TeglonLabs/Stahl/workflows/Build/badge.svg)
![Aktionsstatus](https://github.com/TeglonLabs/Stahl/workflows/Docker%20CI/badge.svg)
[![Abdeckungsstatus](https://coveralls.io/repos/github/TeglonLabs/Stahl/badge.svg?branch=master)](https://coveralls.io/github/TeglonLabs/Stahl?branch=master)
[![Discord-Chat](https://img.shields.io/discord/1152443024715034675.svg?logo=discord&label=discord)](https://discord.gg/WwFRXdN6HU)
[![Matrix-Chat](https://img.shields.io/matrix/stahl:matrix.org?logo=element&label=matrix)](https://matrix.to/#/#stahl:matrix.org)

<a href="https://mattwparas.github.io/stahl-playground/dev">
    <b>Probieren Sie es im Playground</b>
</a>
·
<a href="https://mattwparas.github.io/stahl/book">
    <b>Das Stahl-Buch lesen (WIP)</b>
</a>

</div>

## Erste Schritte

Dieses Github-Repository enthält einen CLI-Interpreter. Um ihn online auszuprobieren, besuchen Sie den [Stahl Playground](https://mattwparas.github.io/stahl-playground/dev). Um einen REPL mit den Crates lokal zu verwenden, stellen Sie sicher, dass Rust installiert ist.

Klonen Sie dann das Repo und führen Sie folgenden Befehl aus:

```bash
cargo run
```

Dies startet eine REPL-Instanz, die etwa so aussieht:

<p align="center">
  <img src="images/repl.gif" width="100%">
</p>

### Vollständige Installation

Wenn Sie alles installieren möchten, führen Sie einfach folgenden Befehl aus:

```bash
cargo xtask install
```

Dies installiert:

- Den Stahl-Interpreter, `stahl`
- Den Dylib-Installer, `cargo-stahl-lib` (auch über den Interpreter verfügbar)
- Den Stahl Language Server
- Die Standardbibliothek, zu finden im `cogs`-Verzeichnis

### Pakete

Wenn Sie den Speicherort der installierten Pakete anpassen möchten, setzen Sie bitte die Umgebungsvariable `STAHL_HOME`. Stahl geht derzeit vom Standard `$HOME/.stahl` aus, wenn die Umgebungsvariable nicht gesetzt ist.

## Über

`Stahl` ist ein einbettbarer Scheme-Interpreter, der auch eine eigenständige CLI enthält. Inspiriert größtenteils von Racket, zielt die Sprache darauf ab, eine ergonomische Scheme-Variante zu sein, die hilfreich für die Einbettung in Anwendungen ist oder eigenständig mit leistungsstarken, in Rust implementierten Funktionen verwendet werden kann. Die Sprachimplementierung selbst enthält ein ziemlich mächtiges Makrosystem basierend auf dem `syntax-rules`-Stil und eine Bytecode-Virtual-Machine. Derzeit ist sie weitgehend konform mit R5RS, es fehlt nur die Unterstützung für `let-syntax`. Unterstützung für R7RS ist in Arbeit.

> **Warnung**
> Die API ist relativ instabil ohne Garantien und kann sich jederzeit vor 1.0 ändern. Es gibt zweifellos Fehler, und alle größeren Fehlerberichte werden schnell bearbeitet. Davon abgesehen benutze ich es selbst als täglichen Treiber für viele Skriptaufgaben.

## Funktionen

- `syntax-rules`-Stil Makros werden unterstützt
- Einfache Integration mit Rust-Funktionen und Strukturen
- Einfaches Aufrufen eines Skripts aus Rust oder über eine separate Datei
- Effizient - gängige Funktionen und Datenstrukturen sind auf Leistung optimiert (`map`, `filter`, usw.)
- Verträge höherer Ordnung
- Eingebaute unveränderliche Datenstrukturen umfassen:
  - Listen
  - Vektoren
  - Hashmaps
  - Hashsets

## Verträge

Inspiriert von Rackets Verträgen höherer Ordnung, implementiert* `Stahl` Verträge höherer Ordnung, um Design by Contract zu ermöglichen, vereinfacht durch ein `define/contract`-Makro für bessere Ergonomie. Racket nutzt ein Konzept namens _blame_, das versucht, die verletzende Partei zu identifizieren - `Stahl` hat noch kein voll ausgearbeitetes Blame-System, aber daran wird gearbeitet. Hier sind einige Beispiele:

```scheme
;; Einfache flache Verträge
(define/contract (test x y)
    (->/c even? even? odd?)
    (+ x y 1))

(test 2 2) ;; => 5

(define/contract (test-violation x y)
    (->/c even? even? odd?)
    (+ x y 1))

(test-violation 1 2) ;; Vertragsverletzung

```

Verträge sind als _Werte_ implementiert, so dass sie an Funktionen gebunden sind. Dies ermöglicht die Überprüfung von Verträgen bei Funktionen selbst, da Funktionen herumgereicht werden können:

```scheme
;; Verträge höherer Ordnung, Überprüfung bei Anwendung
(define/contract (higher-order func y)
    (->/c (->/c even? odd?) even? even?)
    (+ 1 (func y)))

(higher-order (lambda (x) (+ x 1)) 2) ;; => 4

(define/contract (higher-order-violation func y)
    (->/c (->/c even? odd?) even? even?)
    (+ 1 (func y)))

(higher-order-violation (lambda (x) (+ x 2)) 2) ;; Vertragsverletzung
```

Verträge bei Funktionen werden erst überprüft, wenn sie angewendet werden, sodass eine Funktion, die eine _vertraglich gebundene_ Funktion zurückgibt, keine Verletzung verursacht, bis diese Funktion tatsächlich verwendet wird:

```scheme
;; Weitere Verträge höherer Ordnung, werden bei Anwendung geprüft
(define/contract (output)
    (->/c (->/c string? int?))
    (lambda (x) 10))

(define/contract (accept func)
    (->/c (->/c string? int?) string?)
    "cool cool cool")

(accept (output)) ;; => "cool cool cool"

;; unterschiedliche Verträge beim Argument
(define/contract (accept-violation func)
    (->/c (->/c string? string?) string?)
    (func "applesauce")
    "cool cool cool")

(accept-violation (output)) ;; Vertragsverletzung

;; erzeugt eine Funktion
(define/contract (generate-closure)
    (->/c (->/c string? int?))
    (lambda (x) 10))

;; ruft generate-closure auf, was zu einer Vertragsverletzung führen sollte
(define/contract (accept-violation)
    (->/c (->/c string? string?))
    (generate-closure))

((accept-violation) "test") ;; Vertragsverletzung
```

Vielleicht ein nuancierterer Fall:

```scheme
(define/contract (output)
    (->/c (->/c string? int?))
    (lambda (x) 10.2))

(define/contract (accept)
    (->/c (->/c string? number?))
    (output))


((accept) "test") ;; Vertragsverletzung 10.2 erfüllt number?, aber _nicht_ int?
```

\* Sehr stark in Arbeit

## Transducers

Inspiriert von Clojures Transducern hat `Stahl` ein ähnliches Objekt, das irgendwo auf halbem Weg zwischen Transducern und Iteratoren liegt. Betrachten Sie folgendes:

```scheme

(mapping (lambda (x) (+ x 1))) ;; => <#iterator>
(filtering even?) ;; => <#iterator>
(taking 15) ;; => <#iterator>

(compose
    (mapping add1)
    (filtering odd?)
    (taking 15)) ;; => <#iterator>
```

Jeder dieser Ausdrücke gibt ein `<#iterator>`-Objekt aus, was bedeutet, dass sie mit `transduce` kompatibel sind. `transduce` nimmt einen Transducer (d.h. `<#iterator>`) und eine Sammlung, die iteriert werden kann (`list`, `vector`, `stream`, `hashset`, `hashmap`, `string`, `struct`) und wendet den Transducer an.

```scheme
;; Akzeptiert Listen
(transduce (list 1 2 3 4 5) (mapping (lambda (x) (+ x 1))) (into-list)) ;; => '(2 3 4 5 6)

;; Akzeptiert Vektoren
(transduce (vector 1 2 3 4 5) (mapping (lambda (x) (+ x 1))) (into-vector)) ;; '#(2 3 4 5 6)

;; Akzeptiert sogar Streams!
(define (integers n)
    (stream-cons n (lambda () (integers (+ 1 n)))))

(transduce (integers 0) (taking 5) (into-list)) ;; => '(0 1 2 3 4)
```

Transduce akzeptiert auch eine Reducer-Funktion. Oben haben wir `into-list` und `into-vector` verwendet, aber unten können wir jeden beliebigen Reducer verwenden:

```scheme
;; (-> transducer reducing-function initial-value iterable)
(transduce (list 0 1 2 3) (mapping (lambda (x) (+ x 1))) (into-reducer + 0)) ;; => 10
```

Compose kombiniert einfach die Iterator-Funktionen und lässt uns zwischenzeitliche Allokationen vermeiden. Die Komposition funktioniert von links nach rechts - sie verkettet jeden Wert durch die Funktionen und akkumuliert dann in den Ausgabetyp. Siehe folgendes:

```scheme
(define xf
    (compose
        (mapping add1)
        (filtering odd?)
        (taking 5)))

(transduce (range 0 100) xf (into-list)) ;; => '(1 3 5 7 9)
```

## Module

Um eine wachsende Codebasis zu unterstützen, hat Stahl Modulunterstützung für Projekte, die sich über mehrere Dateien erstrecken. Stahl-Dateien können Werte `provide`n (bereitstellen, mit angehängten Verträgen) und Module aus anderen Dateien `require`n (anfordern):

```scheme
;; main.scm
(require "provide.scm")

(even->odd 10)


;; provide.scm
(provide
    (contract/out even->odd (->/c even? odd?))
    no-contract
    flat-value)

(define (even->odd x)
    (+ x 1))

(define (accept-number x) (+ x 10))

(define (no-contract) "cool cool cool")
(define flat-value 15)

(displayln "Calling even->odd with some bad inputs but its okay")
(displayln (even->odd 1))
```

Hier können wir sehen, dass wenn wir `main` ausführen würden, es den Inhalt von `provide` einschließen würde, und nur bereitgestellte Werte wären von `main` aus zugänglich. Der Vertrag wird an der Vertragsgrenze angehängt, also kann man innerhalb des `provide`-Moduls den Vertrag verletzen, aber außerhalb des Moduls wird der Vertrag angewendet.

Ein paar Anmerkungen zu Modulen:

- Zyklische Abhängigkeiten sind nicht erlaubt
- Module werden nur einmal kompiliert und über mehrere Dateien hinweg verwendet. Wenn `A` `B` und `C` benötigt, und `B` `C` benötigt, wird `C` einmal kompiliert und zwischen `A` und `B` geteilt.
- Module werden bei Änderungen neu kompiliert, und alle abhängigen Dateien werden ebenfalls nach Bedarf neu kompiliert.

## Leistung

Vorläufige Benchmarks zeigen folgendes auf meinem Rechner:

| Benchmark | Stahl    | Python   |
| --------- | -------- | -------- |
| (fib 28)  | 63.383ms | 65.10 ms |
| (ack 3 3) | 0.303 ms | 0.195 ms |

## Beispiele für das Einbetten von Rust-Werten in die virtuelle Maschine

Rust-Werte, -Typen und -Funktionen können einfach in Stahl eingebettet werden. Mit dem `register_fn`-Aufruf können Sie Funktionen einfach einbetten:

```rust
rost::rost! {
    benutze stahl_vm::engine::Engine;
    benutze stahl_vm::register_fn::RegisterFn;

    fk external_function(arg1: usize, arg2: usize) -> usize {
        arg1 + arg2
    }

    fk option_function(arg1: Möglichkeit<Zeichenkette>) -> Möglichkeit<Zeichenkette> {
        arg1
    }

    fk result_function(arg1: Möglichkeit<Zeichenkette>) -> Ergebnis<Zeichenkette, Zeichenkette> {
        wenn lass Etwas(inner) = arg1 {
            Gut(inner)
        } anderenfalls {
            Fehler("Got a none".to_string())
        }
    }

    öffentlich fk main() {
        lass änd vm = Engine::new();

        // Hier können wir Funktionen registrieren
        // Jede Funktion kann Parameter akzeptieren, die `FromStahlVal` implementieren und
        // Werte zurückgeben, die `IntoStahlVal` implementieren
        vm.register_fn("external-function", external_function);

        // Siehe die Dokumentation für weitere Informationen über `FromStahlVal` und `IntoStahlVal`
        // aber wir können sehen, dass sogar Funktionen, die Option<T> oder Result<T,E> akzeptieren/zurückgeben
        // registriert werden können
        vm.register_fn("option-function", option_function);

        // Ergebniswerte werden direkt auf Fehler in der VM abgebildet und sprudeln zurück nach oben
        vm.register_fn("result-function", result_function);

        vm.run(
            r#"
            (define foo (external-function 10 25))
            (define bar (option-function "applesauce"))
            (define baz (result-function "bananas"))
        "#,
        )
        .entpacken();

        lass foo = vm.extract::<usize>("foo").entpacken();
        ausgabe!("foo: {}", foo);
        behaupte_gleich!(35, foo);

        // Kann auch einen Wert extrahieren, indem der Typ an der Variable angegeben wird
        lass bar: Zeichenkette = vm.extract("bar").entpacken();
        ausgabe!("bar: {}", bar);
        behaupte_gleich!("applesauce".to_string(), bar);

        lass baz: Zeichenkette = vm.extract("baz").entpacken();
        ausgabe!("baz: {}", baz);
        behaupte_gleich!("bananas".to_string(), baz);
    }
}
```

Wir können auch Strukturen selbst einbetten:

```rust
rost::rost! {
    benutze stahl_vm::engine::Engine;
    benutze stahl_vm::register_fn::RegisterFn;

    benutze stahl_derive::Stahl;

    // Um einen Typ mit Stahl zu registrieren,
    // muss er Clone, Debug und Stahl implementieren
    #[derive(Clone, Debug, Stahl, PartialEq)]
    öffentlich struktur ExternalStruct {
        foo: usize,
        bar: Zeichenkette,
        baz: f64,
    }

    umstz ExternalStruct {
        öffentlich fk new(foo: usize, bar: Zeichenkette, baz: f64) -> Selbst {
            ExternalStruct { foo, bar, baz }
        }

        // Einbetten von Funktionen, die self per Value nehmen
        öffentlich fk method_by_value(selbst) -> usize {
            selbst.foo
        }

        öffentlich fk method_by_reference(&selbst) -> usize {
            selbst.foo
        }

        // Setter sollten den Wert aktualisieren und eine neue Instanz zurückgeben (funktionales Setzen)
        öffentlich fk set_foo(änd selbst, foo: usize) -> Selbst {
            selbst.foo = foo;
            selbst
        }
    }

    öffentlich fk main() {
        lass änd vm = Engine::new();

        // Das Registrieren eines Typs gibt Zugriff auf ein Prädikat für den Typ
        vm.register_type::<ExternalStruct>("ExternalStruct?");

        // Strukturen in Stahl haben typischerweise einen Konstruktor, der der Name der Struktur ist
        vm.register_fn("ExternalStruct", ExternalStruct::new);

        // register_fn kann verkettet werden
        vm.register_fn("method-by-value", ExternalStruct::method_by_value)
            .register_fn("method-by-reference", ExternalStruct::method_by_reference)
            .register_fn("set-foo", ExternalStruct::set_foo);

        lass external_struct = ExternalStruct::new(1, "foo".to_string(), 12.4);

        // Das Registrieren eines externen Werts ist fehlbar, wenn die Konvertierung aus irgendeinem Grund fehlschlägt
        // Zum Beispiel ist das Registrieren eines Err(T) fehlbar. Die meisten Implementierungen außerhalb von manuellen
        // sollten jedoch nicht fehlschlagen
        vm.register_external_value("external-struct", external_struct)
            .entpacken();

        lass output = vm
            .run(
                r#"
                (define new-external-struct (set-foo external-struct 100))
                (define get-output (method-by-value external-struct))
                (define second-new-external-struct (ExternalStruct 50 "bananas" 72.6))
                "last-result"
            "#,
            )
            .entpacken();

        lass new_external_struct = vm.extract::<ExternalStruct>("new-external-struct").entpacken();
        ausgabe!("new_external_struct: {:?}", new_external_struct);
        behaupte_gleich!(
            ExternalStruct::new(100, "foo".to_string(), 12.4),
            new_external_struct
        );

        // Kann auch einen Wert extrahieren, indem der Typ an der Variable angegeben wird
        lass get_output: usize = vm.extract("get-output").entpacken();
        ausgabe!("get_output: {}", get_output);
        behaupte_gleich!(1, get_output);

        lass second_new_external_struct: ExternalStruct =
            vm.extract("second-new-external-struct").entpacken();
        ausgabe!(
            "second_new_external_struct: {:?}",
            second_new_external_struct
        );
        behaupte_gleich!(
            ExternalStruct::new(50, "bananas".to_string(), 72.6),
            second_new_external_struct
        );

        // Wir erhalten auch die Ausgabe der VM als Wert jedes ausgeführten Ausdrucks
        // wir können die Ergebnisse einfach durch Drucken inspizieren, wie so
        ausgabe!("{:?}", output);
    }
}
```

Siehe den examples-Ordner für weitere Beispiele zum Einbetten von Werten und zur Interaktion mit der Außenwelt.

## Lizenz

Lizenziert unter entweder

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) oder http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

nach Ihrer Wahl.

## Mitwirken

Sofern Sie nichts anderes ausdrücklich angeben, gilt jeder Beitrag, der von Ihnen absichtlich zur Aufnahme in das Werk eingereicht wurde, wie in der Apache-2.0-Lizenz definiert, als dual lizenziert wie oben, ohne zusätzliche Bedingungen oder Konditionen.

Siehe [CONTRIBUTING.md](./CONTRIBUTING.md).
