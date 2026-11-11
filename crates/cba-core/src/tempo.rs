//! Instantes e datas sem dependência de calendário.
//!
//! O modelo guarda instantes em milissegundos desde 1970-01-01 UTC, e as datas dos
//! documentos (`criado`), em UTC.

use std::time::{SystemTime, UNIX_EPOCH};

use cba_kb::frontmatter::Data;

/// Milissegundos desde 1970-01-01 UTC.
pub fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Data UTC de um instante em milissegundos.
pub fn data_utc(ms: u64) -> Data {
    // Algoritmo "civil_from_days" de Howard Hinnant.
    let dias = (ms / 86_400_000) as i64;
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let ano = (yoe + era * 400 + i64::from(mes <= 2)) as u16;
    Data::nova(ano, mes, dia).expect("o algoritmo só produz datas válidas")
}

/// Data UTC de hoje.
pub fn hoje() -> Data {
    data_utc(agora_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas_conhecidas() {
        assert_eq!(data_utc(0).to_string(), "1970-01-01");
        // 2000-02-29 12:00 UTC (ano bissexto).
        assert_eq!(data_utc(951_825_600_000).to_string(), "2000-02-29");
        // 2026-11-11 00:00 UTC.
        assert_eq!(data_utc(1_794_355_200_000).to_string(), "2026-11-11");
        // Um milissegundo antes da virada do ano.
        assert_eq!(data_utc(1_798_761_599_999).to_string(), "2026-12-31");
    }
}
