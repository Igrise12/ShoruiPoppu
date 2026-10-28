#!/usr/bin/env python3
"""Create a synthetic XLSX workbook for local performance checks."""
import argparse
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", help="new XLSX file to create")
parser.add_argument("--rows", type=int, default=200_000)
args = parser.parse_args()
if not 1 <= args.rows <= 1_000_000:
    parser.error("--rows must be between 1 and 1,000,000")

with open(args.output, "xb") as output, zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    archive.writestr("_rels/.rels", '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>')
    archive.writestr("xl/workbook.xml", '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Large" sheetId="1" r:id="rId1"/></sheets></workbook>')
    archive.writestr("xl/_rels/workbook.xml.rels", '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>')
    with archive.open("xl/worksheets/sheet1.xml", "w") as sheet:
        sheet.write(b'<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>')
        for row in range(1, args.rows + 1):
            sheet.write(f'<row r="{row}"><c r="A{row}"><v>{row}</v></c></row>'.encode())
        sheet.write(b'</sheetData></worksheet>')
