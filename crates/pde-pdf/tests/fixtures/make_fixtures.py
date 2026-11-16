#!/usr/bin/env python3
"""Génère les PDF de test de pde-pdf (fichiers committés à côté de ce script).

Usage : python3 make_fixtures.py
  - simple.pdf    : écrit à la main, sans dépendance ;
  - encrypted.pdf : simple.pdf chiffré en AES-256, nécessite `pypdf` et
                    `cryptography` (pip install pypdf cryptography).

Les contenus sont fictifs ; le mot de passe de test n'est pas un secret.
simple.pdf est reproductible ; encrypted.pdf change à chaque génération
(sel et vecteurs d'initialisation aléatoires) : ne le régénérer que si besoin.
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent
# Mot de passe du PDF chiffré de test (repris par les tests Rust).
TEST_PASSWORD = "mot-de-passe-de-test"


def text_string(value: str) -> bytes:
    """Chaîne de texte PDF en UTF-16BE avec BOM (accents sûrs)."""
    return b"<FEFF" + value.encode("utf-16-be").hex().upper().encode() + b">"


def stream(content: bytes) -> bytes:
    return b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream"


def build_simple() -> bytes:
    page1 = (
        b"BT /F1 24 Tf 72 760 Td (Bonjour PdfEditor) Tj ET\n"
        # Rectangle noir plein : zone sombre connue pour les tests de rendu.
        b"0 g 72 600 200 100 re f\n"
    )
    page2 = b"BT /F1 18 Tf 72 760 Td (Page deux) Tj ET\n"
    objects = {
        1: b"<< /Type /Catalog /Pages 2 0 R /Outlines 8 0 R /PageMode /UseOutlines >>",
        2: b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>",
        # Page 2 tournée d'un quart de tour : affichée en paysage (842 × 595).
        4: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Rotate 90 "
        b"/Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>",
        5: stream(page1),
        6: stream(page2),
        7: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        8: b"<< /Type /Outlines /First 9 0 R /Last 10 0 R /Count 3 >>",
        9: b"<< /Title " + text_string("Introduction") + b" /Parent 8 0 R /Next 10 0 R "
        b"/Dest [3 0 R /Fit] >>",
        10: b"<< /Title " + text_string("Annexe") + b" /Parent 8 0 R /Prev 9 0 R "
        b"/First 11 0 R /Last 11 0 R /Count 1 /Dest [4 0 R /Fit] >>",
        11: b"<< /Title " + text_string("Détail") + b" /Parent 10 0 R /Dest [4 0 R /Fit] >>",
        12: b"<< /Title " + text_string("Document de test") + b" /Producer (make_fixtures.py) >>",
    }
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}
    for number in sorted(objects):
        offsets[number] = len(out)
        out += b"%d 0 obj\n" % number + objects[number] + b"\nendobj\n"
    xref = len(out)
    count = max(objects) + 1
    out += b"xref\n0 %d\n0000000000 65535 f \n" % count
    for number in range(1, count):
        out += b"%010d 00000 n \n" % offsets[number]
    out += b"trailer\n<< /Size %d /Root 1 0 R /Info 12 0 R >>\n" % count
    out += b"startxref\n%d\n%%%%EOF\n" % xref
    return bytes(out)


def build_encrypted(simple: Path) -> bytes:
    import io

    from pypdf import PdfReader, PdfWriter

    writer = PdfWriter(clone_from=PdfReader(simple))
    writer.encrypt(user_password=TEST_PASSWORD, owner_password=None, algorithm="AES-256")
    buffer = io.BytesIO()
    writer.write(buffer)
    return buffer.getvalue()


def main() -> None:
    simple = HERE / "simple.pdf"
    simple.write_bytes(build_simple())
    print(f"écrit : {simple.name}")
    try:
        (HERE / "encrypted.pdf").write_bytes(build_encrypted(simple))
        print("écrit : encrypted.pdf")
    except ImportError:
        print("pypdf / cryptography absents : encrypted.pdf non régénéré")


if __name__ == "__main__":
    main()
