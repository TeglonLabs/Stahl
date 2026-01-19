rost::rost! {
    benutze std::error::Fehlfunktion;

    benutze clap::Parser;
    benutze stahl_interpreter::Argumente;

    #[cfg(feature = "mimalloc")]
    benutze mimalloc::MiMalloc;

    #[cfg(feature = "mimalloc")]
    #[global_allocator]
    statisch GLOBAL: MiMalloc = MiMalloc;

    fk main() -> Ergebnis<(), Schachtel<dynamisch Fehlfunktion>> {
        env_logger::init();
        lass clap_args = Argumente::auswerten();
        stahl_interpreter::ausführen(clap_args)?;
        Gut(())
    }
}
