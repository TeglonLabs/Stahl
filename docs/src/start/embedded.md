# Using Stahl as an embedded scripting engine

Stahl can be used as a scripting language within a Rust program. You can achieve
this by creating a Stahl `Engine` object. The `Engine` object allows you to
execute Scheme code and interact with it from your Rust program. For more
details about `Engine`, see the [Engine API](../engine/engine.html).

## Stahl Engine

The Stahl Virtual Machine is provided by the `stahl-core` trait.

```toml
[dependencies]
stahl-core = { git="https://github.com/TeglonLabs/Stahl.git", branch = "master" }
```

The following example runs a few expressions in a Stahl `Engine` and asserts
that the results are as expected.

```rust,noplaypen
use stahl::stahl_vm::engine::Engine;
use stahl::StahlVal;

fn main() {
    let mut stahl_engine = Engine::new();
    let answer = stahl_engine.run(
        (r#"
      (+ 1 2 3 4)
      (+ 5 6 7 8)
    "#),
    );
    assert_eq!(answer, vec![StahlVal::IntV(10), StahlVal::IntV(26)])
}
```

### Engine::new

Creates a new engine. The `Engine` is used to run Stahl Scheme code. Note that
the `Engine` is not `Send` or `Sync`. This means it is bound to the current
thread and cannot be shared or sent across other threads.

```rust,noplaypen
let mut stahl_engine = Engine::new();
```

### Engine::run

Runs a Stahl expression and returns the result as a `Vec<StahlVal>`. If any
error occurs, then `Err(StahlErr)` is returned.

```rust,noplaypen
let mut stahl_engine = Engine::new();
assert_eq!(stahl_engine.run("(+ 1 1)"), Ok(vec![StahlVal::IntV(2)]));
assert!(stahl_engine.run("(+ 1 undefined-identifier)").is_err());
```

## Embedding The Stahl REPL

Repl functionality is provided by the `stahl-repl` crate.

```toml
[dependencies]
stahl-repl = { git="https://github.com/TeglonLabs/Stahl.git", branch = "master" }
```

### run_repl

`run_repl` runs the Stahl repl until an IO error is encountered or the user
exits the repl. The repl may be exited by:

- Running the `(quit)` Stahl Scheme function.
- Pressing either `ctrl+c` or `ctrl+d` within the repl.

```rust,noplaypen
let stahl_engine = Engine::new();
stahl_repl::run_repl(stahl_engine).unwrap();
```
