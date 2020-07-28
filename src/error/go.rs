use std::convert::{TryFrom, TryInto};
use std::fmt;

use crate::Buffer;
use cosmwasm_vm::FfiError;

/// This enum gives names to the status codes returned from Go callbacks to Rust.
///
/// The go code will return one of these variants when returning.
///
/// cbindgen:prefix-with-name
// NOTE TO DEVS: If you change the values assigned to the variants of this enum, You must also
//               update the match statement in the From conversion below.
//               Otherwise all hell may break loose.
//               You have been warned.
//
#[repr(i32)] // This makes it so the enum looks like a simple i32 to Go
#[derive(PartialEq)]
pub enum GoResult {
    Ok = 0,
    /// Go panicked for an unexpected reason.
    Panic = 1,
    /// Go received a bad argument from Rust
    BadArgument = 2,
    /// Ran out of gas while using the SDK (e.g. storage)
    OutOfGas = 3,
    /// An error happened during normal operation of a Go callback, which should abort the contract
    Other = 4,
    /// An error happened during normal operation of a Go callback, which should be fed back to the contract
    User = 5,
}

impl TryFrom<GoResult> for Result<(), FfiError> {
    type Error = &'static str;

    fn try_from(other: GoResult) -> Result<Self, Self::Error> {
        match other {
            GoResult::Ok => Ok(Ok(())),
            GoResult::Panic => Ok(Err(FfiError::foreign_panic())),
            GoResult::BadArgument => Ok(Err(FfiError::bad_argument())),
            GoResult::OutOfGas => Ok(Err(FfiError::out_of_gas())),
            GoResult::Other => Err("Unspecified error in Go code"), // no conversion possible due to missing error message
            GoResult::User => Err("Unspecified error in Go code"), // no conversion possible due to missing error message
        }
    }
}

impl From<i32> for GoResult {
    fn from(n: i32) -> Self {
        use GoResult::*;
        // This conversion treats any number that is not otherwise an expected value as `GoError::Other`
        match n {
            0 => Ok,
            1 => Panic,
            2 => BadArgument,
            3 => OutOfGas,
            5 => User,
            _ => Other,
        }
    }
}

impl fmt::Display for GoResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoResult::Ok => write!(f, "Ok"),
            GoResult::Panic => write!(f, "Panic"),
            GoResult::BadArgument => write!(f, "BadArgument"),
            GoResult::OutOfGas => write!(f, "OutOfGas"),
            GoResult::Other => write!(f, "Other Error"),
            GoResult::User => write!(f, "User Error"),
        }
    }
}

impl GoResult {
    /// This is a wrapper around `impl TryFrom<GoResult> for Result<(), FfiError>` that uses a fallback
    /// if output is not-empty, use that as the error message
    /// otherwise, call default() to generate a default message.
    /// If it is GoResult::User the error message will be returned to the contract.
    /// Otherwise, the returned error will trigger a trap in the VM and abort contract execution immediately.
    ///
    /// Safety: this reads data from an externally provided buffer and assumes valid utf-8 encoding
    /// Only call if you trust the code that provides output to be correct
    pub unsafe fn into_ffi_result<F>(self, output: Buffer, default: F) -> Result<(), FfiError>
    where
        F: Fn() -> String,
    {
        let is_user_error = self == GoResult::User;
        self.try_into().unwrap_or_else(|_| {
            let msg = if output.ptr.is_null() {
                default()
            } else {
                // We initialize `output` with a null pointer. if it is not null,
                // that means it was initialized by the go code, with values generated by `memory::allocate_rust`
                String::from_utf8_lossy(&output.consume()).into()
            };
            if is_user_error {
                Err(FfiError::user_err(msg))
            } else {
                Err(FfiError::unknown(msg))
            }
        })
    }
}
