use crate::rvals::{Result, StahlVal};
use crate::stop;

pub struct ControlOperations {}
impl ControlOperations {
    pub fn to_string() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            let mut error_message = String::new();

            for arg in args {
                let error_val = arg.to_string();
                error_message.push(' ');
                error_message.push_str(error_val.trim_matches('\"'));
            }

            Ok(StahlVal::StringV(error_message.into()))
        })
    }

    pub fn error() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            let mut error_message = String::new();

            if !args.is_empty() {
                for arg in args {
                    let error_val = arg.to_string();
                    error_message.push(' ');
                    error_message.push_str(error_val.trim_matches('\"'));
                }

                stop!(Generic => error_message);
            } else {
                stop!(ArityMismatch => "error takes at least one argument");
            }
        })
    }
}
