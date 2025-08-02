use core::ops::Deref;

use acpi::{AcpiHandler, AcpiTables, tcpa::Tcpa, tpm2::Tpm2};

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    if let Ok(tpm2) = acpi_tables.find_table::<Tpm2>() {
        let tpm2 = tpm2.deref();
        log::info!("TPM 2.0: {tpm2:#X?}");
    } else if let Ok(tpm) = acpi_tables.find_table::<Tcpa>() {
        let tpm = tpm.deref();
        log::info!("TPM 1.2: {tpm:#X?}");
    } else {
        log::warn!("No TPM 1.2 or TPM 2.0 found");
    }
}
