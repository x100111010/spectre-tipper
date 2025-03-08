use std::fmt::Display;

use spectre_consensus_core::constants::SOMPI_PER_SPECTRE;

use crate::{error::Error, result::Result};

pub fn try_parse_required_nonzero_spectre_as_sompi_u64<S: ToString + Display>(
    spectre_amount: Option<S>,
) -> Result<u64> {
    if let Some(spectre_amount) = spectre_amount {
        let sompi_amount = spectre_amount.to_string().parse::<f64>().map_err(|_| {
            Error::custom(format!(
                "Supplied Spectre amount is not valid: '{spectre_amount}'"
            ))
        })? * SOMPI_PER_SPECTRE as f64;
        if sompi_amount < 0.0 {
            Err(Error::custom(
                "Supplied Spectre amount is not valid: '{spectre_amount}'",
            ))
        } else {
            let sompi_amount = sompi_amount as u64;
            if sompi_amount == 0 {
                Err(Error::custom(
                    "Supplied required Spectre amount must not be a zero: '{spectre_amount}'",
                ))
            } else {
                Ok(sompi_amount)
            }
        }
    } else {
        Err(Error::custom("Missing Spectre amount"))
    }
}
