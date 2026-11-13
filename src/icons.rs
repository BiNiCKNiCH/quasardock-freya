//! Echte App-Icons (SPEC §7): PNG direkt, ICO-Mehrbild dekomprimieren,
//! `.exe`/`.dll` per PE-Ressourcen („MZ"). Fehlende/defekte Quelle →
//! Buchstaben-Fallback, nie Toast-Spam.
//!
//! Jede `get_app_icon`-Anfrage schreibt eine Zeile nach `Data/logs/icons.txt`
//! (Pfad, Ergebnis, Fehlerort). Pfadformen sind Windows-tauglich
//! (Backslash, Laufwerksbuchstabe) — die Klassifizierung nutzt nur die
//! Dateiendung, unabhängig von Trennzeichen.

use std::fs;
use std::io::Write;
use std::path::Path;

/// Art der Icon-Quelle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconKind {
    Png,
    Ico,
    Pe,
    Unknown,
}

/// Klassifizierte Icon-Quelle (Pfad unverändert weitergereicht).
#[derive(Clone, Debug)]
pub struct IconSource {
    pub kind: IconKind,
    pub path: String,
}

/// Decodiertes Icon als PNG-Bytes (für die UI-Darstellung).
#[derive(Clone, Debug)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

/// Klassifiziert eine Icon-Quelle anhand der Dateiendung. Windows-Pfade mit
/// Backslash und Laufwerksbuchstaben (z. B. `C:\Apps\app.exe`) werden korrekt
/// erkannt, weil nur das Suffix geprüft wird.
pub fn classify_icon(path: &str) -> IconSource {
    let lower = path.to_lowercase();
    let kind = if lower.ends_with(".png") {
        IconKind::Png
    } else if lower.ends_with(".ico") {
        IconKind::Ico
    } else if lower.ends_with(".exe") || lower.ends_with(".dll") || lower.ends_with(".sys") {
        IconKind::Pe
    } else {
        IconKind::Unknown
    };
    IconSource {
        kind,
        path: path.to_string(),
    }
}

/// Initial-Buchstabe für den Buchstaben-Fallback (SPEC §7).
pub fn fallback_initial(name: &str) -> char {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?')
}

/// Lädt ein App-Icon. Gibt `None` zurück (und protokolliert den Fallback),
/// wenn die Quelle fehlt, unbekannt oder nicht lesbar ist.
pub fn get_app_icon(icon_path: &str, logs_dir: &Path) -> Option<IconImage> {
    if icon_path.trim().is_empty() {
        log_icon_event(logs_dir, icon_path, "fallback", Some("leerer pfad"));
        return None;
    }
    let src = classify_icon(icon_path);
    let result = match src.kind {
        IconKind::Png => load_png(&src.path),
        IconKind::Ico => load_ico(&src.path),
        IconKind::Pe => load_pe_icon(&src.path),
        IconKind::Unknown => {
            // Unbekannte Endung: trotzdem versuchen, als Bild zu laden.
            fs::read(&src.path)
                .ok()
                .and_then(|b| decode_bytes_to_png(&b))
        }
    };

    match &result {
        Some(img) => log_icon_event(
            logs_dir,
            icon_path,
            "ok",
            Some(&format!("{}x{}", img.width, img.height)),
        ),
        None => log_icon_event(logs_dir, icon_path, "fallback", Some("decode fehlgeschlagen")),
    }
    result
}

/// Schreibt genau eine Diagnosezeile nach `Data/logs/icons.txt` (SPEC §7).
pub fn log_icon_event(logs_dir: &Path, source: &str, result: &str, detail: Option<&str>) {
    let _ = fs::create_dir_all(logs_dir);
    let file = logs_dir.join("icons.txt");
    let ts = chrono_stamp();
    let detail = detail.unwrap_or("");
    let line = format!(
        "{}  src={}  result={}  detail={}\n",
        ts, source, result, detail
    );
    // Anhängen; bei Fehler still (kein Toast-Spam).
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&file) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Einfacher, allocationsarmer Zeitstempel (kein externes Crate nötig).
fn chrono_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // ISO-ähnlich: YYYY-MM-DDTHH:MM:SS (UTC, vereinfacht).
    let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
    let day = secs / 86400;
    format!("D{:05}T{:02}:{:02}:{:02}Z", day, h, m, s)
}

// ---------------------------------------------------------------------------
// Dekodierung
// ---------------------------------------------------------------------------

/// Dekodiert Bild-Bytes (PNG/BMP/ICO-Rohdaten) und liefert normalisiertes PNG.
fn decode_bytes_to_png(bytes: &[u8]) -> Option<IconImage> {
    let img = image::load_from_memory(bytes).ok()?;
    use image::ImageEncoder;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut buf = Vec::with_capacity((w * h * 4) as usize + 1024);
    {
        let enc = image::codecs::png::PngEncoder::new(&mut buf);
        enc.write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
    }
    Some(IconImage {
        width: w,
        height: h,
        png: buf,
    })
}

/// Kodiert RGBA-Daten (schon dekodiert) direkt als PNG-Bytes.
fn png_from_rgba(rgba: &[u8], w: u32, h: u32) -> Option<IconImage> {
    use image::ImageEncoder;
    let mut buf = Vec::with_capacity((w * h * 4) as usize + 1024);
    {
        let enc = image::codecs::png::PngEncoder::new(&mut buf);
        enc.write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
    }
    Some(IconImage {
        width: w,
        height: h,
        png: buf,
    })
}

fn load_png(path: &str) -> Option<IconImage> {
    let bytes = fs::read(path).ok()?;
    decode_bytes_to_png(&bytes)
}

fn load_ico(path: &str) -> Option<IconImage> {
    let mut file = fs::File::open(path).ok()?;
    let icon_dir = ico::IconDir::read(&mut file).ok()?;
    let mut entries: Vec<ico::IconDirEntry> = icon_dir.entries().to_vec();
    // Größtes Bild zuerst (meiste Rohdaten).
    entries.sort_by(|a, b| b.data().len().cmp(&a.data().len()));
    let best = entries.first()?;
    // `decode` liefert ein `ico::IconImage` mit RGBA (auch bei BMP-codierten Einträgen).
    let img = best.decode().ok()?;
    png_from_rgba(&img.rgba_data(), img.width(), img.height())
}

fn load_pe_icon(path: &str) -> Option<IconImage> {
    let bytes = fs::read(path).ok()?;
    let ico = extract_pe_icon(&bytes)?;
    // Über den ico-Crate-Decoder dekodieren (kennt das DIB-Format der Einträge).
    let mut cursor = std::io::Cursor::new(ico);
    let dir = ico::IconDir::read(&mut cursor).ok()?;
    let best = dir.entries().iter().max_by_key(|e| e.data().len())?;
    let img = best.decode().ok()?;
    png_from_rgba(&img.rgba_data(), img.width(), img.height())
}

// ---------------------------------------------------------------------------
// PE-Ressourcen extrahieren („MZ" → RT_GROUP_ICON → RT_ICON)
// ---------------------------------------------------------------------------

/// Extrahiert das erste (größte) Icon aus einer PE-Datei und liefert gültige
/// `.ico`-Rohdaten. Bei jeder Unstimmigkeit wird `None` geliefert (kein Panic).
fn extract_pe_icon(file: &[u8]) -> Option<Vec<u8>> {
    if file.len() < 64 || &file[0..2] != b"MZ" {
        return None;
    }
    let e_lfanew = read_u32(file, 0x3C)? as usize;
    if e_lfanew + 4 >= file.len() || &file[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }
    let coff = e_lfanew + 4;
    let num_sections = read_u16(file, coff + 2)? as usize;
    let opt_hdr_size = read_u16(file, coff + 16)? as usize;
    let opt_hdr = coff + 20;
    if opt_hdr + opt_hdr_size >= file.len() {
        return None;
    }
    let magic = read_u16(file, opt_hdr)?;
    // Offset der Data-Directory-Anzahl: PE32=0x5C, PE32+=0x6C.
    let num_rva_off = if magic == 0x10b { 0x5C } else { 0x6C };
    let _num_rva = read_u32(file, opt_hdr + num_rva_off)? as usize;
    // Resource-Directory ist Eintrag 2 (0-basiert).
    let res_rva = read_u32(file, opt_hdr + num_rva_off + 8 + 2 * 8)?;
    if res_rva == 0 {
        return None;
    }
    let section_base = opt_hdr + opt_hdr_size;
    let rva_to_off = |rva: u32| -> Option<usize> {
        rva_to_file_offset(file, section_base, num_sections, rva)
    };
    let res_off = rva_to_off(res_rva)?;

    // RT_GROUP_ICON (Typ 14) finden.
    let group_icon_off = find_resource_id(file, res_off, rva_to_off, 14)?;
    // Ersten Eintrag (Sprache) im Group-Icon-Unterverzeichnis nehmen.
    let group_data = first_leaf(file, group_icon_off, rva_to_off)?;
    let grp = parse_group_icon(file, group_data)?;

    // RT_ICON (Typ 3) Unterschema für die eigentlichen Bilddaten.
    let icon_type_off = find_resource_id(file, res_off, rva_to_off, 3)?;

    // Bilddaten einsammeln.
    let mut images: Vec<Vec<u8>> = Vec::new();
    for ge in &grp.entries {
        if let Some(data_off) = find_resource_id(file, icon_type_off, rva_to_off, ge.id as u32) {
            if let Some(leaf) = first_leaf(file, data_off, rva_to_off) {
                if let Some(bytes) = read_resource_data(file, leaf) {
                    images.push(bytes);
                }
            }
        }
    }
    if images.is_empty() {
        return None;
    }

    // .ico zusammenbauen: ICONDIR + ICONDIRENTRY* + Daten.
    let count = grp.entries.len() as u16;
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&[0u8, 0, 1, 0]); // reserved, type=1
    out.extend_from_slice(&count.to_le_bytes());
    let mut data_off_pos = 6 + (16 * grp.entries.len());
    let mut data_blob = Vec::new();
    for (i, ge) in grp.entries.iter().enumerate() {
        let img = &images[i];
        // ICONDIRENTRY (16 Bytes).
        out.push(ge.width);
        out.push(ge.height);
        out.push(ge.color_count);
        out.push(0); // reserved
        out.extend_from_slice(&ge.planes.to_le_bytes());
        out.extend_from_slice(&ge.bit_count.to_le_bytes());
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data_off_pos as u32).to_le_bytes());
        data_blob.extend_from_slice(img);
        data_off_pos += img.len();
    }
    out.extend_from_slice(&data_blob);
    Some(out)
}

struct GroupIcon {
    entries: Vec<GroupIconEntry>,
}

struct GroupIconEntry {
    width: u8,
    height: u8,
    color_count: u8,
    planes: u16,
    bit_count: u16,
    id: u16,
}

fn parse_group_icon(file: &[u8], data_off: usize) -> Option<GroupIcon> {
    // GRPICONDIR: reserved(2)=0, type(2)=1, count(2).
    let _reserved = read_u16(file, data_off)?;
    let _type = read_u16(file, data_off + 2)?;
    let count = read_u16(file, data_off + 4)? as usize;
    let mut entries = Vec::with_capacity(count);
    let base = data_off + 6;
    for i in 0..count {
        let e = base + i * 14;
        if e + 14 > file.len() {
            return None;
        }
        entries.push(GroupIconEntry {
            width: file[e],
            height: file[e + 1],
            color_count: file[e + 2],
            planes: read_u16(file, e + 4)?,
            bit_count: read_u16(file, e + 6)?,
            id: read_u16(file, e + 12)?,
        });
    }
    Some(GroupIcon { entries })
}

/// Findet im Ressourcen-Unterverzeichnis bei `dir_off` den Eintrag mit der
/// angegebenen ID (nur ID-Einträge, keine namensbasierten) und liefert dessen
/// Offset (relativ zu `res_off`).
fn find_resource_id<F>(file: &[u8], dir_off: usize, rva_to_off: F, id: u32) -> Option<usize>
where
    F: Fn(u32) -> Option<usize>,
{
    let (named, ids) = read_dir_counts(file, dir_off)?;
    let total = named + ids;
    let mut e = dir_off + 16;
    for _ in 0..total {
        if e + 8 > file.len() {
            return None;
        }
        let name_id = read_u32(file, e)?;
        let offset = read_u32(file, e + 4)?;
        let is_id = name_id & 0x8000_0000 == 0;
        if is_id && name_id == id {
            // High-Bit im Offset => Unterverzeichnis; Offset ist relativ zu res_off.
            if offset & 0x8000_0000 != 0 {
                return Some((offset & 0x7FFF_FFFF) as usize);
            }
            // Sonst Blatt: RVA -> Datei-Offset.
            return rva_to_off(offset);
        }
        e += 8;
    }
    None
}

/// Nimmt den ersten Blatt-Eintrag unter `dir_off` (rekursiv) und liefert dessen
/// Ressourcen-Offset (Datei-Offset des DATA-Eintrags).
fn first_leaf<F>(file: &[u8], dir_off: usize, rva_to_off: F) -> Option<usize>
where
    F: Fn(u32) -> Option<usize> + Copy,
{
    let (named, ids) = read_dir_counts(file, dir_off)?;
    let total = named + ids;
    let mut e = dir_off + 16;
    for _ in 0..total {
        if e + 8 > file.len() {
            return None;
        }
        let offset = read_u32(file, e + 4)?;
        if offset & 0x8000_0000 != 0 {
            // Unterverzeichnis: relativ zu res_off (dir_off liegt bereits relativ).
            let sub = (offset & 0x7FFF_FFFF) as usize;
            if let Some(leaf) = first_leaf(file, sub, rva_to_off) {
                return Some(leaf);
            }
        } else {
            return rva_to_off(offset);
        }
        e += 8;
    }
    None
}

/// Liest die eigentlichen Bilddaten eines DATA-Eintrags (Offset zeigt auf
/// IMAGE_RESOURCE_DATA_ENTRY: RVA(4) + Size(4)).
fn read_resource_data(file: &[u8], data_off: usize) -> Option<Vec<u8>> {
    let rva = read_u32(file, data_off)?;
    let size = read_u32(file, data_off + 4)? as usize;
    let off = rva_to_file_offset_global(file, rva)?;
    if off + size > file.len() {
        return None;
    }
    Some(file[off..off + size].to_vec())
}

fn read_dir_counts(file: &[u8], dir_off: usize) -> Option<(usize, usize)> {
    if dir_off + 16 > file.len() {
        return None;
    }
    let named = read_u16(file, dir_off + 12)? as usize;
    let ids = read_u16(file, dir_off + 14)? as usize;
    Some((named, ids))
}

/// RVA -> Datei-Offset mittels Sektionstabelle.
fn rva_to_file_offset(file: &[u8], section_base: usize, num_sections: usize, rva: u32) -> Option<usize> {
    rva_to_file_offset_impl(file, section_base, num_sections, rva)
}

fn rva_to_file_offset_global(file: &[u8], rva: u32) -> Option<usize> {
    // Globale Variante ohne bekannten Sektionsbasis: Sektionen von coff+20+opt_hdr.
    let e_lfanew = read_u32(file, 0x3C)? as usize;
    let coff = e_lfanew + 4;
    let opt_hdr_size = read_u16(file, coff + 16)? as usize;
    let section_base = coff + 20 + opt_hdr_size;
    let num_sections = read_u16(file, coff + 2)? as usize;
    rva_to_file_offset_impl(file, section_base, num_sections, rva)
}

fn rva_to_file_offset_impl(file: &[u8], section_base: usize, num_sections: usize, rva: u32) -> Option<usize> {
    let mut s = section_base;
    for _ in 0..num_sections {
        if s + 40 > file.len() {
            return None;
        }
        let va = read_u32(file, s + 12)?;
        let raw = read_u32(file, s + 20)? as usize;
        let size = read_u32(file, s + 16)? as usize;
        if rva >= va && rva < va + size as u32 {
            return Some(raw + (rva - va) as usize);
        }
        s += 40;
    }
    None
}

fn read_u16(file: &[u8], off: usize) -> Option<u16> {
    if off + 2 > file.len() {
        return None;
    }
    Some(u16::from_le_bytes([file[off], file[off + 1]]))
}

fn read_u32(file: &[u8], off: usize) -> Option<u32> {
    if off + 4 > file.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        file[off],
        file[off + 1],
        file[off + 2],
        file[off + 3],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn tmp_logs(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("qd-icons-test-{}-{}-{}", pid, name, n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn klassifizierung_windows_pfade() {
        assert_eq!(classify_icon("C:\\Apps\\app.exe").kind, IconKind::Pe);
        assert_eq!(classify_icon("C:\\icons\\a.ICO").kind, IconKind::Ico);
        assert_eq!(classify_icon("D:/Bilder/logo.png").kind, IconKind::Png);
        assert_eq!(classify_icon("//server/share/x.dll").kind, IconKind::Pe);
        assert_eq!(classify_icon("keine_endung").kind, IconKind::Unknown);
    }

    #[test]
    fn fallback_initial_buchstabe() {
        assert_eq!(fallback_initial("Browser"), 'B');
        assert_eq!(fallback_initial("123 test"), '1');
        assert_eq!(fallback_initial(""), '?');
        assert_eq!(fallback_initial("übung"), 'Ü');
    }

    #[test]
    fn echtes_windows_ico_wird_geladen() {
        // Echtes Multi-Image-ICO erzeugen (ico-Crate) und dekodieren.
        let logs = tmp_logs("ico");
        let ico_path = logs.join("test.ico");
        make_real_ico(&ico_path);
        let img = get_app_icon(ico_path.to_str().unwrap(), &logs).expect("ICO muss laden");
        assert!(img.width > 0 && img.height > 0, "ICO-Bildmaße ungültig");
        assert!(!img.png.is_empty(), "PNG-Bytes fehlen");
        // Diagnosezeile vorhanden?
        let log = fs::read_to_string(logs.join("icons.txt")).unwrap();
        assert!(log.contains("result=ok"), "Log: {}", log);
        let _ = fs::remove_dir_all(&logs);
    }

    #[test]
    fn fehlende_quelle_fallback_und_log() {
        let logs = tmp_logs("missing");
        let res = get_app_icon("C:\\nicht\\vorhanden.ico", &logs);
        assert!(res.is_none(), "Fehlende Quelle -> None");
        let log = fs::read_to_string(logs.join("icons.txt")).unwrap();
        assert!(log.contains("result=fallback"), "Log: {}", log);
        assert!(log.contains("C:\\nicht\\vorhanden.ico"), "Pfad muss im Log stehen");
        let _ = fs::remove_dir_all(&logs);
    }

    #[test]
    fn pe_fehlerhaft_kein_panic() {
        let logs = tmp_logs("pe");
        // Datei mit „MZ", aber keinem gültigen PE-Header.
        let bad = logs.join("bad.exe");
        fs::write(&bad, b"MZ\x00\x00kurz").unwrap();
        let res = get_app_icon(bad.to_str().unwrap(), &logs);
        assert!(res.is_none());
        // Ein synthetisches „MZ" ohne weitere Struktur darf nicht abstürzen.
        let _ = fs::remove_dir_all(&logs);
    }

    /// Erzeugt ein echtes Multi-Bild-ICO (16x16 und 32x32) via `ico`-Crate.
    fn make_real_ico(path: &Path) {
        let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
        for size in [16u32, 32u32] {
            let rgba = vec![0u8; (size * size * 4) as usize];
            let img = ico::IconImage::from_rgba_data(size, size, rgba);
            let entry = ico::IconDirEntry::encode(&img).unwrap();
            icon_dir.add_entry(entry);
        }
        let mut f = std::fs::File::create(path).unwrap();
        icon_dir.write(&mut f).unwrap();
    }
}
