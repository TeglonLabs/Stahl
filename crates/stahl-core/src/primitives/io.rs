use crate::rvals::{Result, StahlVal};
use crate::stop;
use std::io;
// use std::io::Write;

// mod primitives;

pub struct IoFunctions {}
impl IoFunctions {
    // pub fn sandboxed_display() -> StahlVal {
    //     StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
    //         if args.len() == 1 {
    //             let print_val = &args[0];

    //             let output_port = DEFAULT_OUTPUT_PORT.with(|x| x.clone());

    //             match &*output_port.borrow() {
    //                 crate::values::port::StahlPort::StdOutput(out) => match &print_val {
    //                     StahlVal::StringV(s) => write!(out.borrow_mut().lock(), "{s}"),
    //                     _ => write!(out.borrow_mut().lock(), "{print_val}"),
    //                 },
    //                 crate::values::port::StahlPort::StringOutput(out) => match &print_val {
    //                     StahlVal::StringV(s) => write!(out.borrow_mut(), "{s}"),
    //                     _ => write!(out.borrow_mut(), "{print_val}"),
    //                 },
    //                 // crate::values::port::StahlPort::Closed => todo!(),
    //                 other => stop!(Generic => "Unable to write to port: {:?}", other),
    //             }?;

    //             Ok(StahlVal::Void)
    //         } else {
    //             stop!(ArityMismatch => "display takes one argument");
    //         }
    //     })
    // }

    // pub fn sandboxed_newline() -> StahlVal {
    //     StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
    //         if args.is_empty() {
    //             // println!();

    //             let output_port = DEFAULT_OUTPUT_PORT.with(|x| x.clone());

    //             match &*output_port.borrow() {
    //                 crate::values::port::StahlPort::StdOutput(out) => {
    //                     writeln!(out.borrow_mut().lock())
    //                 }
    //                 crate::values::port::StahlPort::StringOutput(out) => {
    //                     writeln!(out.borrow_mut())
    //                 }
    //                 // crate::values::port::StahlPort::Closed => todo!(),
    //                 other => stop!(Generic => "Unable to write to port: {:?}", other),
    //             }?;

    //             Ok(StahlVal::Void)
    //         } else {
    //             stop!(ArityMismatch => "newline takes no arguments");
    //         }
    //     })
    // }

    pub fn display() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            for arg in args {
                match &arg {
                    StahlVal::StringV(s) => print!("{s}"),
                    _ => print!("{arg}"),
                }
            }

            Ok(StahlVal::Void)
        })
    }

    pub fn displayln() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            for arg in args {
                match &arg {
                    StahlVal::StringV(s) => {
                        print!("{s}")
                    }
                    _ => print!("{arg}"),
                }
            }

            println!();

            Ok(StahlVal::Void)
        })
    }

    pub fn newline() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.is_empty() {
                println!();
                Ok(StahlVal::Void)
            } else {
                stop!(ArityMismatch => "newline takes no arguments");
            }
        })
    }

    pub fn read_to_string() -> StahlVal {
        StahlVal::FuncV(|_args: &[StahlVal]| -> Result<StahlVal> {
            let mut input_text = String::new();
            io::stdin().read_line(&mut input_text)?;
            Ok(StahlVal::StringV(input_text.trim_end().into()))
        })
    }
}
