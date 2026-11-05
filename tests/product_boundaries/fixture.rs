//! Stored-method ZIP fixture for a minimal public OOXML workbook.
//!
//! The bytes follow PKZIP APPNOTE stored entries and ECMA-376 part names.
//! This module does not parse archives and does not call the extractor.

/// Public CRC-32 (ISO-HDLC) of `123456789`.
pub const CRC32_VECTOR: u32 = 0xCBF4_3926;

/// CRC-32/ISO-HDLC checksum used by stored ZIP entries.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Build a stored ZIP. Entry names are archive paths with `/` separators.
pub fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let name_bytes = name.as_bytes();
        assert!(name_bytes.len() <= u16::MAX as usize);
        assert!(data.len() <= u32::MAX as usize);
        let offset = u32::try_from(locals.len()).expect("zip offset");
        let checksum = crc32(data);
        locals.extend_from_slice(&local_header(name_bytes, data, checksum));
        locals.extend_from_slice(name_bytes);
        locals.extend_from_slice(data);
        central.extend_from_slice(&central_header(name_bytes, data, checksum, offset));
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32::try_from(locals.len()).expect("central offset");
    let central_size = u32::try_from(central.len()).expect("central size");
    let entries_len = u16::try_from(entries.len()).expect("entry count");
    let mut out = locals;
    out.extend_from_slice(&central);
    out.extend_from_slice(&end_of_central_directory(
        entries_len,
        central_size,
        central_offset,
    ));
    out
}

/// Workbook whose cells reference shared-string indexes, in row order.
pub fn shared_string_workbook(strings: &[String], rows: &[(u32, usize)]) -> Vec<u8> {
    let shared = shared_strings_xml(strings, rows.len());
    let sheet = sheet_xml(rows);
    let content_types = content_types_xml();
    let root_rels = root_rels_xml();
    let workbook = workbook_xml();
    let workbook_rels = workbook_rels_xml();
    let styles = styles_xml();
    stored_zip(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes()),
        ("xl/styles.xml", styles.as_bytes()),
        ("xl/sharedStrings.xml", shared.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}

fn shared_strings_xml(strings: &[String], references: usize) -> String {
    let mut body = String::new();
    for text in strings {
        body.push_str("<si><t xml:space=\"preserve\">");
        body.push_str(&xml_escape(text));
        body.push_str("</t></si>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{references}\" uniqueCount=\"{unique}\">{body}</sst>",
        unique = strings.len(),
    )
}

fn sheet_xml(rows: &[(u32, usize)]) -> String {
    let mut body = String::new();
    for (row, string_index) in rows {
        let excel_row = row + 1;
        body.push_str(&format!(
            "<row r=\"{excel_row}\"><c r=\"A{excel_row}\" t=\"s\"><v>{string_index}</v></c></row>"
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{body}</sheetData></worksheet>"
    )
}

fn content_types_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/></Types>".to_string()
}

fn root_rels_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>".to_string()
}

fn workbook_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>".to_string()
}

fn workbook_rels_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>".to_string()
}

fn styles_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"1\"><fill><patternFill patternType=\"none\"/></fill></fills><borders count=\"1\"><border/></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs></styleSheet>".to_string()
}

fn local_header(name: &[u8], data: &[u8], checksum: u32) -> [u8; 30] {
    let mut header = [0u8; 30];
    header[0..4].copy_from_slice(&0x0403_4b50u32.to_le_bytes());
    header[4..6].copy_from_slice(&20u16.to_le_bytes());
    header[6..8].copy_from_slice(&0x0800u16.to_le_bytes());
    put_sizes(&mut header, name, data, checksum, 14);
    header
}

fn central_header(name: &[u8], data: &[u8], checksum: u32, offset: u32) -> [u8; 46] {
    let mut header = [0u8; 46];
    header[0..4].copy_from_slice(&0x0201_4b50u32.to_le_bytes());
    header[4..6].copy_from_slice(&20u16.to_le_bytes());
    header[6..8].copy_from_slice(&20u16.to_le_bytes());
    header[8..10].copy_from_slice(&0x0800u16.to_le_bytes());
    put_sizes_central(&mut header, name, data, checksum);
    header[42..46].copy_from_slice(&offset.to_le_bytes());
    header
}

fn put_sizes(header: &mut [u8; 30], name: &[u8], data: &[u8], checksum: u32, crc_at: usize) {
    header[crc_at..crc_at + 4].copy_from_slice(&checksum.to_le_bytes());
    let size = u32::try_from(data.len()).expect("size").to_le_bytes();
    header[crc_at + 4..crc_at + 8].copy_from_slice(&size);
    header[crc_at + 8..crc_at + 12].copy_from_slice(&size);
    let name_len = u16::try_from(name.len()).expect("name").to_le_bytes();
    header[crc_at + 12..crc_at + 14].copy_from_slice(&name_len);
}

fn put_sizes_central(header: &mut [u8; 46], name: &[u8], data: &[u8], checksum: u32) {
    header[16..20].copy_from_slice(&checksum.to_le_bytes());
    let size = u32::try_from(data.len()).expect("size").to_le_bytes();
    header[20..24].copy_from_slice(&size);
    header[24..28].copy_from_slice(&size);
    let name_len = u16::try_from(name.len()).expect("name").to_le_bytes();
    header[28..30].copy_from_slice(&name_len);
}

fn end_of_central_directory(entries: u16, central_size: u32, central_offset: u32) -> [u8; 22] {
    let mut header = [0u8; 22];
    header[0..4].copy_from_slice(&0x0605_4b50u32.to_le_bytes());
    header[8..10].copy_from_slice(&entries.to_le_bytes());
    header[10..12].copy_from_slice(&entries.to_le_bytes());
    header[12..16].copy_from_slice(&central_size.to_le_bytes());
    header[16..20].copy_from_slice(&central_offset.to_le_bytes());
    header
}
