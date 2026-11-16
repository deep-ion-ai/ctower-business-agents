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

/// Dias desde 1970-01-01 (negativo antes).
pub fn dias_desde_1970(data: Data) -> i64 {
    // Algoritmo "days_from_civil" de Howard Hinnant.
    let (ano, mes, dia) = (
        i64::from(data.ano),
        i64::from(data.mes),
        i64::from(data.dia),
    );
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (mes + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dia - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A data `dias` depois de `data` (antes, se negativo). Datas anteriores a 1970
/// ficam em 1970-01-01.
pub fn somar_dias(data: Data, dias: i64) -> Data {
    let total = dias_desde_1970(data).saturating_add(dias).max(0) as u64;
    data_utc(total * 86_400_000)
}

/// Dias de `de` até `ate` (negativo se `ate` vem antes).
pub fn dias_entre(de: Data, ate: Data) -> i64 {
    dias_desde_1970(ate) - dias_desde_1970(de)
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

    #[test]
    fn aritmetica_de_dias() {
        let d = |t: &str| Data::analisar(t).unwrap();
        assert_eq!(dias_desde_1970(d("1970-01-01")), 0);
        assert_eq!(
            dias_desde_1970(d("2026-11-11")),
            1_794_355_200_000 / 86_400_000
        );
        assert_eq!(somar_dias(d("2026-11-14"), 30).to_string(), "2026-12-14");
        assert_eq!(somar_dias(d("2026-12-20"), 30).to_string(), "2027-01-19");
        assert_eq!(somar_dias(d("2028-02-28"), 1).to_string(), "2028-02-29");
        assert_eq!(somar_dias(d("2026-03-01"), -1).to_string(), "2026-02-28");
        assert_eq!(dias_entre(d("2026-11-14"), d("2026-12-14")), 30);
        assert_eq!(dias_entre(d("2026-12-14"), d("2026-11-14")), -30);
        for n in [0u64, 59, 365, 10_957, 20_000, 40_000] {
            assert_eq!(dias_desde_1970(data_utc(n * 86_400_000)), n as i64);
        }
    }
}
