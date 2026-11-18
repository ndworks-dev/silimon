//! Regras puras de métricas e formatação, sem dependência de AppKit.

use std::collections::VecDeque;

use macmon::Metrics;

pub const HISTORY_LEN: usize = 120;
pub const INTERVALS_MS: [u32; 3] = [1000, 2000, 5000];
pub const DEFAULT_INTERVAL_MS: u32 = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMetric {
    CpuTemp,
    GpuTemp,
    CpuUsage,
    GpuUsage,
    MemPressure,
    MemUsed,
    Swap,
    ChipPower,
    SystemPower,
}

impl BarMetric {
    pub const ALL: [BarMetric; 9] = [
        BarMetric::CpuTemp,
        BarMetric::GpuTemp,
        BarMetric::CpuUsage,
        BarMetric::GpuUsage,
        BarMetric::MemPressure,
        BarMetric::MemUsed,
        BarMetric::Swap,
        BarMetric::ChipPower,
        BarMetric::SystemPower,
    ];

    pub fn key(self) -> &'static str {
        match self {
            BarMetric::CpuTemp => "cpu_temp",
            BarMetric::GpuTemp => "gpu_temp",
            BarMetric::CpuUsage => "cpu_usage",
            BarMetric::GpuUsage => "gpu_usage",
            BarMetric::MemPressure => "mem_pressure",
            BarMetric::MemUsed => "mem_used",
            BarMetric::Swap => "swap",
            BarMetric::ChipPower => "chip_power",
            BarMetric::SystemPower => "system_power",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            BarMetric::CpuTemp => "Temperatura da CPU",
            BarMetric::GpuTemp => "Temperatura da GPU",
            BarMetric::CpuUsage => "Uso da CPU",
            BarMetric::GpuUsage => "Uso da GPU",
            BarMetric::MemPressure => "Pressão de memória",
            BarMetric::MemUsed => "Memória em uso",
            BarMetric::Swap => "Swap em uso",
            BarMetric::ChipPower => "Consumo do chip",
            BarMetric::SystemPower => "Consumo do sistema",
        }
    }

    /// Maior valor comum do mostrador, usado para reservar o espaço na barra.
    fn template(self) -> &'static str {
        match self {
            BarMetric::CpuTemp | BarMetric::GpuTemp => "100°",
            BarMetric::CpuUsage | BarMetric::GpuUsage | BarMetric::MemPressure => "100%",
            BarMetric::MemUsed | BarMetric::Swap => "00,0 GB",
            BarMetric::ChipPower | BarMetric::SystemPower => "00,0 W",
        }
    }

    fn bar_parts(self, m: &Metrics) -> (&'static str, String) {
        let temp = |v: Option<f32>| v.map_or("—".into(), |t| format!("{t:.0}°"));
        match self {
            BarMetric::CpuTemp => ("CPU", temp(m.temp.cpu_temp_avg)),
            BarMetric::GpuTemp => ("GPU", temp(m.temp.gpu_temp_avg)),
            BarMetric::CpuUsage => ("CPU", percent(m.cpu_active_ratio)),
            BarMetric::GpuUsage => ("GPU", percent(m.gpu_active_ratio)),
            BarMetric::MemPressure => {
                ("MEM", m.memory.pressure_percent.map_or("—".into(), |p| format!("{p}%")))
            }
            BarMetric::MemUsed => ("RAM", gigabytes(m.memory.ram_usage)),
            BarMetric::Swap => ("SWAP", gigabytes(m.memory.swap_usage)),
            BarMetric::ChipPower => ("SoC", watts(m.chip_power)),
            BarMetric::SystemPower => ("SIS", watts(m.sys_power)),
        }
    }
}

/// Lê a seleção salva, ignorando chaves desconhecidas e mantendo a ordem do menu.
pub fn parse_selection(saved: Option<&str>) -> Vec<BarMetric> {
    let Some(saved) = saved else {
        return vec![BarMetric::CpuTemp];
    };
    let keys: Vec<&str> = saved.split(',').map(str::trim).collect();
    BarMetric::ALL.into_iter().filter(|m| keys.contains(&m.key())).collect()
}

pub fn serialize_selection(selection: &[BarMetric]) -> String {
    selection.iter().map(|m| m.key()).collect::<Vec<_>>().join(",")
}

const FIGURE_SPACE: char = '\u{2007}';
const COMPACT_TEMPLATE: &str = "100°C";

fn digit_count(value: &str) -> usize {
    value.chars().filter(char::is_ascii_digit).count()
}

/// Completa à esquerda com espaços da largura de um dígito, alinhando os números à direita.
fn pad_digits(value: String, template: &str) -> String {
    let digits = digit_count(&value);
    if digits == 0 {
        return value;
    }
    let missing = digit_count(template).saturating_sub(digits);
    std::iter::repeat_n(FIGURE_SPACE, missing).chain(value.chars()).collect()
}

/// Pares de rótulo e valor exibidos na barra. Rótulo vazio indica valor sozinho.
pub fn bar_segments(selection: &[BarMetric], metrics: Option<&Metrics>) -> Vec<(&'static str, String)> {
    let Some(m) = metrics else {
        return if selection.is_empty() { Vec::new() } else { vec![("", "—".into())] };
    };
    if selection == [BarMetric::CpuTemp] {
        let value = m.temp.cpu_temp_avg.map_or("—".into(), |t| format!("{t:.0}°C"));
        return vec![("", pad_digits(value, COMPACT_TEMPLATE))];
    }
    selection
        .iter()
        .map(|s| {
            let (label, value) = s.bar_parts(m);
            (label, pad_digits(value, s.template()))
        })
        .collect()
}

/// Segmentos com o maior valor comum de cada métrica, para medir a largura fixa do item.
pub fn bar_template(selection: &[BarMetric]) -> Vec<(&'static str, String)> {
    if selection == [BarMetric::CpuTemp] {
        return vec![("", COMPACT_TEMPLATE.to_string())];
    }
    selection
        .iter()
        .map(|s| (s.bar_parts(&Metrics::default()).0, s.template().to_string()))
        .collect()
}

pub fn bar_title(selection: &[BarMetric], metrics: Option<&Metrics>) -> String {
    bar_segments(selection, metrics)
        .iter()
        .map(|(label, value)| {
            if label.is_empty() { value.clone() } else { format!("{label} {value}") }
        })
        .collect::<Vec<_>>()
        .join("  ")
        .replace(FIGURE_SPACE, "")
}

pub fn percent(ratio: f32) -> String {
    format!("{:.0}%", (ratio * 100.0).clamp(0.0, 100.0))
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

pub fn gigabytes(bytes: u64) -> String {
    format!("{} GB", decimal(bytes as f64 / 1_073_741_824.0, 1))
}

pub fn watts(value: f32) -> String {
    format!("{} W", decimal(value as f64, if value < 10.0 { 2 } else { 1 }))
}

pub fn rate(bytes_per_second: u64) -> String {
    const UNITS: [&str; 4] = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut value = bytes_per_second as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{} {}", decimal(value, if unit == 0 { 0 } else { 1 }), UNITS[unit])
}

/// Série de tamanho fixo. Leituras ausentes viram NaN e interrompem a linha do gráfico.
#[derive(Default)]
pub struct History(VecDeque<f32>);

impl History {
    pub fn push(&mut self, value: Option<f32>) {
        if self.0.len() == HISTORY_LEN {
            self.0.pop_front();
        }
        self.0.push_back(value.unwrap_or(f32::NAN));
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = f32> + '_ {
        self.0.iter().copied()
    }
}

/// Faixa do eixo em múltiplos de 10 °C, com ao menos 20 °C de amplitude.
pub fn temperature_range<'a>(series: impl IntoIterator<Item = &'a History>) -> (f32, f32) {
    let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
    for value in series.into_iter().flat_map(History::values).filter(|v| v.is_finite()) {
        low = low.min(value);
        high = high.max(value);
    }
    if !low.is_finite() {
        return (30.0, 50.0);
    }
    let low = (low / 10.0).floor() * 10.0;
    let high = (high / 10.0).ceil() * 10.0;
    (low, high.max(low + 20.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_is_cpu_temperature_only() {
        assert_eq!(parse_selection(None), vec![BarMetric::CpuTemp]);
    }

    #[test]
    fn empty_selection_is_preserved() {
        assert!(parse_selection(Some("")).is_empty());
    }

    #[test]
    fn selection_round_trips_in_menu_order_and_drops_unknown_keys() {
        let parsed = parse_selection(Some("mem_pressure,nope,cpu_temp"));
        assert_eq!(parsed, vec![BarMetric::CpuTemp, BarMetric::MemPressure]);
        assert_eq!(parse_selection(Some(&serialize_selection(&parsed))), parsed);
    }

    #[test]
    fn single_cpu_temperature_matches_compact_title() {
        let mut m = Metrics::default();
        m.temp.cpu_temp_avg = Some(36.6);
        assert_eq!(bar_title(&[BarMetric::CpuTemp], Some(&m)), "37°C");
    }

    #[test]
    fn multiple_metrics_are_labelled() {
        let mut m = Metrics::default();
        m.temp.cpu_temp_avg = Some(40.0);
        m.memory.pressure_percent = Some(65);
        let title = bar_title(&[BarMetric::CpuTemp, BarMetric::MemPressure], Some(&m));
        assert_eq!(title, "CPU 40°  MEM 65%");
    }

    #[test]
    fn segments_separate_labels_from_values() {
        let mut m = Metrics::default();
        m.temp.cpu_temp_avg = Some(40.0);
        m.memory.pressure_percent = Some(65);
        let parts = bar_segments(&[BarMetric::CpuTemp, BarMetric::MemPressure], Some(&m));
        assert_eq!(
            parts,
            vec![("CPU", "\u{2007}40°".to_string()), ("MEM", "\u{2007}65%".to_string())]
        );
        assert_eq!(
            bar_segments(&[BarMetric::CpuTemp], Some(&m)),
            vec![("", "\u{2007}40°C".to_string())]
        );
    }

    #[test]
    fn values_always_occupy_three_digit_slots() {
        for (value, expected) in [
            ("8%", "\u{2007}\u{2007}8%"),
            ("88%", "\u{2007}88%"),
            ("100%", "100%"),
        ] {
            assert_eq!(pad_digits(value.to_string(), "100%"), expected);
        }
        assert_eq!(pad_digits("8,6 GB".to_string(), "00,0 GB"), "\u{2007}8,6 GB");
        assert_eq!(pad_digits("0,27 W".to_string(), "00,0 W"), "0,27 W");
    }

    #[test]
    fn missing_values_and_oversized_values_are_not_padded() {
        assert_eq!(pad_digits("—".to_string(), "100%"), "—");
        assert_eq!(pad_digits("128,0 GB".to_string(), "00,0 GB"), "128,0 GB");
    }

    #[test]
    fn template_matches_selection() {
        assert_eq!(bar_template(&[BarMetric::CpuTemp]), vec![("", "100°C".to_string())]);
        assert_eq!(
            bar_template(&[BarMetric::CpuTemp, BarMetric::MemPressure]),
            vec![("CPU", "100°".to_string()), ("MEM", "100%".to_string())]
        );
        assert!(bar_template(&[]).is_empty());
    }

    #[test]
    fn missing_readings_show_a_dash() {
        let m = Metrics::default();
        assert_eq!(bar_title(&[BarMetric::CpuTemp], Some(&m)), "—");
        assert_eq!(bar_title(&[BarMetric::CpuTemp], None), "—");
    }

    #[test]
    fn units_use_brazilian_decimal_separator() {
        assert_eq!(gigabytes(1_610_612_736), "1,5 GB");
        assert_eq!(watts(0.268), "0,27 W");
        assert_eq!(watts(12.34), "12,3 W");
        assert_eq!(rate(512), "512 B/s");
        assert_eq!(rate(1536), "1,5 KB/s");
    }

    #[test]
    fn history_is_bounded() {
        let mut h = History::default();
        for i in 0..(HISTORY_LEN + 5) {
            h.push(Some(i as f32));
        }
        assert_eq!(h.values().len(), HISTORY_LEN);
        assert_eq!(h.values().next(), Some(5.0));
    }

    #[test]
    fn temperature_range_ignores_gaps_and_keeps_minimum_span() {
        let mut h = History::default();
        h.push(Some(37.0));
        h.push(None);
        h.push(Some(38.0));
        assert_eq!(temperature_range([&h]), (30.0, 50.0));
        h.push(Some(83.0));
        assert_eq!(temperature_range([&h]), (30.0, 90.0));
    }
}
